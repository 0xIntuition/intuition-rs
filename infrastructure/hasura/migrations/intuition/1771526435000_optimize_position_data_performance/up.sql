-- Optimize _tmp_position_data in get_pnl_leaderboard_period.
--
-- Problem: The DISTINCT ON join for snap_end scans all history (8.5s),
-- and the LATERAL for snap_start fires 67K times (~22s). Total: ~31s.
--
-- Fix: LATERAL per ACCOUNT (149 calls) instead of per POSITION (67K).
-- Each LATERAL does two DISTINCT ON queries scoped to one account_id,
-- which the compressed index (account_id, term_id, curve_id, bucket DESC)
-- can serve with a narrow index seek instead of a full-history scan.
--
-- Verified: 19.3x speedup (14.7s → 764ms), zero data mismatches across 67,638 rows.
--
-- DEPENDS ON:
--   1771526434000_epoch_realized_pnl_only_deployed_capital
--
-- SCOPE:
--   Modified:  get_pnl_leaderboard_period (only _tmp_position_data step)
--   Unchanged: get_pnl_leaderboard, get_vault_leaderboard, get_vault_leaderboard_period

CREATE OR REPLACE FUNCTION get_pnl_leaderboard_period(
  p_start_date TIMESTAMPTZ,
  p_end_date TIMESTAMPTZ,
  p_limit INTEGER DEFAULT 100,
  p_offset INTEGER DEFAULT 0,
  p_sort_by TEXT DEFAULT 'total_pnl',
  p_sort_order TEXT DEFAULT 'DESC',
  p_exclude_protocol_accounts BOOLEAN DEFAULT TRUE,
  p_min_positions INTEGER DEFAULT 1,
  p_min_volume NUMERIC DEFAULT 0,
  p_term_id TEXT DEFAULT NULL,
  p_min_deposit NUMERIC DEFAULT 0
)
RETURNS SETOF pnl_leaderboard_entry AS $$
DECLARE
  v_start_timestamp      BIGINT;
  v_end_timestamp        BIGINT;
  v_bucket_start         TIMESTAMPTZ;
  v_bucket_end           TIMESTAMPTZ;
  v_limit                INTEGER;
  v_offset               INTEGER;
  v_cagg_cutoff          TIMESTAMPTZ;
