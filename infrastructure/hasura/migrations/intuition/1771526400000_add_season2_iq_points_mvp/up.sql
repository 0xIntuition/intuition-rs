-- Season 2 IQ Points MVP
--
-- Design choices:
-- - TRUST/USD snapshots every 12 hours at canonical UTC boundaries (00:00 and 12:00)
-- - Epoch settlement allowed after epoch end + 12 hours
-- - Freeze-once model with optional manual force re-settlement before finalization

-- ========================================
-- 1. SEASON 2 EPOCH CALENDAR
-- ========================================

CREATE TABLE IF NOT EXISTS season2_epoch (
  epoch INTEGER PRIMARY KEY,
  start_at TIMESTAMPTZ NOT NULL,
  end_at TIMESTAMPTZ NOT NULL,
  settle_after TIMESTAMPTZ NOT NULL,
  settled_at TIMESTAMPTZ,
  last_settlement_force BOOLEAN NOT NULL DEFAULT FALSE,
  is_final BOOLEAN NOT NULL DEFAULT FALSE,
  finalized_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT season2_epoch_time_range_check CHECK (start_at < end_at),
  CONSTRAINT season2_epoch_finalize_check CHECK (
    (is_final = FALSE AND finalized_at IS NULL)
    OR (is_final = TRUE AND finalized_at IS NOT NULL)
  )
);

-- Season 2 timeline from strategy doc:
-- - Starts at Epoch 8 on 2026-02-24
-- - Ends at start of Epoch 31 on 2027-01-12
-- - 23 epochs total (8..30), each 14 days
INSERT INTO season2_epoch (epoch, start_at, end_at, settle_after)
SELECT
  e AS epoch,
  (TIMESTAMPTZ '2026-02-24 00:00:00+00' + ((e - 8) * INTERVAL '14 days')) AS start_at,
  (TIMESTAMPTZ '2026-02-24 00:00:00+00' + ((e - 7) * INTERVAL '14 days')) AS end_at,
  (TIMESTAMPTZ '2026-02-24 00:00:00+00' + ((e - 7) * INTERVAL '14 days') + INTERVAL '12 hours') AS settle_after
FROM generate_series(8, 30) AS e
ON CONFLICT (epoch) DO NOTHING;

-- ========================================
-- 2. TRUST/USD SNAPSHOTS
-- ========================================

CREATE TABLE IF NOT EXISTS season2_trust_price_snapshot (
  snapshot_at TIMESTAMPTZ PRIMARY KEY,
  price_usd NUMERIC(20, 10) NOT NULL,
  source TEXT NOT NULL DEFAULT 'coingecko',
  source_ref TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT season2_trust_price_snapshot_positive_price CHECK (price_usd > 0)
);

CREATE INDEX IF NOT EXISTS idx_season2_trust_price_snapshot_time_desc
  ON season2_trust_price_snapshot (snapshot_at DESC);

CREATE OR REPLACE FUNCTION upsert_season2_trust_price_snapshot(
  p_snapshot_at TIMESTAMPTZ,
  p_price_usd NUMERIC(20, 10),
  p_source TEXT DEFAULT 'coingecko',
  p_source_ref TEXT DEFAULT NULL
)
RETURNS SETOF season2_trust_price_snapshot AS $$
BEGIN
  RETURN QUERY
  INSERT INTO season2_trust_price_snapshot (
    snapshot_at,
    price_usd,
    source,
    source_ref
  )
  VALUES (
    p_snapshot_at,
    p_price_usd,
    COALESCE(NULLIF(TRIM(p_source), ''), 'coingecko'),
    p_source_ref
  )
  ON CONFLICT (snapshot_at) DO UPDATE
  SET
    price_usd = EXCLUDED.price_usd,
    source = EXCLUDED.source,
    source_ref = EXCLUDED.source_ref,
    updated_at = NOW()
  RETURNING *;
END;
$$ LANGUAGE plpgsql VOLATILE;

-- ========================================
-- 3. LEADERBOARD PAYOUT CURVE
-- ========================================

CREATE TABLE IF NOT EXISTS season2_leaderboard_payout (
  rank INTEGER PRIMARY KEY,
  iq_points NUMERIC(30, 0) NOT NULL,
  CONSTRAINT season2_leaderboard_payout_rank_check CHECK (rank BETWEEN 1 AND 25),
  CONSTRAINT season2_leaderboard_payout_iq_check CHECK (iq_points > 0)
);

