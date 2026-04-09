-- Migration: Fix per-position staleness detection in refresh_position_cumulative_hourly
--
-- Bug: The function used a single global MAX(bucket) to filter new rows from
-- position_change_hourly. Positions that went inactive and then resumed had their
-- new rows skipped because they fell before the global max. This caused stale
-- leaderboard data and incorrect PnL calculations for 39+ positions.
--
-- Fix: Three-phase approach:
--   Phase 0: Global fast path (preserves current 1-5s performance)
--   Phase 1: Detect stale + new positions (only when new data exists)
--   Phase 2: Process stale positions with per-position time filters

-- ========================================
-- Step 0: Add unique constraint for ON CONFLICT support
-- ========================================
SET statement_timeout = '10min';

CREATE UNIQUE INDEX IF NOT EXISTS idx_pch_unique_position_bucket
  ON position_cumulative_hourly (account_id, term_id, curve_id, bucket);

-- ========================================
-- Step 1: One-time backfill of stale and missing positions
-- ========================================

-- Backfill all positions that are stale or missing entirely from position_cumulative_hourly.
-- Uses ON CONFLICT DO UPDATE to authoritatively overwrite any previously-incorrect rows.
WITH stale_keys AS (
  -- Find positions where cagg has newer data than cumulative
  SELECT
    pch_cagg.account_id,
    pch_cagg.term_id,
    pch_cagg.curve_id,
    cum_last.last_cum_bucket
  FROM (
    SELECT account_id, term_id, curve_id, MAX(bucket) AS last_cagg_bucket
    FROM position_change_hourly
    GROUP BY account_id, term_id, curve_id
  ) pch_cagg
  LEFT JOIN LATERAL (
    SELECT bucket AS last_cum_bucket
    FROM position_cumulative_hourly existing
    WHERE existing.account_id = pch_cagg.account_id
      AND existing.term_id    = pch_cagg.term_id
      AND existing.curve_id   = pch_cagg.curve_id
    ORDER BY existing.bucket DESC
    LIMIT 1
  ) cum_last ON true
  WHERE cum_last.last_cum_bucket IS NULL                                     -- missing entirely
     OR pch_cagg.last_cagg_bucket > cum_last.last_cum_bucket + interval '1 hour'  -- behind
),
-- For positions with existing cumulative rows, get their last cumulative values
existing_last AS (
  SELECT
    sk.account_id,
    sk.term_id,
    sk.curve_id,
    sk.last_cum_bucket,
    COALESCE(pch.cumulative_shares, 0) AS base_shares,
    COALESCE(pch.cumulative_assets_in, 0) AS base_assets_in,
    COALESCE(pch.cumulative_assets_out, 0) AS base_assets_out,
    COALESCE(pch.cumulative_shares_in, 0) AS base_shares_in,
    COALESCE(pch.cumulative_shares_out, 0) AS base_shares_out
  FROM stale_keys sk
  LEFT JOIN position_cumulative_hourly pch
    ON pch.account_id = sk.account_id
   AND pch.term_id    = sk.term_id
   AND pch.curve_id   = sk.curve_id
   AND pch.bucket     = sk.last_cum_bucket
),
-- Compute cumulative rows for missing buckets
new_rows AS (
  SELECT
    cagg.bucket,
    cagg.account_id,
    cagg.term_id,
    cagg.curve_id,
    el.base_shares + SUM(cagg.shares_delta_period) OVER w AS cumulative_shares,
    el.base_assets_in + SUM(cagg.assets_in_period) OVER w AS cumulative_assets_in,
    el.base_assets_out + SUM(cagg.assets_out_period) OVER w AS cumulative_assets_out,
    el.base_shares_in + SUM(cagg.shares_in_period) OVER w AS cumulative_shares_in,
    el.base_shares_out + SUM(cagg.shares_out_period) OVER w AS cumulative_shares_out
  FROM existing_last el
  JOIN position_change_hourly cagg
    ON cagg.account_id = el.account_id
   AND cagg.term_id    = el.term_id
   AND cagg.curve_id   = el.curve_id
   AND (el.last_cum_bucket IS NULL OR cagg.bucket > el.last_cum_bucket)
  WINDOW w AS (
    PARTITION BY cagg.account_id, cagg.term_id, cagg.curve_id
    ORDER BY cagg.bucket
    ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
  )
)
INSERT INTO position_cumulative_hourly (
  bucket, account_id, term_id, curve_id,
  cumulative_shares, cumulative_assets_in, cumulative_assets_out,
  cumulative_shares_in, cumulative_shares_out
)
SELECT
  bucket, account_id, term_id, curve_id,
  cumulative_shares, cumulative_assets_in, cumulative_assets_out,
  cumulative_shares_in, cumulative_shares_out
