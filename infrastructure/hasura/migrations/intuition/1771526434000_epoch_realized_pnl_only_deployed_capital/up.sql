-- ============================================================================
-- Migration: 1771526434000_epoch_realized_pnl_only_deployed_capital
-- ============================================================================
--
-- PROBLEM:
--   The epoch leaderboard was classifying ALL PnL as "realized" when a position
--   was fully closed (shares_at_end = 0), even when the capital was deployed
--   BEFORE the epoch started. This led to misleading leaderboard results.
--
--   Example: proner.eth deposited 28,011 TRUST before Epoch 10, then redeemed
--   during the epoch for 27,451 TRUST. The leaderboard showed realized PnL of
--   -560 TRUST, but no capital was actually deployed within the epoch — the
--   epoch leaderboard should show realized = 0.
--
-- FIX:
--   Realized PnL on epoch leaderboards now only reflects gains/losses on capital
--   deployed (deposited) WITHIN the epoch. Four cases:
--
--   1. No redemptions in period                    → realized = 0
--   2. No in-epoch deposits (pre-existing only)    → realized = 0
--   3. New position (shares_at_start = 0)          → realized = redemptions - cost_basis
--      where cost_basis = deposits * (shares_redeemed / shares_acquired)
--   4. Mixed (pre-existing + in-epoch deposits)    → pro-rata only the epoch fraction:
--      epoch_fraction = shares_acquired / (shares_at_start + shares_acquired)
--      realized = (redemptions * epoch_fraction) - (deposits * shares_redeemed / total_pool)
--
--   The denominator_closed and denominator_open (used for realized_pnl_pct and
--   unrealized_pnl_pct) are updated to use only epoch-deployed capital.
--
-- SCOPE:
--   Modified:  get_pnl_leaderboard_period, get_vault_leaderboard_period
--   Unchanged: get_pnl_leaderboard (alltime), get_vault_leaderboard (alltime)
--
--   The alltime/global profile functions are intentionally NOT modified — they
--   continue to register all realized PnL regardless of when capital was deployed.
--
-- DEPENDS ON:
--   1771526433000_use_hourly_granularity_for_period_boundaries (base functions)
--   1771526432000_fix_bonding_curve_equity_valuation (equity calc preserved)
--
-- ============================================================================