INSERT INTO season2_leaderboard_payout (rank, iq_points)
VALUES
  (1, 419600),
  (2, 335700),
  (3, 251700),
  (4, 230800),
  (5, 209800),
  (6, 188800),
  (7, 167800),
  (8, 146900),
  (9, 125900),
  (10, 104900),
  (11, 83900),
  (12, 79700),
  (13, 75500),
  (14, 71300),
  (15, 67100),
  (16, 62900),
  (17, 58700),
  (18, 54500),
  (19, 50300),
  (20, 46200),
  (21, 42000),
  (22, 37800),
  (23, 33600),
  (24, 29400),
  (25, 25200)
ON CONFLICT (rank) DO NOTHING;

-- ========================================
-- 4. EPOCH PRICE SNAPSHOT + IQ LEDGER
-- ========================================

CREATE TABLE IF NOT EXISTS season2_epoch_price (
  epoch INTEGER PRIMARY KEY REFERENCES season2_epoch(epoch) ON DELETE CASCADE,
  snapshot_count INTEGER NOT NULL,
  average_price_usd NUMERIC(20, 10) NOT NULL,
  computed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT season2_epoch_price_snapshot_count_check CHECK (snapshot_count > 0),
  CONSTRAINT season2_epoch_price_average_price_check CHECK (average_price_usd > 0)
);

CREATE TABLE IF NOT EXISTS season2_iq_ledger (
  id BIGSERIAL PRIMARY KEY,
  epoch INTEGER NOT NULL REFERENCES season2_epoch(epoch) ON DELETE RESTRICT,
  account_id TEXT NOT NULL REFERENCES account(id) ON DELETE RESTRICT,
  entry_type TEXT NOT NULL,
  source_id TEXT NOT NULL,
  iq_points NUMERIC(30, 0) NOT NULL,
  metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT season2_iq_ledger_entry_type_check CHECK (
    entry_type IN ('fee', 'leaderboard_pnl', 'leaderboard_roi')
  ),
  CONSTRAINT season2_iq_ledger_iq_points_check CHECK (iq_points >= 0),
  CONSTRAINT season2_iq_ledger_epoch_entry_source_unique UNIQUE (epoch, entry_type, source_id)
);

CREATE INDEX IF NOT EXISTS idx_season2_iq_ledger_account_epoch
  ON season2_iq_ledger (account_id, epoch);

CREATE INDEX IF NOT EXISTS idx_season2_iq_ledger_epoch
  ON season2_iq_ledger (epoch);

-- ========================================
-- 5. READ FUNCTION FOR ACCOUNT BREAKDOWN
-- ========================================

CREATE OR REPLACE FUNCTION get_season2_iq_account_breakdown(
  p_account_id TEXT
)
RETURNS TABLE (
  epoch INTEGER,
  start_at TIMESTAMPTZ,
  end_at TIMESTAMPTZ,
  is_final BOOLEAN,
  fee_iq NUMERIC,
  leaderboard_pnl_iq NUMERIC,
  leaderboard_roi_iq NUMERIC,
  total_iq NUMERIC
) AS $$
  SELECT
    e.epoch,
    e.start_at,
    e.end_at,
    e.is_final,
    COALESCE(SUM(l.iq_points) FILTER (WHERE l.entry_type = 'fee'), 0)::NUMERIC AS fee_iq,
    COALESCE(SUM(l.iq_points) FILTER (WHERE l.entry_type = 'leaderboard_pnl'), 0)::NUMERIC AS leaderboard_pnl_iq,
    COALESCE(SUM(l.iq_points) FILTER (WHERE l.entry_type = 'leaderboard_roi'), 0)::NUMERIC AS leaderboard_roi_iq,
    COALESCE(SUM(l.iq_points), 0)::NUMERIC AS total_iq
  FROM season2_epoch e
  LEFT JOIN season2_iq_ledger l
    ON l.epoch = e.epoch
   AND l.account_id = p_account_id
  GROUP BY e.epoch, e.start_at, e.end_at, e.is_final
  HAVING COALESCE(SUM(l.iq_points), 0) > 0
  ORDER BY e.epoch ASC;
$$ LANGUAGE sql STABLE;

-- ========================================
-- 6. SETTLEMENT FUNCTIONS
-- ========================================

