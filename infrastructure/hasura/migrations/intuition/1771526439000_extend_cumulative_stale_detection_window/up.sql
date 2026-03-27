-- Extend the stale detection window in refresh_position_cumulative_hourly
-- from 24 hours to 7 days.
--
-- Problem: The CAGG end_offset race condition can create mid-stream gaps when
-- position_change_hourly materializes a bucket after the cumulative refresh has
-- already advanced past it. With a 24-hour lookback, gaps older than 24 hours
-- become permanent. With 7 days, the hourly CronJob has 168 chances to catch
-- any gap before it slips past the window — effectively impossible to miss.
--
-- Keeps the (config JSONB) signature for K8s CronJob compatibility.

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
  -- Phase 0: Global fast path — append new buckets beyond the watermark
  -- =====================================================================
  SELECT MAX(bucket) INTO v_global_last FROM position_cumulative_hourly;
  SELECT MAX(bucket) INTO v_cagg_last   FROM position_change_hourly;

  IF v_global_last IS NULL THEN
    RETURN;
  END IF;

  IF v_cagg_last IS NULL OR v_cagg_last <= v_global_last THEN
    RETURN;
  END IF;

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
      AND existing.bucket    <= v_global_last
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
  -- Phase 1: Detect stale and new positions (7-day lookback window)
  -- =====================================================================
  CREATE TEMP TABLE _tmp_stale_positions (
    account_id TEXT NOT NULL,
    term_id    TEXT NOT NULL,
    curve_id   NUMERIC NOT NULL,
    last_cum_bucket TIMESTAMPTZ
  ) ON COMMIT DROP;

  -- Stale: positions where the CAGG has a more recent bucket than cumulative
  INSERT INTO _tmp_stale_positions (account_id, term_id, curve_id, last_cum_bucket)
  SELECT
    pch_recent.account_id,
    pch_recent.term_id,
    pch_recent.curve_id,
    cum_last.last_cum_bucket
  FROM (
    SELECT account_id, term_id, curve_id, MAX(bucket) AS last_cagg_bucket
    FROM position_change_hourly
    WHERE bucket > v_global_last - interval '7 days'
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
  WHERE pch_recent.last_cagg_bucket > cum_last.last_cum_bucket;

  -- New: positions that exist in CAGG but have no cumulative row at all
  INSERT INTO _tmp_stale_positions (account_id, term_id, curve_id, last_cum_bucket)
  SELECT
    pch_new.account_id,
    pch_new.term_id,
    pch_new.curve_id,
    NULL
  FROM (
    SELECT DISTINCT account_id, term_id, curve_id
    FROM position_change_hourly
    WHERE bucket > v_global_last - interval '7 days'
  ) pch_new
  WHERE NOT EXISTS (
    SELECT 1
    FROM position_cumulative_hourly existing
    WHERE existing.account_id = pch_new.account_id
      AND existing.term_id    = pch_new.term_id
      AND existing.curve_id   = pch_new.curve_id
    LIMIT 1
  )
  AND NOT EXISTS (
    SELECT 1 FROM _tmp_stale_positions sp
    WHERE sp.account_id = pch_new.account_id
      AND sp.term_id    = pch_new.term_id
      AND sp.curve_id   = pch_new.curve_id
  );

  SELECT count(*) INTO v_stale_count FROM _tmp_stale_positions WHERE last_cum_bucket IS NOT NULL;
  SELECT count(*) INTO v_new_count   FROM _tmp_stale_positions WHERE last_cum_bucket IS NULL;

  IF v_stale_count = 0 AND v_new_count = 0 THEN
    RETURN;
  END IF;

  -- =====================================================================
  -- Phase 2: Process stale positions (fill gaps from last known state)
  -- =====================================================================
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
        AND existing.bucket    <= rec.last_cum_bucket
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

  -- =====================================================================
  -- Phase 3: Process new positions (no prior cumulative rows at all)
  -- =====================================================================
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
