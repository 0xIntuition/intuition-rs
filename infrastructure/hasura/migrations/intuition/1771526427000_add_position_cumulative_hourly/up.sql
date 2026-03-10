-- Create position_cumulative_hourly hypertable
--
-- Stores running totals of shares, deposits, and redemptions per
-- (account_id, term_id, curve_id) at hourly granularity.
-- Source: position_change_hourly continuous aggregate.

-- ========================================
-- Step 1: Create hypertable
-- ========================================
CREATE TABLE IF NOT EXISTS position_cumulative_hourly (
  bucket           TIMESTAMPTZ   NOT NULL,
  account_id       TEXT          NOT NULL,
  term_id          TEXT          NOT NULL,
  curve_id         NUMERIC       NOT NULL,
  cumulative_shares     NUMERIC  NOT NULL DEFAULT 0,
  cumulative_assets_in  NUMERIC  NOT NULL DEFAULT 0,
  cumulative_assets_out NUMERIC  NOT NULL DEFAULT 0,
  cumulative_shares_in  NUMERIC  NOT NULL DEFAULT 0,
  cumulative_shares_out NUMERIC  NOT NULL DEFAULT 0
);

SELECT create_hypertable(
  'position_cumulative_hourly', 'bucket',
  if_not_exists => true
);

-- ========================================
-- Step 2: Indexes for point-in-time lookups
-- ========================================
CREATE INDEX IF NOT EXISTS idx_pch_account_term_curve_bucket
  ON position_cumulative_hourly (account_id, term_id, curve_id, bucket DESC);

-- ========================================
-- Step 3: Backfill from position_change_hourly
-- ========================================
SET statement_timeout = '60min';

INSERT INTO position_cumulative_hourly (
  bucket, account_id, term_id, curve_id,
  cumulative_shares, cumulative_assets_in, cumulative_assets_out,
  cumulative_shares_in, cumulative_shares_out
)
SELECT
  bucket,
  account_id,
  term_id,
  curve_id,
  SUM(shares_delta_period)  OVER w AS cumulative_shares,
  SUM(assets_in_period)     OVER w AS cumulative_assets_in,
  SUM(assets_out_period)    OVER w AS cumulative_assets_out,
  SUM(shares_in_period)     OVER w AS cumulative_shares_in,
  SUM(shares_out_period)    OVER w AS cumulative_shares_out
FROM position_change_hourly
WINDOW w AS (
  PARTITION BY account_id, term_id, curve_id
  ORDER BY bucket
  ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
);

RESET statement_timeout;

-- ========================================
-- Step 4: Compression policy (30-day chunk)
-- ========================================
ALTER TABLE position_cumulative_hourly
  SET (timescaledb.compress,
       timescaledb.compress_segmentby = 'account_id, term_id, curve_id',
       timescaledb.compress_orderby = 'bucket DESC');

SELECT add_compression_policy('position_cumulative_hourly',
  compress_after => INTERVAL '30 days',
  if_not_exists => true);

-- ========================================
-- Step 5: Hourly refresh job
-- ========================================
-- Incrementally appends new rows from position_change_hourly.
-- Scheduled 5 minutes after the cagg refresh so fresh hourly data is available.
CREATE OR REPLACE FUNCTION refresh_position_cumulative_hourly(config JSONB)
RETURNS VOID LANGUAGE plpgsql AS $$
DECLARE
  v_last_bucket TIMESTAMPTZ;
BEGIN
  -- Find the latest bucket already materialized
  SELECT MAX(bucket) INTO v_last_bucket FROM position_cumulative_hourly;

  -- If the table is empty, nothing to do (backfill handles initial load)
  IF v_last_bucket IS NULL THEN
    RETURN;
  END IF;

  -- For each (account, term, curve) that has new hourly data after v_last_bucket,
  -- compute running totals starting from their latest existing cumulative row.
  INSERT INTO position_cumulative_hourly (
    bucket, account_id, term_id, curve_id,
    cumulative_shares, cumulative_assets_in, cumulative_assets_out,
    cumulative_shares_in, cumulative_shares_out
  )
  SELECT
    pch.bucket,
    pch.account_id,
    pch.term_id,
    pch.curve_id,
    prev.cumulative_shares     + SUM(pch.shares_delta_period) OVER w AS cumulative_shares,
    prev.cumulative_assets_in  + SUM(pch.assets_in_period)    OVER w AS cumulative_assets_in,
    prev.cumulative_assets_out + SUM(pch.assets_out_period)   OVER w AS cumulative_assets_out,
    prev.cumulative_shares_in  + SUM(pch.shares_in_period)    OVER w AS cumulative_shares_in,
    prev.cumulative_shares_out + SUM(pch.shares_out_period)   OVER w AS cumulative_shares_out
  FROM position_change_hourly pch
  JOIN LATERAL (
    SELECT
      cumulative_shares, cumulative_assets_in, cumulative_assets_out,
      cumulative_shares_in, cumulative_shares_out
    FROM position_cumulative_hourly existing
    WHERE existing.account_id = pch.account_id
      AND existing.term_id    = pch.term_id
      AND existing.curve_id   = pch.curve_id
    ORDER BY existing.bucket DESC
    LIMIT 1
  ) prev ON true
  WHERE pch.bucket > v_last_bucket
  WINDOW w AS (
    PARTITION BY pch.account_id, pch.term_id, pch.curve_id
    ORDER BY pch.bucket
    ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
  )
  ON CONFLICT DO NOTHING;
END;
$$;

SELECT add_job('refresh_position_cumulative_hourly',
  schedule_interval => INTERVAL '1 hour',
  initial_start     => now() + INTERVAL '65 minutes');