CREATE OR REPLACE FUNCTION settle_season2_epoch(
  p_epoch INTEGER,
  p_force BOOLEAN DEFAULT FALSE
)
RETURNS TABLE (
  epoch INTEGER,
  snapshot_count INTEGER,
  average_trust_usd NUMERIC(20, 10),
  fee_entries_inserted BIGINT,
  pnl_entries_inserted BIGINT,
  roi_entries_inserted BIGINT,
  fee_iq_total NUMERIC(30, 0),
  pnl_iq_total NUMERIC(30, 0),
  roi_iq_total NUMERIC(30, 0),
  total_iq NUMERIC(30, 0)
) AS $$
DECLARE
  v_start_at TIMESTAMPTZ;
  v_end_at TIMESTAMPTZ;
  v_settle_after TIMESTAMPTZ;
  v_is_final BOOLEAN;
  v_snapshot_count INTEGER;
  v_avg_price NUMERIC(20, 10);

  v_fee_entries_inserted BIGINT := 0;
  v_pnl_entries_inserted BIGINT := 0;
  v_roi_entries_inserted BIGINT := 0;

  v_fee_iq_total NUMERIC(30, 0) := 0;
  v_pnl_iq_total NUMERIC(30, 0) := 0;
  v_roi_iq_total NUMERIC(30, 0) := 0;
BEGIN
  SELECT
    se.start_at,
    se.end_at,
    se.settle_after,
    se.is_final
  INTO
    v_start_at,
    v_end_at,
    v_settle_after,
    v_is_final
  FROM season2_epoch se
  WHERE se.epoch = p_epoch;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'Season 2 epoch % not found', p_epoch;
  END IF;

  IF v_is_final THEN
    RAISE EXCEPTION 'Season 2 epoch % is already finalized', p_epoch;
  END IF;

  IF NOW() < v_settle_after THEN
    RAISE EXCEPTION
      'Season 2 epoch % cannot be settled before settle_after (%)',
      p_epoch,
      v_settle_after;
  END IF;

  SELECT
    COUNT(*)::INTEGER,
    AVG(stps.price_usd)::NUMERIC(20, 10)
  INTO
    v_snapshot_count,
    v_avg_price
  FROM season2_trust_price_snapshot stps
  WHERE stps.snapshot_at >= v_start_at
    AND stps.snapshot_at < v_end_at;

  IF v_snapshot_count = 0 OR v_avg_price IS NULL THEN
    RAISE EXCEPTION 'No TRUST/USD snapshots found for Season 2 epoch %', p_epoch;
  END IF;

  INSERT INTO season2_epoch_price (
    epoch,
    snapshot_count,
    average_price_usd,
    computed_at,
    updated_at
  )
  VALUES (
    p_epoch,
    v_snapshot_count,
    v_avg_price,
    NOW(),
    NOW()
  )
  ON CONFLICT (epoch) DO UPDATE
  SET
    snapshot_count = EXCLUDED.snapshot_count,
    average_price_usd = EXCLUDED.average_price_usd,
    computed_at = EXCLUDED.computed_at,
    updated_at = NOW();

  IF p_force THEN
    DELETE FROM season2_iq_ledger
    WHERE epoch = p_epoch;
  END IF;

  -- Fee IQ:  amount(wei TRUST) -> TRUST -> USD via epoch avg -> IQ at 2000/$1
  WITH inserted_fee AS (
    INSERT INTO season2_iq_ledger (
      epoch,
      account_id,
      entry_type,
      source_id,
      iq_points,
      metadata
    )
    SELECT
      p_epoch,
      pfa.sender_id,
      'fee',
      pfa.id,
      ROUND((((pfa.amount / 1000000000000000000::NUMERIC) * v_avg_price) * 2000), 0)::NUMERIC(30, 0),
      jsonb_build_object(
        'fee_amount_raw', pfa.amount,
        'average_trust_usd', v_avg_price,
        'formula', '(amount_trust * average_trust_usd) * 2000'
      )
    FROM protocol_fee_accrued pfa
    WHERE pfa.epoch = p_epoch::NUMERIC
    ON CONFLICT (epoch, entry_type, source_id) DO NOTHING
    RETURNING iq_points
  )
  SELECT
    COUNT(*)::BIGINT,
    COALESCE(SUM(iq_points), 0)::NUMERIC(30, 0)
  INTO
    v_fee_entries_inserted,
    v_fee_iq_total
  FROM inserted_fee;

  -- Nominal PnL leaderboard IQ
  WITH inserted_pnl AS (
    INSERT INTO season2_iq_ledger (
      epoch,
      account_id,
      entry_type,
      source_id,
      iq_points,
      metadata
    )
    SELECT
      p_epoch,
      lb.account_id,
      'leaderboard_pnl',
      lb.account_id,
      slp.iq_points,
      jsonb_build_object(
        'rank', lb.rank,
        'leaderboard', 'pnl',
        'sort_by', 'total_pnl'
      )
    FROM get_pnl_leaderboard_period(
      v_start_at,
      v_end_at,
      25,
      0,
      'total_pnl',
      'DESC',
      TRUE,
      1,
      0,
      NULL
    ) lb
    JOIN season2_leaderboard_payout slp
      ON slp.rank = lb.rank::INTEGER
    ON CONFLICT (epoch, entry_type, source_id) DO NOTHING
    RETURNING iq_points
  )
  SELECT
    COUNT(*)::BIGINT,
    COALESCE(SUM(iq_points), 0)::NUMERIC(30, 0)
  INTO
    v_pnl_entries_inserted,
    v_pnl_iq_total
  FROM inserted_pnl;

  -- ROI leaderboard IQ
  WITH inserted_roi AS (
    INSERT INTO season2_iq_ledger (
      epoch,
      account_id,
      entry_type,
      source_id,
      iq_points,
      metadata
    )
    SELECT
      p_epoch,
      lb.account_id,
      'leaderboard_roi',
      lb.account_id,
      slp.iq_points,
      jsonb_build_object(
        'rank', lb.rank,
        'leaderboard', 'roi',
        'sort_by', 'pnl_pct'
      )
    FROM get_pnl_leaderboard_period(
      v_start_at,
      v_end_at,
      25,
      0,
      'pnl_pct',
      'DESC',
      TRUE,
      1,
      0,
      NULL
    ) lb
    JOIN season2_leaderboard_payout slp
      ON slp.rank = lb.rank::INTEGER
    ON CONFLICT (epoch, entry_type, source_id) DO NOTHING
    RETURNING iq_points
  )
  SELECT
    COUNT(*)::BIGINT,
    COALESCE(SUM(iq_points), 0)::NUMERIC(30, 0)
  INTO
    v_roi_entries_inserted,
    v_roi_iq_total
  FROM inserted_roi;

  UPDATE season2_epoch
  SET
    settled_at = NOW(),
    last_settlement_force = p_force,
    updated_at = NOW()
  WHERE season2_epoch.epoch = p_epoch;

  SELECT
    COALESCE(SUM(sil.iq_points) FILTER (WHERE sil.entry_type = 'fee'), 0)::NUMERIC(30, 0),
    COALESCE(SUM(sil.iq_points) FILTER (WHERE sil.entry_type = 'leaderboard_pnl'), 0)::NUMERIC(30, 0),
    COALESCE(SUM(sil.iq_points) FILTER (WHERE sil.entry_type = 'leaderboard_roi'), 0)::NUMERIC(30, 0)
  INTO
    v_fee_iq_total,
    v_pnl_iq_total,
    v_roi_iq_total
  FROM season2_iq_ledger sil
  WHERE sil.epoch = p_epoch;

  RETURN QUERY
  SELECT
    p_epoch,
    v_snapshot_count,
    v_avg_price,
    v_fee_entries_inserted,
    v_pnl_entries_inserted,
    v_roi_entries_inserted,
    COALESCE(v_fee_iq_total, 0)::NUMERIC(30, 0),
    COALESCE(v_pnl_iq_total, 0)::NUMERIC(30, 0),
    COALESCE(v_roi_iq_total, 0)::NUMERIC(30, 0),
    (COALESCE(v_fee_iq_total, 0) + COALESCE(v_pnl_iq_total, 0) + COALESCE(v_roi_iq_total, 0))::NUMERIC(30, 0);
