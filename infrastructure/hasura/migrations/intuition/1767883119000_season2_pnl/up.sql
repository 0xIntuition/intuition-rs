-- Season 2 PnL infrastructure

-- ========================================
-- POSITION CHANGE HYPERTABLE
-- ========================================

CREATE TABLE IF NOT EXISTS position_change (
  id BIGSERIAL,
  created_at TIMESTAMPTZ NOT NULL,
  account_id TEXT NOT NULL,
  term_id TEXT NOT NULL,
  curve_id NUMERIC(78, 0) NOT NULL,
  shares_delta NUMERIC(78, 0) NOT NULL,
  assets_in NUMERIC(78, 0) NOT NULL DEFAULT 0,
  assets_out NUMERIC(78, 0) NOT NULL DEFAULT 0,
  event_type TEXT NOT NULL,
  event_id TEXT NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  transaction_hash TEXT NOT NULL,
  log_index BIGINT NOT NULL,
  CONSTRAINT position_change_event_type_check CHECK (event_type IN ('deposit', 'redemption'))
) WITH (
  timescaledb.hypertable,
  timescaledb.partition_column='created_at'
);

SELECT set_chunk_time_interval('position_change', INTERVAL '1 day');

-- ========================================
-- INDEXES
-- ========================================

CREATE INDEX IF NOT EXISTS idx_position_change_account_term_curve_time
  ON position_change (account_id, term_id, curve_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_position_change_term_curve_time
  ON position_change (term_id, curve_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_position_change_account_time
  ON position_change (account_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_position_change_event
  ON position_change (event_type, event_id);

-- ========================================
-- COMPRESSION
-- ========================================

ALTER TABLE position_change SET (
  timescaledb.compress,
  timescaledb.compress_segmentby = 'account_id, term_id, curve_id',
  timescaledb.compress_orderby = 'created_at DESC'
);

SELECT add_compression_policy('position_change', INTERVAL '7 days');

-- ========================================
-- CONTINUOUS AGGREGATES
-- ========================================

CREATE MATERIALIZED VIEW IF NOT EXISTS position_change_hourly
WITH (timescaledb.continuous)
AS SELECT
  time_bucket('1 h'::interval, created_at) AS bucket,
  account_id,
  term_id,
  curve_id,
  SUM(shares_delta) AS shares_delta_period,
  SUM(assets_in) AS assets_in_period,
  SUM(assets_out) AS assets_out_period,
  COUNT(*) AS transaction_count
FROM position_change
GROUP BY 1, 2, 3, 4;

ALTER MATERIALIZED VIEW position_change_hourly
  SET (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('position_change_hourly',
  start_offset => INTERVAL '3 hours',
  end_offset => INTERVAL '1 hour',
  schedule_interval => INTERVAL '1 hour');

CREATE MATERIALIZED VIEW IF NOT EXISTS position_change_daily
WITH (timescaledb.continuous)
AS SELECT
  time_bucket('1 day'::interval, bucket) AS bucket,
  account_id,
  term_id,
  curve_id,
  SUM(shares_delta_period) AS shares_delta_period,
  SUM(assets_in_period) AS assets_in_period,
  SUM(assets_out_period) AS assets_out_period,
  SUM(transaction_count) AS transaction_count
FROM position_change_hourly
GROUP BY 1, 2, 3, 4;

ALTER MATERIALIZED VIEW position_change_daily
  SET (timescaledb.materialized_only = false);

SELECT add_continuous_aggregate_policy('position_change_daily',
  start_offset => INTERVAL '3 days',
  end_offset => INTERVAL '1 day',
  schedule_interval => INTERVAL '1 day');

-- ========================================
-- PNL CHART FUNCTION
-- ========================================

CREATE OR REPLACE FUNCTION get_position_pnl_chart(
  p_account_id TEXT,
  p_term_id TEXT,
  p_curve_id NUMERIC(78, 0),
  p_start_time TIMESTAMPTZ,
  p_end_time TIMESTAMPTZ,
  p_interval INTERVAL DEFAULT INTERVAL '1 hour'
)
RETURNS TABLE (
  time TIMESTAMPTZ,
  shares_total NUMERIC(78, 0),
  share_price NUMERIC(78, 0),
  equity_value NUMERIC,
  total_assets_in NUMERIC(78, 0),
  total_assets_out NUMERIC(78, 0),
  net_invested NUMERIC,
  total_pnl NUMERIC,
  pnl_pct NUMERIC(10, 4)
) AS $$
BEGIN
  RETURN QUERY
  WITH
  position_buckets AS (
    SELECT
      time_bucket_gapfill(
        p_interval,
        pc.created_at,
        start => p_start_time,
        finish => p_end_time
      ) AS bucket,
      SUM(pc.shares_delta) AS shares_delta_period,
      SUM(pc.assets_in) AS assets_in_period,
      SUM(pc.assets_out) AS assets_out_period
    FROM position_change pc
    WHERE pc.account_id = p_account_id
      AND pc.term_id = p_term_id
      AND pc.curve_id = p_curve_id
      AND pc.created_at >= p_start_time
      AND pc.created_at <= p_end_time
    GROUP BY time_bucket_gapfill(
      p_interval,
      pc.created_at,
      start => p_start_time,
      finish => p_end_time
    )
  ),
  position_cumulative AS (
    SELECT
      bucket,
      SUM(shares_delta_period) OVER (ORDER BY bucket) AS shares_total,
      SUM(assets_in_period) OVER (ORDER BY bucket) AS total_assets_in,
      SUM(assets_out_period) OVER (ORDER BY bucket) AS total_assets_out
    FROM position_buckets
  ),
  price_buckets AS (
    SELECT
      time_bucket_gapfill(
        p_interval,
        spc.updated_at,
        start => p_start_time,
        finish => p_end_time
      ) AS bucket,
      locf(last(spc.share_price, spc.updated_at)) AS share_price
    FROM share_price_change spc
    WHERE spc.term_id = p_term_id
      AND spc.curve_id = p_curve_id
      AND spc.updated_at >= p_start_time
      AND spc.updated_at <= p_end_time
    GROUP BY time_bucket_gapfill(
      p_interval,
      spc.updated_at,
      start => p_start_time,
      finish => p_end_time
    )
  )
  SELECT
    COALESCE(pc.bucket, pr.bucket) AS time,
    locf(pc.shares_total) AS shares_total,
    pr.share_price,
    (locf(pc.shares_total) * pr.share_price / 1e18)::NUMERIC AS equity_value,
    locf(pc.total_assets_in) AS total_assets_in,
    locf(pc.total_assets_out) AS total_assets_out,
    (locf(pc.total_assets_in) - locf(pc.total_assets_out))::NUMERIC AS net_invested,
    ((locf(pc.shares_total) * pr.share_price / 1e18)
      + locf(pc.total_assets_out)
      - locf(pc.total_assets_in))::NUMERIC AS total_pnl,
    (((locf(pc.shares_total) * pr.share_price / 1e18)
      + locf(pc.total_assets_out)
      - locf(pc.total_assets_in)) * 100.0
      / GREATEST(locf(pc.total_assets_in) - locf(pc.total_assets_out), 1))::NUMERIC(10,4) AS pnl_pct
  FROM position_cumulative pc
  FULL OUTER JOIN price_buckets pr ON pc.bucket = pr.bucket
  ORDER BY time;
END;
$$ LANGUAGE plpgsql STABLE;

-- ========================================
-- POSITION CHANGE TRIGGERS
-- ========================================

CREATE OR REPLACE FUNCTION insert_position_change_from_deposit()
RETURNS TRIGGER AS $$
BEGIN
  INSERT INTO position_change (
    created_at,
    account_id,
    term_id,
    curve_id,
    shares_delta,
    assets_in,
    assets_out,
    event_type,
    event_id,
    block_number,
    transaction_hash,
    log_index
  ) VALUES (
    NEW.created_at,
    NEW.receiver_id,
    NEW.term_id,
    NEW.curve_id,
    NEW.shares,
    NEW.assets_after_fees,
    0,
    'deposit',
    NEW.id,
    NEW.block_number,
    NEW.transaction_hash,
    NEW.log_index
  );
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER deposit_position_change_trigger
AFTER INSERT ON deposit
FOR EACH ROW
EXECUTE FUNCTION insert_position_change_from_deposit();

CREATE OR REPLACE FUNCTION insert_position_change_from_redemption()
RETURNS TRIGGER AS $$
BEGIN
  INSERT INTO position_change (
    created_at,
    account_id,
    term_id,
    curve_id,
    shares_delta,
    assets_in,
    assets_out,
    event_type,
    event_id,
    block_number,
    transaction_hash,
    log_index
  ) VALUES (
    NEW.created_at,
    NEW.sender_id,
    NEW.term_id,
    NEW.curve_id,
    -NEW.shares,
    0,
    NEW.assets,
    'redemption',
    NEW.id,
    NEW.block_number,
    NEW.transaction_hash,
    NEW.log_index
  );
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER redemption_position_change_trigger
AFTER INSERT ON redemption
FOR EACH ROW
EXECUTE FUNCTION insert_position_change_from_redemption();
