-- Migration: Backfill position_cumulative_hourly gaps and clean up broken TimescaleDB job
--
-- Context: The refresh_position_cumulative_hourly function was created with signature
-- (config JSONB) but TimescaleDB's scheduler calls (job_id INTEGER, config JSONB).
-- Result: 188 consecutive failures, 0 successes. 288+ missing cumulative rows.
--
-- The refresh is now handled by a K8s CronJob (position-cumulative-refresh) that
-- calls refresh_position_cumulative_hourly(NULL::JSONB) directly. We keep the
-- existing (config JSONB) signature to maintain CronJob compatibility.
--
-- This migration:
--   1. Backfills trailing gaps (positions where MAX(hourly) > MAX(cumulative))
--   2. Fixes mid-stream gaps (missing buckets in the middle of a position's history)
--   3. Deletes the broken TimescaleDB job (CronJob handles scheduling)

SET statement_timeout = '10min';

-- ========================================
-- Step 1: Backfill trailing gaps
-- ========================================
-- Catches positions where cumulative is behind hourly (most common case).

WITH stale_keys AS (
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
  WHERE cum_last.last_cum_bucket IS NULL
     OR pch_cagg.last_cagg_bucket > cum_last.last_cum_bucket
),
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

-- ========================================
-- Step 2: Fix mid-stream gaps
-- ========================================
-- Catches buckets missing in the middle of a position's history (e.g., 14:00 missing
-- but 15:00 exists with wrong running totals). Step 1 won't fix these because
-- MAX(hourly) == MAX(cumulative) for these positions.
--
-- Wrapped in DO $$ block because Hasura runs each top-level statement in a separate
-- transaction — temp tables would not survive between statements.
DO $$
BEGIN
  -- 2a: Find positions with mid-stream gaps
  CREATE TEMP TABLE _tmp_gap_positions ON COMMIT DROP AS
  SELECT DISTINCT pch.account_id, pch.term_id, pch.curve_id,
    MIN(pch.bucket) AS first_gap_bucket
  FROM position_change_hourly pch
  LEFT JOIN position_cumulative_hourly pc
    ON pch.account_id = pc.account_id AND pch.term_id = pc.term_id
    AND pch.curve_id = pc.curve_id AND pch.bucket = pc.bucket
  WHERE pc.account_id IS NULL
  GROUP BY pch.account_id, pch.term_id, pch.curve_id;

  -- Skip if no gaps found
  IF NOT EXISTS (SELECT 1 FROM _tmp_gap_positions) THEN
    RETURN;
  END IF;

  -- Allow deletes on compressed chunks
  SET LOCAL timescaledb.max_tuples_decompressed_per_dml_transaction = 0;

  -- 2b: Delete cumulative rows from the first gap onwards (they have wrong running totals)
  DELETE FROM position_cumulative_hourly pc
  USING _tmp_gap_positions gp
  WHERE pc.account_id = gp.account_id
    AND pc.term_id = gp.term_id
    AND pc.curve_id = gp.curve_id
    AND pc.bucket >= gp.first_gap_bucket;

  -- 2c: Get the base state just before the first gap for each position
  CREATE TEMP TABLE _tmp_base_state ON COMMIT DROP AS
  SELECT
    gp.account_id, gp.term_id, gp.curve_id,
    cum.last_bucket,
    COALESCE(cum.cumulative_shares, 0) AS base_shares,
    COALESCE(cum.cumulative_assets_in, 0) AS base_assets_in,
    COALESCE(cum.cumulative_assets_out, 0) AS base_assets_out,
    COALESCE(cum.cumulative_shares_in, 0) AS base_shares_in,
    COALESCE(cum.cumulative_shares_out, 0) AS base_shares_out
  FROM _tmp_gap_positions gp
  LEFT JOIN LATERAL (
    SELECT bucket AS last_bucket,
      cumulative_shares, cumulative_assets_in, cumulative_assets_out,
      cumulative_shares_in, cumulative_shares_out
    FROM position_cumulative_hourly
    WHERE account_id = gp.account_id AND term_id = gp.term_id AND curve_id = gp.curve_id
      AND bucket < gp.first_gap_bucket
    ORDER BY bucket DESC LIMIT 1
  ) cum ON true;

  -- 2d: Rebuild from base state forward using position_change_hourly
  INSERT INTO position_cumulative_hourly (
    bucket, account_id, term_id, curve_id,
    cumulative_shares, cumulative_assets_in, cumulative_assets_out,
    cumulative_shares_in, cumulative_shares_out
  )
  SELECT
    cagg.bucket, cagg.account_id, cagg.term_id, cagg.curve_id,
    bs.base_shares + SUM(cagg.shares_delta_period) OVER w,
    bs.base_assets_in + SUM(cagg.assets_in_period) OVER w,
    bs.base_assets_out + SUM(cagg.assets_out_period) OVER w,
    bs.base_shares_in + SUM(cagg.shares_in_period) OVER w,
    bs.base_shares_out + SUM(cagg.shares_out_period) OVER w
  FROM _tmp_base_state bs
  JOIN position_change_hourly cagg
    ON cagg.account_id = bs.account_id AND cagg.term_id = bs.term_id AND cagg.curve_id = bs.curve_id
    AND (bs.last_bucket IS NULL OR cagg.bucket > bs.last_bucket)
  WINDOW w AS (
    PARTITION BY cagg.account_id, cagg.term_id, cagg.curve_id
    ORDER BY cagg.bucket ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
  )
  ON CONFLICT (account_id, term_id, curve_id, bucket) DO UPDATE SET
    cumulative_shares = EXCLUDED.cumulative_shares,
    cumulative_assets_in = EXCLUDED.cumulative_assets_in,
    cumulative_assets_out = EXCLUDED.cumulative_assets_out,
    cumulative_shares_in = EXCLUDED.cumulative_shares_in,
    cumulative_shares_out = EXCLUDED.cumulative_shares_out;
END $$;

RESET statement_timeout;

-- ========================================
-- Step 3: Delete broken TimescaleDB job
-- ========================================
-- The K8s CronJob (position-cumulative-refresh) handles scheduling now.
-- Delete the broken TimescaleDB job that has 188 failures / 0 successes.
DO $$
DECLARE
  v_job_id INTEGER;
BEGIN
  FOR v_job_id IN
    SELECT job_id FROM timescaledb_information.jobs
    WHERE proc_name = 'refresh_position_cumulative_hourly'
  LOOP
    PERFORM delete_job(v_job_id);
  END LOOP;
END $$;