END;
$$ LANGUAGE plpgsql VOLATILE;

CREATE OR REPLACE FUNCTION finalize_season2_epoch(
  p_epoch INTEGER
)
RETURNS TABLE (
  epoch INTEGER,
  is_final BOOLEAN,
  settled_at TIMESTAMPTZ,
  finalized_at TIMESTAMPTZ
) AS $$
DECLARE
  v_settled_at TIMESTAMPTZ;
  v_is_final BOOLEAN;
BEGIN
  SELECT
    se.settled_at,
    se.is_final
  INTO
    v_settled_at,
    v_is_final
  FROM season2_epoch se
  WHERE se.epoch = p_epoch
  FOR UPDATE;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'Season 2 epoch % not found', p_epoch;
  END IF;

  IF v_settled_at IS NULL THEN
    RAISE EXCEPTION 'Season 2 epoch % must be settled before finalization', p_epoch;
  END IF;

  IF NOT v_is_final THEN
    UPDATE season2_epoch
    SET
      is_final = TRUE,
      finalized_at = NOW(),
      updated_at = NOW()
    WHERE season2_epoch.epoch = p_epoch;
  END IF;

  RETURN QUERY
  SELECT
    se.epoch,
    se.is_final,
    se.settled_at,
    se.finalized_at
  FROM season2_epoch se
  WHERE se.epoch = p_epoch;
END;
$$ LANGUAGE plpgsql VOLATILE;