-- ============================================================================
-- 1. get_pnl_leaderboard_period
-- ============================================================================

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

  CREATE TEMP TABLE _tmp_position_data ON COMMIT DROP AS
  SELECT
    snap_end.account_id,
    snap_end.term_id,
    snap_end.curve_id,
    COALESCE(snap_start.cumulative_shares, 0)::NUMERIC(78,0)     AS shares_at_start,
    snap_end.cumulative_shares::NUMERIC(78,0)                    AS shares_at_end,
    (snap_end.cumulative_assets_in  - COALESCE(snap_start.cumulative_assets_in,  0))::NUMERIC AS period_deposits,
    (snap_end.cumulative_assets_out - COALESCE(snap_start.cumulative_assets_out, 0))::NUMERIC AS period_redemptions,
    ((snap_end.cumulative_assets_in  - COALESCE(snap_start.cumulative_assets_in,  0)) > 0
     OR
     (snap_end.cumulative_assets_out - COALESCE(snap_start.cumulative_assets_out, 0)) > 0
    ) AS had_activity,
    snap_end.cumulative_assets_in::NUMERIC                       AS cumulative_deposits,
    (snap_end.cumulative_shares_in  - COALESCE(snap_start.cumulative_shares_in,  0))::NUMERIC AS shares_acquired_in_period,
    (snap_end.cumulative_shares_out - COALESCE(snap_start.cumulative_shares_out, 0))::NUMERIC AS shares_redeemed_in_period
  FROM (
    SELECT DISTINCT ON (pch.account_id, pch.term_id, pch.curve_id)
      pch.account_id, pch.term_id, pch.curve_id,
      pch.cumulative_shares,
      pch.cumulative_assets_in,
      pch.cumulative_assets_out,
      pch.cumulative_shares_in,
      pch.cumulative_shares_out
    FROM _tmp_active_accounts aa
    JOIN position_cumulative_hourly pch ON pch.account_id = aa.account_id
    WHERE pch.bucket <= v_bucket_end
      AND (p_term_id IS NULL OR pch.term_id = p_term_id)
    ORDER BY pch.account_id, pch.term_id, pch.curve_id, pch.bucket DESC
  ) snap_end
  LEFT JOIN LATERAL (
    SELECT
      cumulative_shares,
      cumulative_assets_in,
      cumulative_assets_out,
      cumulative_shares_in,
      cumulative_shares_out
    FROM position_cumulative_hourly pch
    WHERE pch.account_id = snap_end.account_id
      AND pch.term_id    = snap_end.term_id
      AND pch.curve_id   = snap_end.curve_id
      AND pch.bucket     < v_bucket_start
    ORDER BY pch.bucket DESC
    LIMIT 1
  ) snap_start ON true;

  ANALYZE _tmp_position_data;

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
      -- ---- EPOCH-ONLY REALIZED PnL ----
      -- Only capital deployed (deposited) within this epoch generates realized PnL.
      -- Pre-existing equity that gets redeemed is NOT realized for epoch purposes.
      CASE
        -- No redemptions in period → nothing to realize
        WHEN ppm.period_redemptions <= 0 THEN 0::NUMERIC
        -- No in-epoch deposits → all redemptions are against pre-existing capital → realized = 0
        WHEN ppm.period_deposits <= 0 THEN 0::NUMERIC
        -- New position (no pre-existing shares) → standard: redemptions minus pro-rata cost basis
        WHEN ppm.shares_at_start <= 0 THEN
          (ppm.period_redemptions - TRUNC(
            ppm.period_deposits * ppm.shares_redeemed_in_period
            / NULLIF(ppm.shares_acquired_in_period, 0)
          ))::NUMERIC
        -- Mixed (pre-existing + in-epoch deposits) → pro-rata only the epoch fraction:
        --   epoch_share_of_redemptions = redemptions × (acquired / (start + acquired))
        --   epoch_cost_of_redeemed     = deposits × (redeemed / (start + acquired))
        --   realized = epoch_share_of_redemptions - epoch_cost_of_redeemed
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
      -- ---- EPOCH-ONLY DENOMINATORS (for realized_pnl_pct / unrealized_pnl_pct) ----
      -- denominator_closed: epoch capital that was "at risk" for the realized portion
      -- denominator_open: epoch capital still deployed (not yet redeemed)
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
  'Uses position_cumulative_hourly for efficient snapshot lookups. '
  'Realized PnL only counts in-epoch deployed capital (deposits made during the period). '
  'p_min_deposit (in ETH/TRUST units, default 0) filters out positions where '
  'cumulative all-time deposits are below the threshold. '
  'Sort options: total_pnl/pnl, pnl_pct/roi, realized_pnl, unrealized_pnl, '
  'realized_pnl_pct, unrealized_pnl_pct, win_rate, total_volume/volume, position_count/positions.';

-- ============================================================================
-- 2. get_vault_leaderboard_period
--    Changes vs 1771526433000:
--    - period_activity CTE: added shares_acquired_in_period, shares_redeemed_in_period
--    - position_period_metrics CTE: propagated new share flow columns
--    - realized_pnl: same epoch-only formula as get_pnl_leaderboard_period
--    - denominator_closed / denominator_open: epoch-only capital
-- ============================================================================