BEGIN
  IF p_start_date IS NULL OR p_end_date IS NULL THEN
    RAISE EXCEPTION 'p_start_date and p_end_date are required';
  END IF;

  IF p_start_date >= p_end_date THEN
    RAISE EXCEPTION 'p_start_date must be before p_end_date';
  END IF;

  v_limit  := LEAST(GREATEST(COALESCE(p_limit, 100), 1), 10000);
  v_offset := GREATEST(COALESCE(p_offset, 0), 0);

  v_start_timestamp := EXTRACT(EPOCH FROM p_start_date)::BIGINT;
  v_end_timestamp   := EXTRACT(EPOCH FROM p_end_date)::BIGINT;

  v_bucket_start := date_trunc('hour', p_start_date);
  v_bucket_end   := date_trunc('hour', p_end_date);

  v_cagg_cutoff := date_trunc('hour', NOW());

  SET LOCAL work_mem = '64MB';

  -- Step 1: Active accounts (unchanged)
  CREATE TEMP TABLE _tmp_active_accounts ON COMMIT DROP AS
  SELECT DISTINCT pc.account_id
  FROM position_change_hourly pc
  WHERE pc.bucket >= v_bucket_start
    AND pc.bucket <= v_bucket_end
    AND (p_term_id IS NULL OR pc.term_id = p_term_id)
    AND (NOT p_exclude_protocol_accounts
         OR pc.account_id NOT IN (
           SELECT id FROM account
           WHERE type IN ('ProtocolVault', 'AtomWallet')
         ));

  ANALYZE _tmp_active_accounts;

  -- Step 2: OPTIMIZED position data — LATERAL per account (not per position)
  -- Uses two DISTINCT ON queries per account instead of one global DISTINCT ON
  -- + 67K LATERAL calls. Forces narrow index seeks via account_id binding.
  CREATE TEMP TABLE _tmp_position_data ON COMMIT DROP AS
  SELECT pd.*
  FROM _tmp_active_accounts aa
  CROSS JOIN LATERAL (
    SELECT
      aa.account_id,
      se.term_id,
      se.curve_id,
      COALESCE(ss.cumulative_shares, 0)::NUMERIC(78,0)     AS shares_at_start,
      se.cumulative_shares::NUMERIC(78,0)                   AS shares_at_end,
      (se.cumulative_assets_in  - COALESCE(ss.cumulative_assets_in,  0))::NUMERIC AS period_deposits,
      (se.cumulative_assets_out - COALESCE(ss.cumulative_assets_out, 0))::NUMERIC AS period_redemptions,
      ((se.cumulative_assets_in  - COALESCE(ss.cumulative_assets_in,  0)) > 0
       OR
       (se.cumulative_assets_out - COALESCE(ss.cumulative_assets_out, 0)) > 0
      ) AS had_activity,
      se.cumulative_assets_in::NUMERIC                      AS cumulative_deposits,
      (se.cumulative_shares_in  - COALESCE(ss.cumulative_shares_in,  0))::NUMERIC AS shares_acquired_in_period,
      (se.cumulative_shares_out - COALESCE(ss.cumulative_shares_out, 0))::NUMERIC AS shares_redeemed_in_period
    FROM (
      -- snap_end: latest cumulative for all (term, curve) for this account
      SELECT DISTINCT ON (pch.term_id, pch.curve_id)
        pch.term_id, pch.curve_id,
        pch.cumulative_shares, pch.cumulative_assets_in, pch.cumulative_assets_out,
        pch.cumulative_shares_in, pch.cumulative_shares_out
      FROM position_cumulative_hourly pch
      WHERE pch.account_id = aa.account_id
        AND pch.bucket <= v_bucket_end
        AND (p_term_id IS NULL OR pch.term_id = p_term_id)
      ORDER BY pch.term_id, pch.curve_id, pch.bucket DESC
    ) se
    LEFT JOIN (
      -- snap_start: latest cumulative before period start for this account
      SELECT DISTINCT ON (pch.term_id, pch.curve_id)
        pch.term_id, pch.curve_id,
        pch.cumulative_shares, pch.cumulative_assets_in, pch.cumulative_assets_out,
        pch.cumulative_shares_in, pch.cumulative_shares_out
      FROM position_cumulative_hourly pch
      WHERE pch.account_id = aa.account_id
        AND pch.bucket < v_bucket_start
        AND (p_term_id IS NULL OR pch.term_id = p_term_id)
      ORDER BY pch.term_id, pch.curve_id, pch.bucket DESC
    ) ss ON se.term_id = ss.term_id AND se.curve_id = ss.curve_id
  ) pd;

  ANALYZE _tmp_position_data;

  -- Steps 3-5: Price and vault state lookups (unchanged from 1771526434000)
  CREATE TEMP TABLE _tmp_prices_at_start ON COMMIT DROP AS
  SELECT DISTINCT ON (c.term_id, c.curve_id)
    c.term_id, c.curve_id, c.last_share_price AS share_price
  FROM share_price_change_stats_hourly c
  INNER JOIN (
    SELECT DISTINCT term_id, curve_id FROM _tmp_position_data
    WHERE COALESCE(shares_at_start, 0) > 0
  ) pd ON c.term_id = pd.term_id AND c.curve_id = pd.curve_id
  WHERE c.bucket <= date_trunc('hour', p_start_date)
  ORDER BY c.term_id, c.curve_id, c.bucket DESC;

  CREATE TEMP TABLE _tmp_prices_at_end ON COMMIT DROP AS
  SELECT DISTINCT ON (c.term_id, c.curve_id)
    c.term_id, c.curve_id, c.last_share_price AS share_price
  FROM share_price_change_stats_hourly c
  INNER JOIN (
    SELECT DISTINCT term_id, curve_id FROM _tmp_position_data
    WHERE shares_at_end > 0
  ) pd ON c.term_id = pd.term_id AND c.curve_id = pd.curve_id
  WHERE c.bucket <= date_trunc('hour', p_end_date)
  ORDER BY c.term_id, c.curve_id, c.bucket DESC;

  IF p_end_date > v_cagg_cutoff THEN
    WITH raw_current_hour AS (
      SELECT DISTINCT ON (spc.term_id, spc.curve_id)
        spc.term_id, spc.curve_id, spc.share_price
      FROM share_price_change spc
      WHERE spc.block_timestamp > EXTRACT(EPOCH FROM v_cagg_cutoff)::BIGINT
        AND spc.block_timestamp <= v_end_timestamp
        AND EXISTS (
          SELECT 1 FROM _tmp_prices_at_end pe
          WHERE pe.term_id = spc.term_id AND pe.curve_id = spc.curve_id
        )
      ORDER BY spc.term_id, spc.curve_id, spc.block_timestamp DESC, spc.log_index DESC
    )
    UPDATE _tmp_prices_at_end pe
    SET share_price = rch.share_price
    FROM raw_current_hour rch
    WHERE pe.term_id = rch.term_id AND pe.curve_id = rch.curve_id;

    INSERT INTO _tmp_prices_at_end (term_id, curve_id, share_price)
    SELECT DISTINCT ON (spc.term_id, spc.curve_id)
      spc.term_id, spc.curve_id, spc.share_price
    FROM share_price_change spc
    INNER JOIN (
      SELECT DISTINCT term_id, curve_id FROM _tmp_position_data
      WHERE shares_at_end > 0
    ) pd ON spc.term_id = pd.term_id AND spc.curve_id = pd.curve_id
    WHERE spc.block_timestamp <= v_end_timestamp
      AND NOT EXISTS (
        SELECT 1 FROM _tmp_prices_at_end pe
        WHERE pe.term_id = spc.term_id AND pe.curve_id = spc.curve_id
      )
    ORDER BY spc.term_id, spc.curve_id, spc.block_timestamp DESC, spc.log_index DESC;
  END IF;

  CREATE TEMP TABLE _tmp_vault_state_at_start ON COMMIT DROP AS
  SELECT DISTINCT ON (spc.term_id, spc.curve_id)
    spc.term_id,
    spc.curve_id,
    spc.total_shares AS vault_total_shares
  FROM share_price_change spc
  INNER JOIN (
    SELECT DISTINCT term_id, curve_id
    FROM _tmp_position_data
    WHERE COALESCE(shares_at_start, 0) > 0
      AND curve_id = 2
  ) pd ON spc.term_id = pd.term_id AND spc.curve_id = pd.curve_id
  WHERE spc.block_timestamp <= v_start_timestamp
  ORDER BY spc.term_id, spc.curve_id, spc.block_timestamp DESC, spc.log_index DESC;

  CREATE TEMP TABLE _tmp_vault_state_at_end ON COMMIT DROP AS
  SELECT DISTINCT ON (spc.term_id, spc.curve_id)
    spc.term_id,
    spc.curve_id,
    spc.total_shares AS vault_total_shares
  FROM share_price_change spc
  INNER JOIN (
    SELECT DISTINCT term_id, curve_id
    FROM _tmp_position_data
    WHERE shares_at_end > 0
      AND curve_id = 2
  ) pd ON spc.term_id = pd.term_id AND spc.curve_id = pd.curve_id
  WHERE spc.block_timestamp <= v_end_timestamp
  ORDER BY spc.term_id, spc.curve_id, spc.block_timestamp DESC, spc.log_index DESC;

  -- Main query: CTE chain (unchanged from 1771526434000)
  RETURN QUERY
  WITH
  position_period_metrics AS (
    SELECT
      pd.account_id,
      pd.term_id,
      pd.curve_id,
      COALESCE(pd.shares_at_start, 0)           AS shares_at_start,
      pd.shares_at_end                          AS shares_at_end,
      COALESCE(pd.period_deposits, 0)           AS period_deposits,
      COALESCE(pd.period_redemptions, 0)        AS period_redemptions,
      pd.had_activity,
      COALESCE(pd.cumulative_deposits, 0)       AS cumulative_deposits,
      COALESCE(pd.shares_acquired_in_period, 0) AS shares_acquired_in_period,
      COALESCE(pd.shares_redeemed_in_period, 0) AS shares_redeemed_in_period,
      CASE
        WHEN pd.curve_id = 2 AND COALESCE(pd.shares_at_start, 0) > 0 THEN
          TRUNC(
            (
              TRUNC(
                (vs.vault_total_shares + cc."offset")
                * (vs.vault_total_shares + cc."offset")
                / 1000000000000000000::NUMERIC
              )
              -
              TRUNC(
                (
                  (vs.vault_total_shares + cc."offset" - pd.shares_at_start)
                  * (vs.vault_total_shares + cc."offset" - pd.shares_at_start)
                  + 999999999999999999::NUMERIC
                )
                / 1000000000000000000::NUMERIC
              )
            ) * cc.half_slope / 1000000000000000000::NUMERIC
          )
        ELSE
          TRUNC(
            COALESCE(pd.shares_at_start, 0)
            * COALESCE(ps.share_price, 0)
            / 1000000000000000000::NUMERIC
          )
      END AS equity_at_start,
      CASE
        WHEN pd.curve_id = 2 AND pd.shares_at_end > 0 THEN
          TRUNC(
            (
              TRUNC(
                (ve.vault_total_shares + cc."offset")
                * (ve.vault_total_shares + cc."offset")
                / 1000000000000000000::NUMERIC
              )
              -
              TRUNC(
                (
                  (ve.vault_total_shares + cc."offset" - pd.shares_at_end)
                  * (ve.vault_total_shares + cc."offset" - pd.shares_at_end)
                  + 999999999999999999::NUMERIC
                )
                / 1000000000000000000::NUMERIC
              )
            ) * cc.half_slope / 1000000000000000000::NUMERIC
          )
        ELSE
          TRUNC(
            pd.shares_at_end
            * COALESCE(pe.share_price, 0)
            / 1000000000000000000::NUMERIC
          )
      END AS equity_at_end
    FROM _tmp_position_data pd
    LEFT JOIN _tmp_prices_at_start ps
      ON pd.term_id = ps.term_id AND pd.curve_id = ps.curve_id
    LEFT JOIN _tmp_prices_at_end pe
      ON pd.term_id = pe.term_id AND pd.curve_id = pe.curve_id
    LEFT JOIN curve_config cc
      ON pd.curve_id = cc.curve_id
    LEFT JOIN _tmp_vault_state_at_start vs
      ON pd.term_id = vs.term_id AND pd.curve_id = vs.curve_id
    LEFT JOIN _tmp_vault_state_at_end ve
      ON pd.term_id = ve.term_id AND pd.curve_id = ve.curve_id
  ),

  position_pnl AS (
    SELECT
      ppm.account_id,
      ppm.term_id,
      ppm.curve_id,
      ppm.shares_at_start,
      ppm.shares_at_end,
      ppm.period_deposits,
      ppm.period_redemptions,
      ppm.equity_at_start,
      ppm.equity_at_end,
      ppm.had_activity,
      ppm.cumulative_deposits,
      ppm.shares_acquired_in_period,
      ppm.shares_redeemed_in_period,
      (ppm.equity_at_end - ppm.equity_at_start
        + ppm.period_redemptions - ppm.period_deposits)::NUMERIC AS period_total_pnl,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start
                  + ppm.period_redemptions - ppm.period_deposits) > 0
           THEN 1 ELSE 0 END AS is_winning,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start
                  + ppm.period_redemptions - ppm.period_deposits) < 0
           THEN 1 ELSE 0 END AS is_losing,
      -- ---- EPOCH-ONLY REALIZED PnL (from 1771526434000) ----
      CASE
        WHEN ppm.period_redemptions <= 0 THEN 0::NUMERIC
        WHEN ppm.period_deposits <= 0 THEN 0::NUMERIC
        WHEN ppm.shares_at_start <= 0 THEN
          (ppm.period_redemptions - TRUNC(
            ppm.period_deposits * ppm.shares_redeemed_in_period
            / NULLIF(ppm.shares_acquired_in_period, 0)
          ))::NUMERIC
        ELSE
          (TRUNC(
            ppm.period_redemptions * ppm.shares_acquired_in_period
            / NULLIF(ppm.shares_at_start + ppm.shares_acquired_in_period, 0)
          ) - TRUNC(
            ppm.period_deposits * ppm.shares_redeemed_in_period
            / NULLIF(ppm.shares_at_start + ppm.shares_acquired_in_period, 0)
          ))::NUMERIC
      END AS realized_pnl
    FROM position_period_metrics ppm
  ),

  filtered_pnl AS (
    SELECT * FROM position_pnl pp
    WHERE p_min_deposit <= 0
       OR pp.cumulative_deposits >= p_min_deposit * 1e18
  ),

  account_metrics AS (
    SELECT
      fp.account_id,
      COUNT(DISTINCT (fp.term_id, fp.curve_id))
        FILTER (WHERE fp.had_activity)                           AS period_position_count,
      COUNT(DISTINCT (fp.term_id, fp.curve_id))
        FILTER (WHERE fp.shares_at_end > 0)                      AS active_position_count,
      SUM(fp.is_winning) FILTER (WHERE fp.had_activity)          AS winning_positions,
      SUM(fp.is_losing)  FILTER (WHERE fp.had_activity)          AS losing_positions,
      SUM(fp.period_deposits)::NUMERIC                           AS period_deposits_raw,
      SUM(fp.period_redemptions)::NUMERIC                        AS period_redemptions_raw,
      SUM(fp.period_total_pnl)::NUMERIC                          AS total_pnl_raw,
      SUM(fp.realized_pnl)::NUMERIC                              AS realized_pnl_raw,
      SUM(fp.period_total_pnl - fp.realized_pnl)::NUMERIC        AS unrealized_pnl_raw,
      SUM(fp.equity_at_start)::NUMERIC                           AS equity_at_start,
      SUM(fp.equity_at_end)::NUMERIC                             AS equity_at_end,
      MAX(fp.period_total_pnl)                                   AS best_trade_pnl_raw,
      MIN(fp.period_total_pnl)                                   AS worst_trade_pnl_raw,
      -- ---- EPOCH-ONLY DENOMINATORS (from 1771526434000) ----
      SUM(CASE
        WHEN fp.period_redemptions <= 0 OR fp.period_deposits <= 0 THEN 0
        WHEN fp.shares_at_start <= 0 THEN
          TRUNC(fp.period_deposits * fp.shares_redeemed_in_period
                / NULLIF(fp.shares_acquired_in_period, 0))
        ELSE
          TRUNC(fp.period_deposits * fp.shares_redeemed_in_period
                / NULLIF(fp.shares_at_start + fp.shares_acquired_in_period, 0))
      END)::NUMERIC                                              AS denominator_closed,
      SUM(CASE
        WHEN fp.period_deposits <= 0 OR fp.shares_at_end <= 0 THEN 0
        WHEN fp.period_redemptions <= 0 THEN fp.period_deposits
        WHEN fp.shares_at_start <= 0 THEN
          fp.period_deposits - TRUNC(fp.period_deposits * fp.shares_redeemed_in_period
                / NULLIF(fp.shares_acquired_in_period, 0))
        ELSE
          fp.period_deposits - TRUNC(fp.period_deposits * fp.shares_redeemed_in_period
                / NULLIF(fp.shares_at_start + fp.shares_acquired_in_period, 0))
      END)::NUMERIC                                              AS denominator_open
    FROM filtered_pnl fp
    GROUP BY fp.account_id
    HAVING
      COUNT(DISTINCT (fp.term_id, fp.curve_id)) FILTER (WHERE fp.had_activity)
        >= GREATEST(p_min_positions, 1)
      AND (SUM(fp.period_deposits) + SUM(fp.period_redemptions))
        >= COALESCE(p_min_volume * 1e18, 0)
  ),

  enriched AS (
    SELECT
      am.account_id,
      a.label AS account_label,
      a.image AS account_image,
      am.total_pnl_raw,
      COALESCE(am.realized_pnl_raw, 0)   AS realized_pnl_raw,
      COALESCE(am.unrealized_pnl_raw, 0) AS unrealized_pnl_raw,
      COALESCE(
        (am.total_pnl_raw * 100.0 / NULLIF(
          am.equity_at_start + am.period_deposits_raw
        , 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS pnl_pct,
      COALESCE(
        (COALESCE(am.realized_pnl_raw, 0) * 100.0 / NULLIF(am.denominator_closed, 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS realized_pnl_pct,
      COALESCE(
        (COALESCE(am.unrealized_pnl_raw, 0) * 100.0 / NULLIF(am.denominator_open, 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS unrealized_pnl_pct,
      am.total_pnl_raw                    AS pnl_change_raw,
      am.period_position_count            AS total_position_count,
      am.active_position_count,
      am.winning_positions,
      am.losing_positions,
      COALESCE(
        (am.winning_positions * 100.0 / NULLIF(am.period_position_count, 0))::NUMERIC(10, 2),
        0::NUMERIC(10, 2)
      ) AS win_rate,
      am.period_deposits_raw              AS total_deposits_raw,
      am.period_redemptions_raw           AS total_redemptions_raw,
      (am.period_deposits_raw + am.period_redemptions_raw)::NUMERIC AS total_volume_raw,
      am.equity_at_end                    AS current_equity_value_raw,
      am.best_trade_pnl_raw,
      am.worst_trade_pnl_raw,
      p_start_date                        AS first_position_at,
      p_end_date                          AS last_activity_at
    FROM account_metrics am
    JOIN account a ON am.account_id = a.id
    WHERE NOT p_exclude_protocol_accounts
       OR a.type NOT IN ('ProtocolVault', 'AtomWallet')
  ),

  ranked AS (
    SELECT
      e.*,
      CASE
        WHEN p_sort_by IN ('total_pnl', 'pnl') THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_pnl_raw ASC  NULLS LAST)
            ELSE             ROW_NUMBER() OVER (ORDER BY e.total_pnl_raw DESC NULLS LAST)
          END
        WHEN p_sort_by IN ('pnl_pct', 'roi') THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.pnl_pct ASC  NULLS LAST)
            ELSE             ROW_NUMBER() OVER (ORDER BY e.pnl_pct DESC NULLS LAST)
          END
        WHEN p_sort_by = 'realized_pnl' THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.realized_pnl_raw ASC  NULLS LAST)
            ELSE             ROW_NUMBER() OVER (ORDER BY e.realized_pnl_raw DESC NULLS LAST)
          END
        WHEN p_sort_by = 'unrealized_pnl' THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.unrealized_pnl_raw ASC  NULLS LAST)
            ELSE             ROW_NUMBER() OVER (ORDER BY e.unrealized_pnl_raw DESC NULLS LAST)
          END
        WHEN p_sort_by = 'realized_pnl_pct' THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.realized_pnl_pct ASC  NULLS LAST)
            ELSE             ROW_NUMBER() OVER (ORDER BY e.realized_pnl_pct DESC NULLS LAST)
          END
        WHEN p_sort_by = 'unrealized_pnl_pct' THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.unrealized_pnl_pct ASC  NULLS LAST)
            ELSE             ROW_NUMBER() OVER (ORDER BY e.unrealized_pnl_pct DESC NULLS LAST)
          END
        WHEN p_sort_by = 'win_rate' THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.win_rate ASC  NULLS LAST)
            ELSE             ROW_NUMBER() OVER (ORDER BY e.win_rate DESC NULLS LAST)
          END
        WHEN p_sort_by IN ('total_volume', 'volume') THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_volume_raw ASC  NULLS LAST)
            ELSE             ROW_NUMBER() OVER (ORDER BY e.total_volume_raw DESC NULLS LAST)
          END
        WHEN p_sort_by IN ('position_count', 'positions') THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_position_count ASC  NULLS LAST)
            ELSE             ROW_NUMBER() OVER (ORDER BY e.total_position_count DESC NULLS LAST)
          END
        ELSE ROW_NUMBER() OVER (ORDER BY e.total_pnl_raw DESC NULLS LAST)
      END AS rank
    FROM enriched e
  )

  SELECT
    r.rank,
    r.account_id,
    r.account_label,
    r.account_image,
    r.total_pnl_raw,
    ROUND(r.total_pnl_raw    / 1e18, 4)::NUMERIC(30, 4) AS total_pnl_formatted,
    r.realized_pnl_raw,
    ROUND(r.realized_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS realized_pnl_formatted,
    r.unrealized_pnl_raw,
    ROUND(r.unrealized_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS unrealized_pnl_formatted,
    r.pnl_pct,
    r.realized_pnl_pct,
    r.unrealized_pnl_pct,
    r.pnl_change_raw,
    ROUND(r.pnl_change_raw   / 1e18, 4)::NUMERIC(30, 4) AS pnl_change_formatted,
    r.total_position_count,
    r.active_position_count,
    r.winning_positions,
    r.losing_positions,
    r.win_rate,
    r.total_deposits_raw,
    ROUND(r.total_deposits_raw    / 1e18, 4)::NUMERIC(30, 4) AS total_deposits_formatted,
    r.total_redemptions_raw,
    ROUND(r.total_redemptions_raw / 1e18, 4)::NUMERIC(30, 4) AS total_redemptions_formatted,
    r.total_volume_raw,
    ROUND(r.total_volume_raw      / 1e18, 4)::NUMERIC(30, 4) AS total_volume_formatted,
    r.current_equity_value_raw,
    ROUND(r.current_equity_value_raw / 1e18, 4)::NUMERIC(30, 4) AS current_equity_value_formatted,
    r.best_trade_pnl_raw,
    ROUND(r.best_trade_pnl_raw  / 1e18, 4)::NUMERIC(30, 4) AS best_trade_pnl_formatted,
    r.worst_trade_pnl_raw,
    ROUND(r.worst_trade_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS worst_trade_pnl_formatted,
    NULL::NUMERIC         AS redeemable_assets_raw,
    NULL::NUMERIC(30, 4)  AS redeemable_assets_formatted,
    r.first_position_at,
    r.last_activity_at
  FROM ranked r
  ORDER BY r.rank
  LIMIT v_limit OFFSET v_offset;
END;
$$ LANGUAGE plpgsql VOLATILE;

COMMENT ON FUNCTION get_pnl_leaderboard_period IS
  'Period-specific PnL leaderboard with hourly-granularity period boundaries. '
  'Uses LATERAL-per-account pattern for position_cumulative_hourly lookups (19x faster). '
  'Realized PnL only counts in-epoch deployed capital (deposits made during the period). '
  'p_min_deposit (in ETH/TRUST units, default 0) filters out positions where '
  'cumulative all-time deposits are below the threshold. '
  'Sort options: total_pnl/pnl, pnl_pct/roi, realized_pnl, unrealized_pnl, '
  'realized_pnl_pct, unrealized_pnl_pct, win_rate, total_volume/volume, position_count/positions.';
