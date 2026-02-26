-- Revert pnl_pct denominator fix: restore the version from
-- 1771526404000_optimize_pnl_leaderboard_period_performance which used
-- CASE WHEN equity_at_start > 0 THEN equity_at_start ELSE period_deposits_raw END

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
  p_term_id TEXT DEFAULT NULL
)
RETURNS SETOF pnl_leaderboard_entry AS $$
DECLARE
  v_start_timestamp BIGINT;
  v_end_timestamp   BIGINT;
  v_bucket_start    TIMESTAMPTZ;
  v_bucket_end      TIMESTAMPTZ;
  v_limit           INTEGER;
  v_offset          INTEGER;
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

  v_bucket_start := date_trunc('day', p_start_date);
  v_bucket_end   := date_trunc('day', p_end_date);

  RETURN QUERY
  WITH
  active_accounts AS (
    SELECT DISTINCT pc.account_id
    FROM position_change_daily pc
    WHERE pc.bucket >= v_bucket_start
      AND pc.bucket <= v_bucket_end
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
  ),

  relevant_vaults AS (
    SELECT DISTINCT pc.term_id, pc.curve_id
    FROM position_change_daily pc
    WHERE pc.account_id IN (SELECT account_id FROM active_accounts)
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
  ),

  prices_at_start AS (
    SELECT DISTINCT ON (spc.term_id, spc.curve_id)
      spc.term_id, spc.curve_id, spc.share_price
    FROM share_price_change spc
    INNER JOIN relevant_vaults rv
      ON spc.term_id = rv.term_id AND spc.curve_id = rv.curve_id
    WHERE spc.block_timestamp <= v_start_timestamp
    ORDER BY spc.term_id, spc.curve_id, spc.block_timestamp DESC, spc.log_index DESC
  ),

  prices_at_end AS (
    SELECT DISTINCT ON (spc.term_id, spc.curve_id)
      spc.term_id, spc.curve_id, spc.share_price
    FROM share_price_change spc
    INNER JOIN relevant_vaults rv
      ON spc.term_id = rv.term_id AND spc.curve_id = rv.curve_id
    WHERE spc.block_timestamp <= v_end_timestamp
    ORDER BY spc.term_id, spc.curve_id, spc.block_timestamp DESC, spc.log_index DESC
  ),

  position_data AS (
    SELECT
      pc.account_id,
      pc.term_id,
      pc.curve_id,
      SUM(pc.shares_delta_period) FILTER (WHERE pc.bucket < v_bucket_start)::NUMERIC
        AS shares_at_start,
      SUM(pc.shares_delta_period)::NUMERIC
        AS shares_at_end,
      SUM(pc.assets_in_period) FILTER (WHERE pc.bucket >= v_bucket_start)::NUMERIC
        AS period_deposits,
      SUM(pc.assets_out_period) FILTER (WHERE pc.bucket >= v_bucket_start)::NUMERIC
        AS period_redemptions,
      BOOL_OR(pc.bucket >= v_bucket_start AND pc.bucket <= v_bucket_end)
        AS had_activity
    FROM position_change_daily pc
    WHERE pc.bucket <= v_bucket_end
      AND pc.account_id IN (SELECT account_id FROM active_accounts)
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
    GROUP BY pc.account_id, pc.term_id, pc.curve_id
  ),

  position_period_metrics AS (
    SELECT
      pd.account_id,
      pd.term_id,
      pd.curve_id,
      COALESCE(pd.shares_at_start, 0)       AS shares_at_start,
      pd.shares_at_end                       AS shares_at_end,
      COALESCE(pd.period_deposits, 0)        AS period_deposits,
      COALESCE(pd.period_redemptions, 0)     AS period_redemptions,
      pd.had_activity,
      TRUNC(
        COALESCE(pd.shares_at_start, 0)
        * COALESCE(ps.share_price, 0)
        / 1000000000000000000::NUMERIC
      ) AS equity_at_start,
      TRUNC(
        pd.shares_at_end
        * COALESCE(pe.share_price, 0)
        / 1000000000000000000::NUMERIC
      ) AS equity_at_end
    FROM position_data pd
    LEFT JOIN prices_at_start ps
      ON pd.term_id = ps.term_id AND pd.curve_id = ps.curve_id
    LEFT JOIN prices_at_end pe
      ON pd.term_id = pe.term_id AND pd.curve_id = pe.curve_id
  ),

  position_pnl AS (
    SELECT
      ppm.account_id,
      ppm.term_id,
      ppm.curve_id,
      ppm.shares_at_end,
      ppm.period_deposits,
      ppm.period_redemptions,
      ppm.equity_at_start,
      ppm.equity_at_end,
      ppm.had_activity,
      (ppm.equity_at_end - ppm.equity_at_start
        + ppm.period_redemptions - ppm.period_deposits)::NUMERIC AS period_total_pnl,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start
                  + ppm.period_redemptions - ppm.period_deposits) > 0
           THEN 1 ELSE 0 END AS is_winning,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start
                  + ppm.period_redemptions - ppm.period_deposits) < 0
           THEN 1 ELSE 0 END AS is_losing
    FROM position_period_metrics ppm
  ),

  account_metrics AS (
    SELECT
      pp.account_id,
      COUNT(DISTINCT (pp.term_id, pp.curve_id))
        FILTER (WHERE pp.had_activity OR pp.shares_at_end > 0)  AS period_position_count,
      COUNT(DISTINCT (pp.term_id, pp.curve_id))
        FILTER (WHERE pp.shares_at_end > 0)                     AS active_position_count,
      SUM(pp.is_winning)                                         AS winning_positions,
      SUM(pp.is_losing)                                          AS losing_positions,
      SUM(pp.period_deposits)::NUMERIC                           AS period_deposits_raw,
      SUM(pp.period_redemptions)::NUMERIC                        AS period_redemptions_raw,
      SUM(pp.period_total_pnl)::NUMERIC                          AS total_pnl_raw,
      SUM(pp.period_total_pnl) FILTER (WHERE pp.shares_at_end <= 0)::NUMERIC
                                                                 AS realized_pnl_raw,
      SUM(pp.period_total_pnl) FILTER (WHERE pp.shares_at_end > 0)::NUMERIC
                                                                 AS unrealized_pnl_raw,
      SUM(pp.equity_at_start)::NUMERIC                           AS equity_at_start,
      SUM(pp.equity_at_end)::NUMERIC                             AS equity_at_end,
      MAX(pp.period_total_pnl)                                   AS best_trade_pnl_raw,
      MIN(pp.period_total_pnl)                                   AS worst_trade_pnl_raw
    FROM position_pnl pp
    GROUP BY pp.account_id
    HAVING
      COUNT(DISTINCT (pp.term_id, pp.curve_id)) FILTER (WHERE pp.had_activity)
        >= GREATEST(p_min_positions, 1)
      AND (SUM(pp.period_deposits) + SUM(pp.period_redemptions))
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
          CASE WHEN am.equity_at_start > 0 THEN am.equity_at_start
               ELSE am.period_deposits_raw
          END, 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS pnl_pct,
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
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION get_pnl_leaderboard_period IS
  'Period-specific PnL leaderboard. Optimized with single-pass position aggregation, '
  'two-CTE price lookup (vs three), relevant_vaults filtering, and TimescaleDB chunk exclusion.';