CREATE OR REPLACE FUNCTION get_vault_leaderboard_period(
  p_term_id TEXT,
  p_start_date TIMESTAMPTZ,
  p_end_date TIMESTAMPTZ,
  p_curve_id NUMERIC DEFAULT NULL,
  p_limit INTEGER DEFAULT 100,
  p_offset INTEGER DEFAULT 0,
  p_sort_by TEXT DEFAULT 'total_pnl',
  p_sort_order TEXT DEFAULT 'DESC'
)
RETURNS SETOF pnl_leaderboard_entry AS $$
DECLARE
  v_start_timestamp BIGINT;
  v_end_timestamp BIGINT;
  v_bucket_start TIMESTAMPTZ;
  v_bucket_end TIMESTAMPTZ;
  v_limit INTEGER;
  v_offset INTEGER;
BEGIN
  IF p_term_id IS NULL OR p_term_id = '' THEN
    RAISE EXCEPTION 'p_term_id is required';
  END IF;

  IF p_start_date IS NULL OR p_end_date IS NULL THEN
    RAISE EXCEPTION 'p_start_date and p_end_date are required';
  END IF;

  IF p_start_date >= p_end_date THEN
    RAISE EXCEPTION 'p_start_date must be before p_end_date';
  END IF;

  v_limit := LEAST(GREATEST(COALESCE(p_limit, 100), 1), 10000);
  v_offset := GREATEST(COALESCE(p_offset, 0), 0);

  v_start_timestamp := EXTRACT(EPOCH FROM p_start_date)::BIGINT;
  v_end_timestamp := EXTRACT(EPOCH FROM p_end_date)::BIGINT;

  v_bucket_start := date_trunc('hour', p_start_date);
  v_bucket_end := date_trunc('hour', p_end_date);

  RETURN QUERY
  WITH
  fc AS (SELECT * FROM fee_config LIMIT 1),

  active_accounts AS (
    SELECT DISTINCT pc.account_id
    FROM position_change_hourly pc
    WHERE pc.bucket >= v_bucket_start
      AND pc.bucket <= v_bucket_end
      AND pc.term_id = p_term_id
      AND (p_curve_id IS NULL OR pc.curve_id = p_curve_id)
  ),

  prices_before_start AS (
    SELECT DISTINCT ON (term_id, curve_id)
      term_id, curve_id, share_price, total_assets, total_shares
    FROM share_price_change
    WHERE term_id = p_term_id
      AND (p_curve_id IS NULL OR curve_id = p_curve_id)
      AND block_timestamp <= v_start_timestamp
    ORDER BY term_id, curve_id, block_timestamp DESC, log_index DESC
  ),
  prices_earliest AS (
    SELECT DISTINCT ON (term_id, curve_id)
      term_id, curve_id, share_price, total_assets, total_shares
    FROM share_price_change
    WHERE term_id = p_term_id
      AND (p_curve_id IS NULL OR curve_id = p_curve_id)
    ORDER BY term_id, curve_id, block_timestamp ASC, log_index ASC
  ),

  vault_state_end AS (
    SELECT DISTINCT ON (term_id, curve_id)
      term_id, curve_id, share_price, total_assets, total_shares
    FROM share_price_change
    WHERE term_id = p_term_id
      AND (p_curve_id IS NULL OR curve_id = p_curve_id)
      AND block_timestamp <= v_end_timestamp
    ORDER BY term_id, curve_id, block_timestamp DESC, log_index DESC
  ),

  -- Refactored: use position_cumulative_hourly for O(log n) point lookup
  -- instead of scanning all historical daily rows with GROUP BY
  position_state_start AS (
    SELECT DISTINCT ON (pch.account_id, pch.term_id, pch.curve_id)
      pch.account_id, pch.term_id, pch.curve_id,
      pch.cumulative_shares AS shares_at_start,
      pch.cumulative_assets_in AS cumulative_deposits_before,
      pch.cumulative_assets_out AS cumulative_redemptions_before
    FROM position_cumulative_hourly pch
    WHERE pch.bucket < v_bucket_start
      AND pch.term_id = p_term_id
      AND (p_curve_id IS NULL OR pch.curve_id = p_curve_id)
    ORDER BY pch.account_id, pch.term_id, pch.curve_id, pch.bucket DESC
  ),

  -- Refactored: same as above for end-of-period snapshot
  position_state_end AS (
    SELECT DISTINCT ON (pch.account_id, pch.term_id, pch.curve_id)
      pch.account_id, pch.term_id, pch.curve_id,
      pch.cumulative_shares AS shares_at_end,
      pch.cumulative_assets_in AS cumulative_deposits_through,
      pch.cumulative_assets_out AS cumulative_redemptions_through
    FROM position_cumulative_hourly pch
    WHERE pch.bucket <= v_bucket_end
      AND pch.term_id = p_term_id
      AND (p_curve_id IS NULL OR pch.curve_id = p_curve_id)
    ORDER BY pch.account_id, pch.term_id, pch.curve_id, pch.bucket DESC
  ),

  -- Bounded hourly scan for period-only activity
  period_activity AS (
    SELECT
      pc.account_id, pc.term_id, pc.curve_id,
      SUM(pc.assets_in_period)::NUMERIC AS period_deposits,
      SUM(pc.assets_out_period)::NUMERIC AS period_redemptions,
      SUM(pc.shares_delta_period)::NUMERIC AS period_shares_delta,
      SUM(pc.shares_in_period)::NUMERIC AS shares_acquired_in_period,
      SUM(pc.shares_out_period)::NUMERIC AS shares_redeemed_in_period
    FROM position_change_hourly pc
    WHERE pc.bucket >= v_bucket_start
      AND pc.bucket <= v_bucket_end
      AND pc.term_id = p_term_id
      AND (p_curve_id IS NULL OR pc.curve_id = p_curve_id)
    GROUP BY pc.account_id, pc.term_id, pc.curve_id
  ),

  position_period_metrics AS (
    SELECT
      COALESCE(pss.account_id, pse.account_id, pa.account_id) AS account_id,
      COALESCE(pss.term_id, pse.term_id, pa.term_id) AS term_id,
      COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) AS curve_id,
      COALESCE(pss.shares_at_start, 0) AS shares_at_start,
      COALESCE(pse.shares_at_end, 0) AS shares_at_end,
      COALESCE(pa.period_deposits, 0) AS period_deposits,
      COALESCE(pa.period_redemptions, 0) AS period_redemptions,
      COALESCE(pa.shares_acquired_in_period, 0) AS shares_acquired_in_period,
      COALESCE(pa.shares_redeemed_in_period, 0) AS shares_redeemed_in_period,
      COALESCE(pbs.share_price, pe_earliest.share_price) AS price_at_start,
      COALESCE(ve.share_price, 0) AS price_at_end,
      COALESCE(ve.total_assets, 0) AS vault_total_assets_end,
      COALESCE(ve.total_shares, 0) AS vault_total_shares_end,
      CASE
        WHEN COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) = 2
             AND COALESCE(pss.shares_at_start, 0) > 0 THEN
          TRUNC(
            (
              TRUNC(
                (COALESCE(pbs.total_shares, pe_earliest.total_shares, 0) + cc."offset")
                * (COALESCE(pbs.total_shares, pe_earliest.total_shares, 0) + cc."offset")
                / 1000000000000000000::NUMERIC
              )
              -
              TRUNC(
                (
                  (COALESCE(pbs.total_shares, pe_earliest.total_shares, 0) + cc."offset" - COALESCE(pss.shares_at_start, 0))
                  * (COALESCE(pbs.total_shares, pe_earliest.total_shares, 0) + cc."offset" - COALESCE(pss.shares_at_start, 0))
                  + 999999999999999999::NUMERIC
                )
                / 1000000000000000000::NUMERIC
              )
            ) * cc.half_slope / 1000000000000000000::NUMERIC
          )
        ELSE
          TRUNC(COALESCE(pss.shares_at_start, 0) * COALESCE(pbs.share_price, pe_earliest.share_price) / 1000000000000000000::NUMERIC)
      END AS equity_at_start,
      CASE
        WHEN COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) = 2
             AND COALESCE(pse.shares_at_end, 0) > 0 THEN
          TRUNC(
            (
              TRUNC((COALESCE(ve.total_shares, 0) + cc."offset") * (COALESCE(ve.total_shares, 0) + cc."offset") / 1000000000000000000::NUMERIC)
              -
              TRUNC(((COALESCE(ve.total_shares, 0) + cc."offset" - COALESCE(pse.shares_at_end, 0)) * (COALESCE(ve.total_shares, 0) + cc."offset" - COALESCE(pse.shares_at_end, 0)) + 999999999999999999::NUMERIC) / 1000000000000000000::NUMERIC)
            )
            * cc.half_slope / 1000000000000000000::NUMERIC
          )
        ELSE
          TRUNC(COALESCE(pse.shares_at_end, 0) * COALESCE(ve.share_price, 0) / 1000000000000000000::NUMERIC)
      END AS equity_at_end,
      CASE WHEN pa.account_id IS NOT NULL THEN TRUE ELSE FALSE END AS had_activity
    FROM position_state_start pss
    FULL OUTER JOIN position_state_end pse
      ON pss.account_id = pse.account_id AND pss.term_id = pse.term_id AND pss.curve_id = pse.curve_id
    FULL OUTER JOIN period_activity pa
      ON COALESCE(pss.account_id, pse.account_id) = pa.account_id
      AND COALESCE(pss.term_id, pse.term_id) = pa.term_id
      AND COALESCE(pss.curve_id, pse.curve_id) = pa.curve_id
    LEFT JOIN prices_before_start pbs
      ON COALESCE(pss.term_id, pse.term_id, pa.term_id) = pbs.term_id
      AND COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) = pbs.curve_id
    LEFT JOIN prices_earliest pe_earliest
      ON COALESCE(pss.term_id, pse.term_id, pa.term_id) = pe_earliest.term_id
      AND COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) = pe_earliest.curve_id
    LEFT JOIN vault_state_end ve
      ON COALESCE(pss.term_id, pse.term_id, pa.term_id) = ve.term_id
      AND COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) = ve.curve_id
    LEFT JOIN curve_config cc
      ON COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) = cc.curve_id
    WHERE COALESCE(pss.account_id, pse.account_id, pa.account_id) IN (SELECT account_id FROM active_accounts)
  ),

  position_pnl AS (
    SELECT
      ppm.*,
      (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits)::NUMERIC AS period_total_pnl,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits) > 0 THEN 1 ELSE 0 END AS is_winning,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits) < 0 THEN 1 ELSE 0 END AS is_losing,
      -- ---- EPOCH-ONLY REALIZED PnL (same logic as get_pnl_leaderboard_period) ----
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
      END AS realized_pnl,
      CASE
        WHEN ppm.shares_at_end = 0 THEN 0::NUMERIC
        WHEN cc.curve_type = 'linear' THEN
          CASE WHEN ppm.vault_total_shares_end > 0
            THEN TRUNC(ppm.shares_at_end * ppm.vault_total_assets_end / ppm.vault_total_shares_end)
            ELSE 0::NUMERIC
          END
        WHEN cc.curve_type = 'offset_progressive' THEN
          TRUNC(
            (
              TRUNC((ppm.vault_total_shares_end + cc."offset") * (ppm.vault_total_shares_end + cc."offset") / 1000000000000000000::NUMERIC)
              -
              TRUNC(((ppm.vault_total_shares_end + cc."offset" - ppm.shares_at_end) * (ppm.vault_total_shares_end + cc."offset" - ppm.shares_at_end) + 999999999999999999::NUMERIC) / 1000000000000000000::NUMERIC)
            )
            * cc.half_slope / 1000000000000000000::NUMERIC
          )
        ELSE 0::NUMERIC
      END AS raw_assets_from_curve
    FROM position_period_metrics ppm
    LEFT JOIN curve_config cc ON ppm.curve_id = cc.curve_id
  ),

  position_with_fees AS (
    SELECT
      pp.*,
      TRUNC((pp.raw_assets_from_curve * f.protocol_fee + f.fee_denominator - 1) / f.fee_denominator)::NUMERIC AS protocol_fee,
      TRUNC((pp.raw_assets_from_curve * f.exit_fee + f.fee_denominator - 1) / f.fee_denominator)::NUMERIC AS exit_fee
    FROM position_pnl pp
    CROSS JOIN fc f
  ),

  position_final AS (
    SELECT
      pwf.*,
      GREATEST(pwf.raw_assets_from_curve - pwf.protocol_fee - pwf.exit_fee, 0)::NUMERIC AS redeemable_assets_raw
    FROM position_with_fees pwf
  ),

  account_metrics AS (
    SELECT
      pf.account_id,
      COUNT(DISTINCT (pf.term_id, pf.curve_id)) FILTER (WHERE pf.had_activity OR pf.shares_at_end > 0) AS period_position_count,
      COUNT(DISTINCT (pf.term_id, pf.curve_id)) FILTER (WHERE pf.shares_at_end > 0) AS active_position_count,
      SUM(pf.is_winning) AS winning_positions,
      SUM(pf.is_losing) AS losing_positions,
      SUM(pf.period_deposits)::NUMERIC AS period_deposits_raw,
      SUM(pf.period_redemptions)::NUMERIC AS period_redemptions_raw,
      SUM(pf.period_total_pnl)::NUMERIC AS total_pnl_raw,
      SUM(pf.realized_pnl)::NUMERIC AS realized_pnl_raw,
      SUM(pf.period_total_pnl - pf.realized_pnl)::NUMERIC AS unrealized_pnl_raw,
      SUM(pf.equity_at_start)::NUMERIC AS equity_at_start,
      SUM(pf.equity_at_end)::NUMERIC AS equity_at_end,
      MAX(pf.period_total_pnl) AS best_trade_pnl_raw,
      MIN(pf.period_total_pnl) AS worst_trade_pnl_raw,
      SUM(pf.redeemable_assets_raw) FILTER (WHERE pf.shares_at_end > 0)::NUMERIC AS redeemable_assets_raw,
      -- ---- EPOCH-ONLY DENOMINATORS ----
      SUM(CASE
        WHEN pf.period_redemptions <= 0 OR pf.period_deposits <= 0 THEN 0
        WHEN pf.shares_at_start <= 0 THEN
          TRUNC(pf.period_deposits * pf.shares_redeemed_in_period
                / NULLIF(pf.shares_acquired_in_period, 0))
        ELSE
          TRUNC(pf.period_deposits * pf.shares_redeemed_in_period
                / NULLIF(pf.shares_at_start + pf.shares_acquired_in_period, 0))
      END)::NUMERIC AS denominator_closed,
      SUM(CASE
        WHEN pf.period_deposits <= 0 OR pf.shares_at_end <= 0 THEN 0
        WHEN pf.period_redemptions <= 0 THEN pf.period_deposits
        WHEN pf.shares_at_start <= 0 THEN
          pf.period_deposits - TRUNC(pf.period_deposits * pf.shares_redeemed_in_period
                / NULLIF(pf.shares_acquired_in_period, 0))
        ELSE
          pf.period_deposits - TRUNC(pf.period_deposits * pf.shares_redeemed_in_period
                / NULLIF(pf.shares_at_start + pf.shares_acquired_in_period, 0))
      END)::NUMERIC AS denominator_open
    FROM position_final pf
    GROUP BY pf.account_id
  ),

  enriched AS (
    SELECT
      am.account_id,
      a.label AS account_label,
      a.image AS account_image,
      am.total_pnl_raw,
      COALESCE(am.realized_pnl_raw, 0) AS realized_pnl_raw,
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
      am.total_pnl_raw AS pnl_change_raw,
      am.period_position_count AS total_position_count,
      am.active_position_count,
      am.winning_positions,
      am.losing_positions,
      COALESCE(
        (am.winning_positions * 100.0 / NULLIF(am.period_position_count, 0))::NUMERIC(10, 2),
        0::NUMERIC(10, 2)
      ) AS win_rate,
      am.period_deposits_raw AS total_deposits_raw,
      am.period_redemptions_raw AS total_redemptions_raw,
      (am.period_deposits_raw + am.period_redemptions_raw)::NUMERIC AS total_volume_raw,
      am.equity_at_end AS current_equity_value_raw,
      am.best_trade_pnl_raw,
      am.worst_trade_pnl_raw,
      COALESCE(am.redeemable_assets_raw, 0) AS redeemable_assets_raw,
      p_start_date AS first_position_at,
      p_end_date AS last_activity_at
    FROM account_metrics am
    JOIN account a ON am.account_id = a.id
    WHERE a.type NOT IN ('ProtocolVault', 'AtomWallet')
  ),

  ranked AS (
    SELECT
      e.*,
      CASE p_sort_by
        WHEN 'total_pnl' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_pnl_raw ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_pnl_raw DESC NULLS LAST) END
        WHEN 'pnl_pct' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.pnl_pct ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.pnl_pct DESC NULLS LAST) END
        WHEN 'win_rate' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.win_rate ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.win_rate DESC NULLS LAST) END
        WHEN 'total_volume' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_volume_raw ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_volume_raw DESC NULLS LAST) END
        WHEN 'position_count' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_position_count ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_position_count DESC NULLS LAST) END
        WHEN 'redeemable_assets' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.redeemable_assets_raw ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.redeemable_assets_raw DESC NULLS LAST) END
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
    ROUND(r.total_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS total_pnl_formatted,
    r.realized_pnl_raw,
    ROUND(r.realized_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS realized_pnl_formatted,
    r.unrealized_pnl_raw,
    ROUND(r.unrealized_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS unrealized_pnl_formatted,
    r.pnl_pct,
    r.realized_pnl_pct,
    r.unrealized_pnl_pct,
    r.pnl_change_raw,
    ROUND(r.pnl_change_raw / 1e18, 4)::NUMERIC(30, 4) AS pnl_change_formatted,
    r.total_position_count,
    r.active_position_count,
    r.winning_positions,
    r.losing_positions,
    r.win_rate,
    r.total_deposits_raw,
    ROUND(r.total_deposits_raw / 1e18, 4)::NUMERIC(30, 4) AS total_deposits_formatted,
    r.total_redemptions_raw,
    ROUND(r.total_redemptions_raw / 1e18, 4)::NUMERIC(30, 4) AS total_redemptions_formatted,
    r.total_volume_raw,
    ROUND(r.total_volume_raw / 1e18, 4)::NUMERIC(30, 4) AS total_volume_formatted,
    r.current_equity_value_raw,
    ROUND(r.current_equity_value_raw / 1e18, 4)::NUMERIC(30, 4) AS current_equity_value_formatted,
    r.best_trade_pnl_raw,
    ROUND(r.best_trade_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS best_trade_pnl_formatted,
    r.worst_trade_pnl_raw,
    ROUND(r.worst_trade_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS worst_trade_pnl_formatted,
    r.redeemable_assets_raw,
    ROUND(r.redeemable_assets_raw / 1e18, 4)::NUMERIC(30, 4) AS redeemable_assets_formatted,
    r.first_position_at,
    r.last_activity_at
  FROM ranked r
  ORDER BY r.rank
  LIMIT v_limit OFFSET v_offset;
END;
$$ LANGUAGE plpgsql VOLATILE;
