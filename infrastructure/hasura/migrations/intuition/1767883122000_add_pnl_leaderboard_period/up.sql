-- PnL Leaderboard Period Functions
-- Period-specific leaderboard functions that calculate metrics ONLY for the specified date range
-- Uses share_price_change table for accurate historical valuations

-- ========================================
-- DROP EXISTING FUNCTIONS (if any)
-- ========================================

DROP FUNCTION IF EXISTS get_pnl_leaderboard_period(TIMESTAMPTZ, TIMESTAMPTZ, INTEGER, INTEGER, TEXT, TEXT, BOOLEAN, INTEGER, NUMERIC, TEXT);
DROP FUNCTION IF EXISTS get_vault_leaderboard_period(TEXT, TIMESTAMPTZ, TIMESTAMPTZ, NUMERIC, INTEGER, INTEGER, TEXT, TEXT);

-- ========================================
-- PERIOD-SPECIFIC GLOBAL LEADERBOARD
-- ========================================

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
  v_end_timestamp BIGINT;
  v_limit INTEGER;
  v_offset INTEGER;
BEGIN
  -- Input validation
  IF p_start_date IS NULL OR p_end_date IS NULL THEN
    RAISE EXCEPTION 'p_start_date and p_end_date are required';
  END IF;

  IF p_start_date >= p_end_date THEN
    RAISE EXCEPTION 'p_start_date must be before p_end_date';
  END IF;

  -- Input validation for limits
  v_limit := LEAST(GREATEST(COALESCE(p_limit, 100), 1), 10000);
  v_offset := GREATEST(COALESCE(p_offset, 0), 0);

  -- Convert timestamps to Unix seconds for share_price_change lookup
  v_start_timestamp := EXTRACT(EPOCH FROM p_start_date)::BIGINT;
  v_end_timestamp := EXTRACT(EPOCH FROM p_end_date)::BIGINT;

  RETURN QUERY
  WITH
  -- Get accounts with activity in the period (FAST - uses continuous aggregate)
  active_accounts AS (
    SELECT DISTINCT pc.account_id
    FROM position_change_daily pc
    WHERE pc.bucket >= p_start_date
      AND pc.bucket <= p_end_date
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
  ),

  -- Get historical share prices at period START for each vault
  -- Uses the most recent price record before the start date
  prices_at_start AS (
    SELECT DISTINCT ON (term_id, curve_id)
      term_id,
      curve_id,
      share_price,
      total_assets,
      total_shares
    FROM share_price_change
    WHERE block_timestamp <= v_start_timestamp
    ORDER BY term_id, curve_id, block_timestamp DESC, log_index DESC
  ),

  -- Get historical share prices at period END for each vault
  prices_at_end AS (
    SELECT DISTINCT ON (term_id, curve_id)
      term_id,
      curve_id,
      share_price,
      total_assets,
      total_shares
    FROM share_price_change
    WHERE block_timestamp <= v_end_timestamp
    ORDER BY term_id, curve_id, block_timestamp DESC, log_index DESC
  ),

  -- Calculate position state at period START using cumulative sums
  -- (sum of all position changes before the period)
  position_state_start AS (
    SELECT
      pc.account_id,
      pc.term_id,
      pc.curve_id,
      SUM(pc.shares_delta_period)::NUMERIC AS shares_at_start,
      SUM(pc.assets_in_period)::NUMERIC AS cumulative_deposits_before,
      SUM(pc.assets_out_period)::NUMERIC AS cumulative_redemptions_before
    FROM position_change_daily pc
    WHERE pc.bucket < p_start_date
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
    GROUP BY pc.account_id, pc.term_id, pc.curve_id
  ),

  -- Calculate position state at period END
  position_state_end AS (
    SELECT
      pc.account_id,
      pc.term_id,
      pc.curve_id,
      SUM(pc.shares_delta_period)::NUMERIC AS shares_at_end,
      SUM(pc.assets_in_period)::NUMERIC AS cumulative_deposits_through,
      SUM(pc.assets_out_period)::NUMERIC AS cumulative_redemptions_through
    FROM position_change_daily pc
    WHERE pc.bucket <= p_end_date
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
    GROUP BY pc.account_id, pc.term_id, pc.curve_id
  ),

  -- Calculate period activity (deposits and redemptions DURING the period)
  period_activity AS (
    SELECT
      pc.account_id,
      pc.term_id,
      pc.curve_id,
      SUM(pc.assets_in_period)::NUMERIC AS period_deposits,
      SUM(pc.assets_out_period)::NUMERIC AS period_redemptions,
      SUM(pc.shares_delta_period)::NUMERIC AS period_shares_delta
    FROM position_change_daily pc
    WHERE pc.bucket >= p_start_date
      AND pc.bucket <= p_end_date
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
    GROUP BY pc.account_id, pc.term_id, pc.curve_id
  ),

  -- Combine all position data with historical prices
  position_period_metrics AS (
    SELECT
      COALESCE(pss.account_id, pse.account_id, pa.account_id) AS account_id,
      COALESCE(pss.term_id, pse.term_id, pa.term_id) AS term_id,
      COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) AS curve_id,
      -- Shares at boundaries
      COALESCE(pss.shares_at_start, 0) AS shares_at_start,
      COALESCE(pse.shares_at_end, 0) AS shares_at_end,
      -- Period activity
      COALESCE(pa.period_deposits, 0) AS period_deposits,
      COALESCE(pa.period_redemptions, 0) AS period_redemptions,
      -- Historical prices
      COALESCE(ps.share_price, 0) AS price_at_start,
      COALESCE(pe.share_price, 0) AS price_at_end,
      -- Equity values at boundaries
      (COALESCE(pss.shares_at_start, 0) * COALESCE(ps.share_price, 0) / 1e18)::NUMERIC AS equity_at_start,
      (COALESCE(pse.shares_at_end, 0) * COALESCE(pe.share_price, 0) / 1e18)::NUMERIC AS equity_at_end,
      -- Was there any activity during the period?
      CASE WHEN pa.account_id IS NOT NULL THEN TRUE ELSE FALSE END AS had_activity
    FROM position_state_start pss
    FULL OUTER JOIN position_state_end pse
      ON pss.account_id = pse.account_id
      AND pss.term_id = pse.term_id
      AND pss.curve_id = pse.curve_id
    FULL OUTER JOIN period_activity pa
      ON COALESCE(pss.account_id, pse.account_id) = pa.account_id
      AND COALESCE(pss.term_id, pse.term_id) = pa.term_id
      AND COALESCE(pss.curve_id, pse.curve_id) = pa.curve_id
    LEFT JOIN prices_at_start ps
      ON COALESCE(pss.term_id, pse.term_id, pa.term_id) = ps.term_id
      AND COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) = ps.curve_id
    LEFT JOIN prices_at_end pe
      ON COALESCE(pss.term_id, pse.term_id, pa.term_id) = pe.term_id
      AND COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) = pe.curve_id
    WHERE COALESCE(pss.account_id, pse.account_id, pa.account_id) IN (SELECT account_id FROM active_accounts)
  ),

  -- Calculate period-specific PnL for each position
  position_pnl AS (
    SELECT
      ppm.*,
      -- Realized PnL during period = redemptions - deposits
      (ppm.period_redemptions - ppm.period_deposits)::NUMERIC AS period_realized_pnl,
      -- Unrealized PnL change = (equity at end) - (equity at start) - (net new investment during period)
      -- Net new investment = deposits - redemptions (what was added/removed)
      -- Unrealized change = equity_end - equity_start - (deposits - redemptions)
      (ppm.equity_at_end - ppm.equity_at_start - (ppm.period_deposits - ppm.period_redemptions))::NUMERIC AS period_unrealized_pnl,
      -- Total period PnL = realized + unrealized change
      -- Simplified: (equity_end - equity_start) + (redemptions - deposits) - (deposits - redemptions) + (redemptions - deposits)
      -- = equity_end - equity_start + 2*(redemptions - deposits) - (deposits - redemptions)
      -- Actually simpler: equity_end - equity_start + (net cash out)
      -- Period PnL = final equity - initial equity + cash withdrawn - cash deposited
      (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits)::NUMERIC AS period_total_pnl,
      -- Position was profitable if period PnL > 0
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits) > 0 THEN 1 ELSE 0 END AS is_winning,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits) < 0 THEN 1 ELSE 0 END AS is_losing
    FROM position_period_metrics ppm
  ),

  -- Aggregate by account
  account_metrics AS (
    SELECT
      pp.account_id,
      -- Position counts (positions with activity during period)
      COUNT(DISTINCT (pp.term_id, pp.curve_id)) FILTER (WHERE pp.had_activity) AS period_position_count,
      -- Active positions at end of period
      COUNT(DISTINCT (pp.term_id, pp.curve_id)) FILTER (WHERE pp.shares_at_end > 0) AS active_position_count,
      -- Win/loss counts based on period PnL
      SUM(pp.is_winning) AS winning_positions,
      SUM(pp.is_losing) AS losing_positions,
      -- Period volumes
      SUM(pp.period_deposits)::NUMERIC AS period_deposits_raw,
      SUM(pp.period_redemptions)::NUMERIC AS period_redemptions_raw,
      -- Period PnL values
      SUM(pp.period_total_pnl)::NUMERIC AS total_pnl_raw,
      SUM(pp.period_realized_pnl)::NUMERIC AS realized_pnl_raw,
      SUM(pp.period_unrealized_pnl)::NUMERIC AS unrealized_pnl_raw,
      -- Equity at period boundaries
      SUM(pp.equity_at_start)::NUMERIC AS equity_at_start,
      SUM(pp.equity_at_end)::NUMERIC AS equity_at_end,
      -- Best/worst trades during period
      MAX(pp.period_total_pnl) AS best_trade_pnl_raw,
      MIN(pp.period_total_pnl) AS worst_trade_pnl_raw
    FROM position_pnl pp
    GROUP BY pp.account_id
    HAVING COUNT(DISTINCT (pp.term_id, pp.curve_id)) FILTER (WHERE pp.had_activity) >= GREATEST(p_min_positions, 1)
      AND (SUM(pp.period_deposits) + SUM(pp.period_redemptions)) >= COALESCE(p_min_volume * 1e18, 0)
  ),

  -- Enrich with account info
  enriched AS (
    SELECT
      am.account_id,
      a.label AS account_label,
      a.image AS account_image,
      am.total_pnl_raw,
      COALESCE(am.realized_pnl_raw, 0) AS realized_pnl_raw,
      COALESCE(am.unrealized_pnl_raw, 0) AS unrealized_pnl_raw,
      -- PnL percentage: period PnL / equity at start (or deposits if no starting equity)
      COALESCE(
        (am.total_pnl_raw * 100.0 / NULLIF(
          CASE WHEN am.equity_at_start > 0 THEN am.equity_at_start
               ELSE am.period_deposits_raw - am.period_redemptions_raw
          END, 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS pnl_pct,
      am.total_pnl_raw AS pnl_change_raw,  -- For period queries, pnl_change = total_pnl
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
      -- Get first/last activity timestamps for period
      p_start_date AS first_position_at,  -- For period queries, use period boundaries
      p_end_date AS last_activity_at
    FROM account_metrics am
    JOIN account a ON am.account_id = a.id
    WHERE NOT p_exclude_protocol_accounts OR a.type NOT IN ('ProtocolVault', 'AtomWallet')
  ),

  -- Apply ranking
  ranked AS (
    SELECT
      e.*,
      CASE p_sort_by
        WHEN 'total_pnl' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_pnl_raw ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_pnl_raw DESC NULLS LAST) END
        WHEN 'pnl_pct' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.pnl_pct ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.pnl_pct DESC NULLS LAST) END
        WHEN 'win_rate' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.win_rate ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.win_rate DESC NULLS LAST) END
        WHEN 'total_volume' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_volume_raw ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_volume_raw DESC NULLS LAST) END
        WHEN 'position_count' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_position_count ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_position_count DESC NULLS LAST) END
        ELSE ROW_NUMBER() OVER (ORDER BY e.total_pnl_raw DESC NULLS LAST)
      END AS rank
    FROM enriched e
  )

  SELECT
    r.rank,
    r.account_id,
    r.account_label,
    r.account_image,
    -- PnL values: raw (wei) and formatted (ETH, 4 decimals)
    r.total_pnl_raw,
    ROUND(r.total_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS total_pnl_formatted,
    r.realized_pnl_raw,
    ROUND(r.realized_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS realized_pnl_formatted,
    r.unrealized_pnl_raw,
    ROUND(r.unrealized_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS unrealized_pnl_formatted,
    r.pnl_pct,
    r.pnl_change_raw,
    ROUND(r.pnl_change_raw / 1e18, 4)::NUMERIC(30, 4) AS pnl_change_formatted,
    -- Position counts
    r.total_position_count,
    r.active_position_count,
    r.winning_positions,
    r.losing_positions,
    r.win_rate,
    -- Volume values: raw (wei) and formatted (ETH, 4 decimals)
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
    -- Redeemable assets (not calculated for main leaderboard - use get_vault_leaderboard_period)
    NULL::NUMERIC AS redeemable_assets_raw,
    NULL::NUMERIC(30, 4) AS redeemable_assets_formatted,
    -- Timestamps (period boundaries for period queries)
    r.first_position_at,
    r.last_activity_at
  FROM ranked r
  ORDER BY r.rank
  LIMIT v_limit OFFSET v_offset;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION get_pnl_leaderboard_period IS 'Returns a period-specific leaderboard showing PnL metrics ONLY for the specified date range. Uses historical share prices for accurate valuations.';

-- ========================================
-- PERIOD-SPECIFIC VAULT LEADERBOARD
-- ========================================

CREATE OR REPLACE FUNCTION get_vault_leaderboard_period(
  p_term_id TEXT,
  p_start_date TIMESTAMPTZ,
  p_end_date TIMESTAMPTZ,
  p_curve_id NUMERIC(78, 0) DEFAULT NULL,
  p_limit INTEGER DEFAULT 100,
  p_offset INTEGER DEFAULT 0,
  p_sort_by TEXT DEFAULT 'total_pnl',
  p_sort_order TEXT DEFAULT 'DESC'
)
RETURNS SETOF pnl_leaderboard_entry AS $$
DECLARE
  v_start_timestamp BIGINT;
  v_end_timestamp BIGINT;
  v_limit INTEGER;
  v_offset INTEGER;
BEGIN
  -- Input validation
  IF p_term_id IS NULL OR p_term_id = '' THEN
    RAISE EXCEPTION 'p_term_id is required';
  END IF;

  IF p_start_date IS NULL OR p_end_date IS NULL THEN
    RAISE EXCEPTION 'p_start_date and p_end_date are required';
  END IF;

  IF p_start_date >= p_end_date THEN
    RAISE EXCEPTION 'p_start_date must be before p_end_date';
  END IF;

  -- Input validation for limits
  v_limit := LEAST(GREATEST(COALESCE(p_limit, 100), 1), 10000);
  v_offset := GREATEST(COALESCE(p_offset, 0), 0);

  -- Convert timestamps to Unix seconds for share_price_change lookup
  v_start_timestamp := EXTRACT(EPOCH FROM p_start_date)::BIGINT;
  v_end_timestamp := EXTRACT(EPOCH FROM p_end_date)::BIGINT;

  RETURN QUERY
  WITH
  -- Get accounts with activity in the period for this vault
  active_accounts AS (
    SELECT DISTINCT pc.account_id
    FROM position_change_daily pc
    WHERE pc.bucket >= p_start_date
      AND pc.bucket <= p_end_date
      AND pc.term_id = p_term_id
      AND (p_curve_id IS NULL OR pc.curve_id = p_curve_id)
  ),

  -- Get historical vault state at period START
  vault_state_start AS (
    SELECT DISTINCT ON (term_id, curve_id)
      term_id,
      curve_id,
      share_price,
      total_assets,
      total_shares
    FROM share_price_change
    WHERE term_id = p_term_id
      AND (p_curve_id IS NULL OR curve_id = p_curve_id)
      AND block_timestamp <= v_start_timestamp
    ORDER BY term_id, curve_id, block_timestamp DESC, log_index DESC
  ),

  -- Get historical vault state at period END
  vault_state_end AS (
    SELECT DISTINCT ON (term_id, curve_id)
      term_id,
      curve_id,
      share_price,
      total_assets,
      total_shares
    FROM share_price_change
    WHERE term_id = p_term_id
      AND (p_curve_id IS NULL OR curve_id = p_curve_id)
      AND block_timestamp <= v_end_timestamp
    ORDER BY term_id, curve_id, block_timestamp DESC, log_index DESC
  ),

  -- Calculate position state at period START
  position_state_start AS (
    SELECT
      pc.account_id,
      pc.term_id,
      pc.curve_id,
      SUM(pc.shares_delta_period)::NUMERIC AS shares_at_start,
      SUM(pc.assets_in_period)::NUMERIC AS cumulative_deposits_before,
      SUM(pc.assets_out_period)::NUMERIC AS cumulative_redemptions_before
    FROM position_change_daily pc
    WHERE pc.bucket < p_start_date
      AND pc.term_id = p_term_id
      AND (p_curve_id IS NULL OR pc.curve_id = p_curve_id)
    GROUP BY pc.account_id, pc.term_id, pc.curve_id
  ),

  -- Calculate position state at period END
  position_state_end AS (
    SELECT
      pc.account_id,
      pc.term_id,
      pc.curve_id,
      SUM(pc.shares_delta_period)::NUMERIC AS shares_at_end,
      SUM(pc.assets_in_period)::NUMERIC AS cumulative_deposits_through,
      SUM(pc.assets_out_period)::NUMERIC AS cumulative_redemptions_through
    FROM position_change_daily pc
    WHERE pc.bucket <= p_end_date
      AND pc.term_id = p_term_id
      AND (p_curve_id IS NULL OR pc.curve_id = p_curve_id)
    GROUP BY pc.account_id, pc.term_id, pc.curve_id
  ),

  -- Calculate period activity
  period_activity AS (
    SELECT
      pc.account_id,
      pc.term_id,
      pc.curve_id,
      SUM(pc.assets_in_period)::NUMERIC AS period_deposits,
      SUM(pc.assets_out_period)::NUMERIC AS period_redemptions,
      SUM(pc.shares_delta_period)::NUMERIC AS period_shares_delta
    FROM position_change_daily pc
    WHERE pc.bucket >= p_start_date
      AND pc.bucket <= p_end_date
      AND pc.term_id = p_term_id
      AND (p_curve_id IS NULL OR pc.curve_id = p_curve_id)
    GROUP BY pc.account_id, pc.term_id, pc.curve_id
  ),

  -- Combine position data with historical prices and vault states
  position_period_metrics AS (
    SELECT
      COALESCE(pss.account_id, pse.account_id, pa.account_id) AS account_id,
      COALESCE(pss.term_id, pse.term_id, pa.term_id) AS term_id,
      COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) AS curve_id,
      -- Shares at boundaries
      COALESCE(pss.shares_at_start, 0) AS shares_at_start,
      COALESCE(pse.shares_at_end, 0) AS shares_at_end,
      -- Period activity
      COALESCE(pa.period_deposits, 0) AS period_deposits,
      COALESCE(pa.period_redemptions, 0) AS period_redemptions,
      -- Historical prices
      COALESCE(vs.share_price, 0) AS price_at_start,
      COALESCE(ve.share_price, 0) AS price_at_end,
      -- Vault states for redeemable_assets calculation
      COALESCE(ve.total_assets, 0) AS vault_total_assets_end,
      COALESCE(ve.total_shares, 0) AS vault_total_shares_end,
      -- Equity values at boundaries
      (COALESCE(pss.shares_at_start, 0) * COALESCE(vs.share_price, 0) / 1e18)::NUMERIC AS equity_at_start,
      (COALESCE(pse.shares_at_end, 0) * COALESCE(ve.share_price, 0) / 1e18)::NUMERIC AS equity_at_end,
      -- Had activity during period?
      CASE WHEN pa.account_id IS NOT NULL THEN TRUE ELSE FALSE END AS had_activity
    FROM position_state_start pss
    FULL OUTER JOIN position_state_end pse
      ON pss.account_id = pse.account_id
      AND pss.term_id = pse.term_id
      AND pss.curve_id = pse.curve_id
    FULL OUTER JOIN period_activity pa
      ON COALESCE(pss.account_id, pse.account_id) = pa.account_id
      AND COALESCE(pss.term_id, pse.term_id) = pa.term_id
      AND COALESCE(pss.curve_id, pse.curve_id) = pa.curve_id
    LEFT JOIN vault_state_start vs
      ON COALESCE(pss.term_id, pse.term_id, pa.term_id) = vs.term_id
      AND COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) = vs.curve_id
    LEFT JOIN vault_state_end ve
      ON COALESCE(pss.term_id, pse.term_id, pa.term_id) = ve.term_id
      AND COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) = ve.curve_id
    WHERE COALESCE(pss.account_id, pse.account_id, pa.account_id) IN (SELECT account_id FROM active_accounts)
  ),

  -- Calculate period PnL and redeemable assets at period end
  position_pnl AS (
    SELECT
      ppm.*,
      -- Period PnL calculations
      (ppm.period_redemptions - ppm.period_deposits)::NUMERIC AS period_realized_pnl,
      (ppm.equity_at_end - ppm.equity_at_start - (ppm.period_deposits - ppm.period_redemptions))::NUMERIC AS period_unrealized_pnl,
      (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits)::NUMERIC AS period_total_pnl,
      -- Win/loss determination
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits) > 0 THEN 1 ELSE 0 END AS is_winning,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits) < 0 THEN 1 ELSE 0 END AS is_losing,
      -- Calculate redeemable assets at period end using historical vault state
      -- Uses the same bonding curve math as get_vault_leaderboard
      CASE
        WHEN ppm.shares_at_end = 0 THEN 0::NUMERIC
        -- Linear curve (curve_id = 1)
        WHEN ppm.curve_id = 1 THEN
          CASE WHEN ppm.vault_total_shares_end > 0
            THEN (ppm.shares_at_end * ppm.vault_total_assets_end / ppm.vault_total_shares_end)::NUMERIC
            ELSE 0::NUMERIC
          END
        -- Offset Progressive curve (curve_id = 2)
        WHEN ppm.curve_id = 2 THEN
          (
            SELECT (
              (
                ((ppm.vault_total_shares_end + 30000000000000000000::NUMERIC) * (ppm.vault_total_shares_end + 30000000000000000000::NUMERIC) / 1e18)::NUMERIC
                -
                (((ppm.vault_total_shares_end + 30000000000000000000::NUMERIC - ppm.shares_at_end) * (ppm.vault_total_shares_end + 30000000000000000000::NUMERIC - ppm.shares_at_end) + 1e18 - 1) / 1e18)::NUMERIC
              )
              * 50000000000000000::NUMERIC / 1e18
            )::NUMERIC
          )
        ELSE 0::NUMERIC
      END AS raw_assets_from_curve
    FROM position_period_metrics ppm
  ),

  -- Apply fees to get redeemable assets
  position_with_fees AS (
    SELECT
      pp.*,
      -- Protocol fee (1.25%, rounds up)
      ((pp.raw_assets_from_curve * 125 + 9999) / 10000)::NUMERIC AS protocol_fee,
      -- Exit fee (0.75%, rounds up)
      ((pp.raw_assets_from_curve * 75 + 9999) / 10000)::NUMERIC AS exit_fee
    FROM position_pnl pp
  ),

  position_final AS (
    SELECT
      pwf.*,
      GREATEST(pwf.raw_assets_from_curve - pwf.protocol_fee - pwf.exit_fee, 0)::NUMERIC AS redeemable_assets_raw
    FROM position_with_fees pwf
  ),

  -- Aggregate by account
  account_metrics AS (
    SELECT
      pf.account_id,
      COUNT(DISTINCT (pf.term_id, pf.curve_id)) FILTER (WHERE pf.had_activity) AS period_position_count,
      COUNT(DISTINCT (pf.term_id, pf.curve_id)) FILTER (WHERE pf.shares_at_end > 0) AS active_position_count,
      SUM(pf.is_winning) AS winning_positions,
      SUM(pf.is_losing) AS losing_positions,
      SUM(pf.period_deposits)::NUMERIC AS period_deposits_raw,
      SUM(pf.period_redemptions)::NUMERIC AS period_redemptions_raw,
      SUM(pf.period_total_pnl)::NUMERIC AS total_pnl_raw,
      SUM(pf.period_realized_pnl)::NUMERIC AS realized_pnl_raw,
      SUM(pf.period_unrealized_pnl)::NUMERIC AS unrealized_pnl_raw,
      SUM(pf.equity_at_start)::NUMERIC AS equity_at_start,
      SUM(pf.equity_at_end)::NUMERIC AS equity_at_end,
      MAX(pf.period_total_pnl) AS best_trade_pnl_raw,
      MIN(pf.period_total_pnl) AS worst_trade_pnl_raw,
      SUM(pf.redeemable_assets_raw) FILTER (WHERE pf.shares_at_end > 0)::NUMERIC AS redeemable_assets_raw
    FROM position_final pf
    GROUP BY pf.account_id
  ),

  -- Enrich with account info
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
          CASE WHEN am.equity_at_start > 0 THEN am.equity_at_start
               ELSE am.period_deposits_raw - am.period_redemptions_raw
          END, 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS pnl_pct,
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
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION get_vault_leaderboard_period IS 'Returns a period-specific leaderboard for a specific vault showing PnL metrics and redeemable_assets calculated at the end of the specified date range using historical share prices.';

-- ========================================
-- INDEXES FOR PERFORMANCE
-- ========================================

-- Index for efficient historical price lookups
CREATE INDEX IF NOT EXISTS idx_share_price_change_term_curve_block_timestamp
  ON share_price_change (term_id, curve_id, block_timestamp DESC, log_index DESC);
