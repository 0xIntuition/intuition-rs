-- Revert to the original refresh_position_cumulative_hourly from migration 1771526427000
-- (restores single global MAX(bucket) approach)

DROP FUNCTION IF EXISTS refresh_position_cumulative_hourly(JSONB);
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

-- Drop the unique index added by this migration
DROP INDEX IF EXISTS idx_pch_unique_position_bucket;

-- Note: The one-time backfill rows inserted by up.sql are correct data and
-- do not need to be removed. They fill gaps that the old function would
-- continue to miss, so leaving them in place is harmless.