FROM new_rows
ON CONFLICT (account_id, term_id, curve_id, bucket) DO UPDATE SET
  cumulative_shares     = EXCLUDED.cumulative_shares,
  cumulative_assets_in  = EXCLUDED.cumulative_assets_in,
  cumulative_assets_out = EXCLUDED.cumulative_assets_out,
  cumulative_shares_in  = EXCLUDED.cumulative_shares_in,
  cumulative_shares_out = EXCLUDED.cumulative_shares_out;

RESET statement_timeout;

-- ========================================
-- Step 2: Replace the refresh function
-- ========================================
DROP FUNCTION IF EXISTS refresh_position_cumulative_hourly(JSONB);
CREATE OR REPLACE FUNCTION refresh_position_cumulative_hourly(config JSONB)
RETURNS VOID LANGUAGE plpgsql AS $$
DECLARE
  v_global_last  TIMESTAMPTZ;
  v_cagg_last    TIMESTAMPTZ;
  v_stale_count  INTEGER;
  v_new_count    INTEGER;
  rec            RECORD;
BEGIN
  SET LOCAL work_mem = '64MB';

  -- =====================================================================
  -- Phase 0: Global fast path
  -- =====================================================================
  SELECT MAX(bucket) INTO v_global_last FROM position_cumulative_hourly;
  SELECT MAX(bucket) INTO v_cagg_last   FROM position_change_hourly;

  -- If the cumulative table is empty, nothing to do (initial backfill handles this)
  IF v_global_last IS NULL THEN
    RETURN;
  END IF;

  -- If no new cagg data at all, nothing to do
  IF v_cagg_last IS NULL OR v_cagg_last <= v_global_last THEN
    RETURN;
  END IF;

  -- Process the global batch: all rows after the global max (same as original function).
  -- This handles the common case where all positions advance in lockstep.
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
    prev.cumulative_shares     + SUM(pch.shares_delta_period)  OVER w AS cumulative_shares,
    prev.cumulative_assets_in  + SUM(pch.assets_in_period)     OVER w AS cumulative_assets_in,
    prev.cumulative_assets_out + SUM(pch.assets_out_period)    OVER w AS cumulative_assets_out,
    prev.cumulative_shares_in  + SUM(pch.shares_in_period)     OVER w AS cumulative_shares_in,
    prev.cumulative_shares_out + SUM(pch.shares_out_period)    OVER w AS cumulative_shares_out
  FROM position_change_hourly pch
  JOIN LATERAL (
    SELECT
      cumulative_shares, cumulative_assets_in, cumulative_assets_out,
      cumulative_shares_in, cumulative_shares_out
    FROM position_cumulative_hourly existing
    WHERE existing.account_id = pch.account_id
      AND existing.term_id    = pch.term_id
      AND existing.curve_id   = pch.curve_id
      AND existing.bucket    <= v_global_last  -- upper bound: prevent partial-run corruption
    ORDER BY existing.bucket DESC
    LIMIT 1
  ) prev ON true
  WHERE pch.bucket > v_global_last
  WINDOW w AS (
    PARTITION BY pch.account_id, pch.term_id, pch.curve_id
    ORDER BY pch.bucket
    ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
  )
  ON CONFLICT DO NOTHING;

  -- =====================================================================
  -- Phase 1: Detect stale and new positions
  -- Only positions with recent cagg activity could be stale.
  -- We look at positions that had activity in the window around the global max,
  -- then check if they're behind.
  -- =====================================================================
  CREATE TEMP TABLE _tmp_stale_positions (
    account_id TEXT NOT NULL,
    term_id    TEXT NOT NULL,
    curve_id   NUMERIC NOT NULL,
    last_cum_bucket TIMESTAMPTZ  -- NULL means brand-new position
  ) ON COMMIT DROP;

  -- Find gap-resume positions: those with cagg data newer than their individual
  -- last cumulative bucket, but that data was at or before the old global max
  -- (so Phase 0 skipped it). We check positions that had cagg activity in a
  -- window up to v_global_last (these are candidates the global filter missed).
  INSERT INTO _tmp_stale_positions (account_id, term_id, curve_id, last_cum_bucket)
  SELECT
    pch_recent.account_id,
    pch_recent.term_id,
    pch_recent.curve_id,
    cum_last.last_cum_bucket
  FROM (
    -- Positions with cagg activity in recent window up to the old global max.
    -- Use a 24-hour lookback to catch positions that resumed recently.
    SELECT account_id, term_id, curve_id, MAX(bucket) AS last_cagg_bucket
    FROM position_change_hourly
    WHERE bucket > v_global_last - interval '24 hours'
      AND bucket <= v_global_last
    GROUP BY account_id, term_id, curve_id
  ) pch_recent
  JOIN LATERAL (
    SELECT bucket AS last_cum_bucket
    FROM position_cumulative_hourly existing
    WHERE existing.account_id = pch_recent.account_id
      AND existing.term_id    = pch_recent.term_id
      AND existing.curve_id   = pch_recent.curve_id
    ORDER BY existing.bucket DESC
    LIMIT 1
  ) cum_last ON true
  -- Only stale if the position's cagg max is ahead of its cumulative max
  WHERE pch_recent.last_cagg_bucket > cum_last.last_cum_bucket;

  -- Also find positions that have cagg rows but ZERO cumulative rows (brand new).
  -- Only check positions with recent activity to avoid full table scan.
  INSERT INTO _tmp_stale_positions (account_id, term_id, curve_id, last_cum_bucket)
  SELECT
    pch_new.account_id,
    pch_new.term_id,
    pch_new.curve_id,
    NULL
  FROM (
    SELECT DISTINCT account_id, term_id, curve_id
    FROM position_change_hourly
    WHERE bucket > v_global_last - interval '24 hours'
  ) pch_new
  WHERE NOT EXISTS (
    SELECT 1
    FROM position_cumulative_hourly existing
    WHERE existing.account_id = pch_new.account_id
      AND existing.term_id    = pch_new.term_id
      AND existing.curve_id   = pch_new.curve_id
    LIMIT 1
  )
  -- Don't re-insert positions already found above
  AND NOT EXISTS (
    SELECT 1 FROM _tmp_stale_positions sp
    WHERE sp.account_id = pch_new.account_id
      AND sp.term_id    = pch_new.term_id
      AND sp.curve_id   = pch_new.curve_id
  );

  -- Count stale vs new
  SELECT count(*) INTO v_stale_count FROM _tmp_stale_positions WHERE last_cum_bucket IS NOT NULL;
  SELECT count(*) INTO v_new_count   FROM _tmp_stale_positions WHERE last_cum_bucket IS NULL;

  -- If nothing to do, we're done
  IF v_stale_count = 0 AND v_new_count = 0 THEN
    RETURN;
  END IF;

  -- =====================================================================
  -- Phase 2: Process stale positions
  -- =====================================================================

  -- Phase 2a: Gap-resume positions (loop for per-position chunk exclusion)
  FOR rec IN
    SELECT * FROM _tmp_stale_positions WHERE last_cum_bucket IS NOT NULL
  LOOP
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
      prev.cumulative_shares     + SUM(pch.shares_delta_period)  OVER w,
      prev.cumulative_assets_in  + SUM(pch.assets_in_period)     OVER w,
      prev.cumulative_assets_out + SUM(pch.assets_out_period)    OVER w,
      prev.cumulative_shares_in  + SUM(pch.shares_in_period)     OVER w,
      prev.cumulative_shares_out + SUM(pch.shares_out_period)    OVER w
    FROM position_change_hourly pch
    JOIN LATERAL (
      SELECT
        cumulative_shares, cumulative_assets_in, cumulative_assets_out,
        cumulative_shares_in, cumulative_shares_out
      FROM position_cumulative_hourly existing
      WHERE existing.account_id = pch.account_id
        AND existing.term_id    = pch.term_id
        AND existing.curve_id   = pch.curve_id
        AND existing.bucket    <= rec.last_cum_bucket  -- explicit upper bound
      ORDER BY existing.bucket DESC
      LIMIT 1
    ) prev ON true
    WHERE pch.account_id = rec.account_id
      AND pch.term_id    = rec.term_id
      AND pch.curve_id   = rec.curve_id
      AND pch.bucket     > rec.last_cum_bucket
    WINDOW w AS (
      PARTITION BY pch.account_id, pch.term_id, pch.curve_id
      ORDER BY pch.bucket
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    )
    ON CONFLICT DO NOTHING;
  END LOOP;

  -- Phase 2b: New positions (batch, max 100 per run)
  -- Bootstrap with full cumulative history using window functions (same as original backfill)
  IF v_new_count > 0 THEN
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
      SUM(pch.shares_delta_period)  OVER w AS cumulative_shares,
      SUM(pch.assets_in_period)     OVER w AS cumulative_assets_in,
      SUM(pch.assets_out_period)    OVER w AS cumulative_assets_out,
      SUM(pch.shares_in_period)     OVER w AS cumulative_shares_in,
      SUM(pch.shares_out_period)    OVER w AS cumulative_shares_out
    FROM (
      SELECT account_id, term_id, curve_id
      FROM _tmp_stale_positions
      WHERE last_cum_bucket IS NULL
      ORDER BY account_id, term_id, curve_id
      LIMIT 100
    ) new_pos
    JOIN position_change_hourly pch
      ON pch.account_id = new_pos.account_id
     AND pch.term_id    = new_pos.term_id
     AND pch.curve_id   = new_pos.curve_id
    WINDOW w AS (
      PARTITION BY pch.account_id, pch.term_id, pch.curve_id
      ORDER BY pch.bucket
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    )
    ON CONFLICT DO NOTHING;
  END IF;
END;
$$;
