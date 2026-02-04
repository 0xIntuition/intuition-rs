-- PnL Leaderboard Functions
-- Season 2 Leaderboard feature for ranking accounts by PnL metrics
-- All monetary values are returned in two formats:
--   *_raw: Base-10 integer (wei, 18 decimals) for precise calculations
--   *_formatted: Human-readable decimal (ETH, max 4 decimal places) for display

-- ========================================
-- RETURN TYPE TABLES FOR HASURA TRACKING
-- ========================================

-- Drop and recreate to update schema
DROP TABLE IF EXISTS pnl_leaderboard_entry CASCADE;
DROP TABLE IF EXISTS account_pnl_rank CASCADE;
DROP TABLE IF EXISTS pnl_leaderboard_stats CASCADE;

CREATE TABLE pnl_leaderboard_entry (
  rank BIGINT,
  account_id TEXT,
  account_label TEXT,
  account_image TEXT,
  -- PnL values (raw = wei, formatted = ETH with 4 decimals)
  total_pnl_raw NUMERIC,
  total_pnl_formatted NUMERIC(30, 4),
  realized_pnl_raw NUMERIC,
  realized_pnl_formatted NUMERIC(30, 4),
  unrealized_pnl_raw NUMERIC,
  unrealized_pnl_formatted NUMERIC(30, 4),
  pnl_pct NUMERIC(20, 4),
  pnl_change_raw NUMERIC,
  pnl_change_formatted NUMERIC(30, 4),
  -- Position counts
  total_position_count BIGINT,
  active_position_count BIGINT,
  winning_positions BIGINT,
  losing_positions BIGINT,
  win_rate NUMERIC(10, 2),
  -- Volume values (raw = wei, formatted = ETH with 4 decimals)
  total_deposits_raw NUMERIC,
  total_deposits_formatted NUMERIC(30, 4),
  total_redemptions_raw NUMERIC,
  total_redemptions_formatted NUMERIC(30, 4),
  total_volume_raw NUMERIC,
  total_volume_formatted NUMERIC(30, 4),
  current_equity_value_raw NUMERIC,
  current_equity_value_formatted NUMERIC(30, 4),
  best_trade_pnl_raw NUMERIC,
  best_trade_pnl_formatted NUMERIC(30, 4),
  worst_trade_pnl_raw NUMERIC,
  worst_trade_pnl_formatted NUMERIC(30, 4),
  -- Redeemable assets (previewRedeem value after fees)
  redeemable_assets_raw NUMERIC,
  redeemable_assets_formatted NUMERIC(30, 4),
  -- Timestamps
  first_position_at TIMESTAMPTZ,
  last_activity_at TIMESTAMPTZ
);

CREATE TABLE account_pnl_rank (
  rank BIGINT,
  total_accounts BIGINT,
  percentile NUMERIC(10, 4),
  account_id TEXT,
  account_label TEXT,
  account_image TEXT,
  total_pnl_raw NUMERIC,
  total_pnl_formatted NUMERIC(30, 4),
  pnl_pct NUMERIC(20, 4),
  win_rate NUMERIC(10, 2),
  total_position_count BIGINT,
  total_volume_raw NUMERIC,
  total_volume_formatted NUMERIC(30, 4)
);

CREATE TABLE pnl_leaderboard_stats (
  total_traders BIGINT,
  total_pnl_sum_raw NUMERIC,
  total_pnl_sum_formatted NUMERIC(30, 4),
  avg_pnl_raw NUMERIC,
  avg_pnl_formatted NUMERIC(30, 4),
  median_pnl_raw NUMERIC,
  median_pnl_formatted NUMERIC(30, 4),
  total_volume_raw NUMERIC,
  total_volume_formatted NUMERIC(30, 4),
  avg_volume_raw NUMERIC,
  avg_volume_formatted NUMERIC(30, 4),
  profitable_traders BIGINT,
  unprofitable_traders BIGINT,
  profitable_pct NUMERIC(10, 2)
);

-- ========================================
-- DROP EXISTING FUNCTIONS (if any)
-- Required because CREATE OR REPLACE cannot change return types
-- ========================================

DROP FUNCTION IF EXISTS get_pnl_leaderboard(INTEGER, INTEGER, TEXT, TIMESTAMPTZ, TIMESTAMPTZ, TEXT, TEXT, BOOLEAN, INTEGER, NUMERIC, TEXT);
DROP FUNCTION IF EXISTS get_account_pnl_rank(TEXT, TEXT, TEXT, TEXT);
DROP FUNCTION IF EXISTS get_pnl_leaderboard_stats(TEXT, TEXT);
DROP FUNCTION IF EXISTS get_vault_leaderboard(TEXT, NUMERIC, INTEGER, INTEGER, TEXT, TEXT);

-- ========================================
-- MAIN LEADERBOARD FUNCTION (OPTIMIZED)
-- ========================================

CREATE OR REPLACE FUNCTION get_pnl_leaderboard(
  p_limit INTEGER DEFAULT 100,
  p_offset INTEGER DEFAULT 0,
  p_time_filter TEXT DEFAULT 'all_time',
  p_start_time TIMESTAMPTZ DEFAULT NULL,
  p_end_time TIMESTAMPTZ DEFAULT NULL,
  p_sort_by TEXT DEFAULT 'total_pnl',
  p_sort_order TEXT DEFAULT 'DESC',
  p_exclude_protocol_accounts BOOLEAN DEFAULT TRUE,
  p_min_positions INTEGER DEFAULT 1,
  p_min_volume NUMERIC DEFAULT 0,
  p_term_id TEXT DEFAULT NULL
)
RETURNS SETOF pnl_leaderboard_entry AS $$
DECLARE
  v_start_time TIMESTAMPTZ;
  v_end_time TIMESTAMPTZ;
  v_limit INTEGER;
  v_offset INTEGER;
BEGIN
  -- Input validation for limits (Must Fix: Add input validation for limits)
  v_limit := LEAST(GREATEST(COALESCE(p_limit, 100), 1), 10000);  -- Clamp between 1 and 10000
  v_offset := GREATEST(COALESCE(p_offset, 0), 0);  -- Ensure non-negative

  v_end_time := COALESCE(p_end_time, NOW());

  CASE p_time_filter
    WHEN '24h' THEN v_start_time := v_end_time - INTERVAL '24 hours';
    WHEN '7d' THEN v_start_time := v_end_time - INTERVAL '7 days';
    WHEN '30d' THEN v_start_time := v_end_time - INTERVAL '30 days';
    WHEN '90d' THEN v_start_time := v_end_time - INTERVAL '90 days';
    WHEN 'custom' THEN v_start_time := COALESCE(p_start_time, '1970-01-01'::TIMESTAMPTZ);
    ELSE v_start_time := '1970-01-01'::TIMESTAMPTZ;
  END CASE;

  RETURN QUERY
  WITH
  -- First get accounts with activity in time window (FAST - uses continuous aggregate)
  active_accounts AS (
    SELECT DISTINCT pc.account_id
    FROM position_change_daily pc
    WHERE pc.bucket >= v_start_time
      AND pc.bucket <= v_end_time
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
  ),
  -- Calculate period PnL change from position_change_daily (for most_improved sorting)
  -- Keep raw values (wei)
  period_pnl_change AS (
    SELECT
      pc.account_id,
      SUM(COALESCE(pc.assets_out_period, 0) - COALESCE(pc.assets_in_period, 0))::NUMERIC AS period_realized_change_raw
    FROM position_change_daily pc
    WHERE pc.bucket >= v_start_time
      AND pc.bucket <= v_end_time
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
    GROUP BY pc.account_id
  ),
  -- Only get positions for active accounts (OPTIMIZED - avoids scanning all 3M+ positions)
  -- All monetary values kept as raw (wei)
  current_positions AS (
    SELECT
      p.account_id,
      p.term_id,
      p.curve_id,
      p.shares,
      p.total_deposit_assets_after_total_fees AS total_deposits_raw,
      p.total_redeem_assets_for_receiver AS total_redemptions_raw,
      p.created_at AS position_created_at,
      p.updated_at AS position_updated_at,
      v.current_share_price,
      (p.shares * v.current_share_price / 1e18)::NUMERIC AS equity_value_raw,
      ((p.shares * v.current_share_price / 1e18) + p.total_redeem_assets_for_receiver - p.total_deposit_assets_after_total_fees)::NUMERIC AS position_pnl_raw
    FROM position p
    JOIN vault v ON p.term_id = v.term_id AND p.curve_id = v.curve_id
    WHERE (p_time_filter = 'all_time' OR p.account_id IN (SELECT account_id FROM active_accounts))
      AND (p_term_id IS NULL OR p.term_id = p_term_id)
  ),
  account_metrics AS (
    SELECT
      cp.account_id,
      COUNT(DISTINCT (cp.term_id, cp.curve_id)) AS total_position_count,
      COUNT(DISTINCT (cp.term_id, cp.curve_id)) FILTER (WHERE cp.shares > 0) AS active_position_count,
      COUNT(*) FILTER (WHERE cp.position_pnl_raw > 0) AS winning_positions,
      COUNT(*) FILTER (WHERE cp.position_pnl_raw < 0) AS losing_positions,
      SUM(cp.total_deposits_raw)::NUMERIC AS total_deposits_raw,
      SUM(cp.total_redemptions_raw)::NUMERIC AS total_redemptions_raw,
      SUM(cp.equity_value_raw)::NUMERIC AS current_equity_value_raw,
      SUM(cp.position_pnl_raw)::NUMERIC AS total_pnl_raw,
      SUM(cp.position_pnl_raw) FILTER (WHERE cp.shares = 0)::NUMERIC AS realized_pnl_raw,
      SUM(cp.position_pnl_raw) FILTER (WHERE cp.shares > 0)::NUMERIC AS unrealized_pnl_raw,
      MAX(cp.position_pnl_raw) AS best_trade_pnl_raw,
      MIN(cp.position_pnl_raw) AS worst_trade_pnl_raw,
      MIN(cp.position_created_at) AS first_position_at,
      MAX(cp.position_updated_at) AS last_activity_at
    FROM current_positions cp
    GROUP BY cp.account_id
    HAVING COUNT(DISTINCT (cp.term_id, cp.curve_id)) >= GREATEST(p_min_positions, 1)
      AND (SUM(cp.total_deposits_raw) + SUM(cp.total_redemptions_raw)) >= COALESCE(p_min_volume * 1e18, 0)
  ),
  enriched AS (
    SELECT
      am.account_id,
      a.label AS account_label,
      a.image AS account_image,
      am.total_pnl_raw,
      COALESCE(am.realized_pnl_raw, 0) AS realized_pnl_raw,
      COALESCE(am.unrealized_pnl_raw, 0) AS unrealized_pnl_raw,
      -- PnL percentage (works on raw values since ratio cancels out)
      COALESCE(
        (am.total_pnl_raw * 100.0 / NULLIF(am.total_deposits_raw - am.total_redemptions_raw, 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS pnl_pct,
      -- pnl_change: For time-filtered queries, use period change; for all_time, use total_pnl
      CASE
        WHEN p_time_filter = 'all_time' THEN am.total_pnl_raw
        ELSE COALESCE(ppc.period_realized_change_raw, 0)
      END AS pnl_change_raw,
      am.total_position_count,
      am.active_position_count,
      am.winning_positions,
      am.losing_positions,
      COALESCE(
        (am.winning_positions * 100.0 / NULLIF(am.total_position_count, 0))::NUMERIC(10, 2),
        0::NUMERIC(10, 2)
      ) AS win_rate,
      am.total_deposits_raw,
      am.total_redemptions_raw,
      (am.total_deposits_raw + am.total_redemptions_raw)::NUMERIC AS total_volume_raw,
      am.current_equity_value_raw,
      am.best_trade_pnl_raw,
      am.worst_trade_pnl_raw,
      am.first_position_at,
      am.last_activity_at
    FROM account_metrics am
    JOIN account a ON am.account_id = a.id
    LEFT JOIN period_pnl_change ppc ON am.account_id = ppc.account_id
    WHERE NOT p_exclude_protocol_accounts OR a.type NOT IN ('ProtocolVault', 'AtomWallet')
  ),
  ranked AS (
    SELECT e.*,
      CASE p_sort_by
        WHEN 'total_pnl' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_pnl_raw ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_pnl_raw DESC NULLS LAST) END
        WHEN 'pnl_pct' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.pnl_pct ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.pnl_pct DESC NULLS LAST) END
        WHEN 'win_rate' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.win_rate ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.win_rate DESC NULLS LAST) END
        WHEN 'total_volume' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_volume_raw ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_volume_raw DESC NULLS LAST) END
        WHEN 'position_count' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_position_count ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_position_count DESC NULLS LAST) END
        WHEN 'newest' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.first_position_at ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.first_position_at DESC NULLS LAST) END
        WHEN 'most_improved' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.pnl_change_raw ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.pnl_change_raw DESC NULLS LAST) END
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
    -- Redeemable assets (not calculated for main leaderboard - use get_vault_leaderboard for this)
    NULL::NUMERIC AS redeemable_assets_raw,
    NULL::NUMERIC(30, 4) AS redeemable_assets_formatted,
    -- Timestamps
    r.first_position_at,
    r.last_activity_at
  FROM ranked r
  ORDER BY r.rank
  LIMIT v_limit OFFSET v_offset;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION get_pnl_leaderboard IS 'Returns a ranked leaderboard of accounts by PnL metrics with configurable time filters and sorting options (total_pnl, pnl_pct, win_rate, total_volume, position_count, newest, most_improved).';

-- ========================================
-- ACCOUNT RANK HELPER FUNCTION
-- ========================================

CREATE OR REPLACE FUNCTION get_account_pnl_rank(
  p_account_id TEXT,
  p_sort_by TEXT DEFAULT 'total_pnl',
  p_time_filter TEXT DEFAULT 'all_time',
  p_term_id TEXT DEFAULT NULL
)
RETURNS SETOF account_pnl_rank AS $$
BEGIN
  -- Input validation
  IF p_account_id IS NULL OR p_account_id = '' THEN
    RETURN;
  END IF;

  RETURN QUERY
  WITH leaderboard AS (
    SELECT
      l.*,
      COUNT(*) OVER () AS total_accounts
    FROM get_pnl_leaderboard(
      p_limit := 10000,
      p_offset := 0,
      p_time_filter := p_time_filter,
      p_sort_by := p_sort_by,
      p_term_id := p_term_id
    ) l
  )
  SELECT
    lb.rank,
    lb.total_accounts,
    COALESCE(
      ((lb.total_accounts - lb.rank + 1) * 100.0 / NULLIF(lb.total_accounts, 0))::NUMERIC(10, 4),
      0::NUMERIC(10, 4)
    ) AS percentile,
    lb.account_id,
    lb.account_label,
    lb.account_image,
    lb.total_pnl_raw,
    lb.total_pnl_formatted,
    lb.pnl_pct,
    lb.win_rate,
    lb.total_position_count,
    lb.total_volume_raw,
    lb.total_volume_formatted
  FROM leaderboard lb
  WHERE lb.account_id = p_account_id;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION get_account_pnl_rank IS 'Returns a specific account''s rank, percentile, and key metrics in the PnL leaderboard.';

-- ========================================
-- LEADERBOARD STATS FUNCTION
-- ========================================

CREATE OR REPLACE FUNCTION get_pnl_leaderboard_stats(
  p_time_filter TEXT DEFAULT 'all_time',
  p_term_id TEXT DEFAULT NULL
)
RETURNS SETOF pnl_leaderboard_stats AS $$
BEGIN
  RETURN QUERY
  SELECT
    COALESCE(COUNT(*), 0)::BIGINT AS total_traders,
    -- Total PnL sum
    COALESCE(SUM(l.total_pnl_raw), 0) AS total_pnl_sum_raw,
    ROUND(COALESCE(SUM(l.total_pnl_raw), 0) / 1e18, 4)::NUMERIC(30, 4) AS total_pnl_sum_formatted,
    -- Average PnL
    COALESCE(AVG(l.total_pnl_raw), 0) AS avg_pnl_raw,
    ROUND(COALESCE(AVG(l.total_pnl_raw), 0) / 1e18, 4)::NUMERIC(30, 4) AS avg_pnl_formatted,
    -- Median PnL
    COALESCE(PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY l.total_pnl_raw)::NUMERIC, 0) AS median_pnl_raw,
    ROUND(COALESCE(PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY l.total_pnl_raw)::NUMERIC, 0) / 1e18, 4)::NUMERIC(30, 4) AS median_pnl_formatted,
    -- Total volume
    COALESCE(SUM(l.total_volume_raw), 0) AS total_volume_raw,
    ROUND(COALESCE(SUM(l.total_volume_raw), 0) / 1e18, 4)::NUMERIC(30, 4) AS total_volume_formatted,
    -- Average volume
    COALESCE(AVG(l.total_volume_raw), 0) AS avg_volume_raw,
    ROUND(COALESCE(AVG(l.total_volume_raw), 0) / 1e18, 4)::NUMERIC(30, 4) AS avg_volume_formatted,
    -- Profitability stats
    COALESCE(COUNT(*) FILTER (WHERE l.total_pnl_raw > 0), 0)::BIGINT AS profitable_traders,
    COALESCE(COUNT(*) FILTER (WHERE l.total_pnl_raw <= 0), 0)::BIGINT AS unprofitable_traders,
    COALESCE(
      (COUNT(*) FILTER (WHERE l.total_pnl_raw > 0) * 100.0 / NULLIF(COUNT(*), 0))::NUMERIC(10, 2),
      0::NUMERIC(10, 2)
    ) AS profitable_pct
  FROM get_pnl_leaderboard(
    p_limit := 10000,
    p_offset := 0,
    p_time_filter := p_time_filter,
    p_term_id := p_term_id
  ) l;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION get_pnl_leaderboard_stats IS 'Returns aggregate statistics for the PnL leaderboard including total traders, average PnL, median PnL, and profitability ratios.';

-- ========================================
-- VAULT-SPECIFIC LEADERBOARD FUNCTION
-- ========================================

CREATE OR REPLACE FUNCTION get_vault_leaderboard(
  p_term_id TEXT,
  p_curve_id NUMERIC(78, 0) DEFAULT NULL,
  p_limit INTEGER DEFAULT 100,
  p_offset INTEGER DEFAULT 0,
  p_sort_by TEXT DEFAULT 'total_pnl',
  p_sort_order TEXT DEFAULT 'DESC'
)
RETURNS SETOF pnl_leaderboard_entry AS $$
DECLARE
  v_limit INTEGER;
  v_offset INTEGER;
BEGIN
  -- Input validation
  IF p_term_id IS NULL OR p_term_id = '' THEN
    RETURN;
  END IF;

  -- Input validation for limits
  v_limit := LEAST(GREATEST(COALESCE(p_limit, 100), 1), 10000);
  v_offset := GREATEST(COALESCE(p_offset, 0), 0);

  RETURN QUERY
  -- All monetary values kept as raw (wei)
  -- Curve constants (hardcoded, immutable in contracts)
  -- SLOPE = 1e17, OFFSET = 3e19, HALF_SLOPE = 5e16
  -- Fee constants: PROTOCOL_FEE = 125 (1.25%), EXIT_FEE = 75 (0.75%), FEE_DENOMINATOR = 10000
  WITH vault_positions AS (
    SELECT
      p.account_id,
      p.term_id,
      p.curve_id,
      p.shares,
      p.total_deposit_assets_after_total_fees AS total_deposits_raw,
      p.total_redeem_assets_for_receiver AS total_redemptions_raw,
      p.created_at AS position_created_at,
      p.updated_at AS position_updated_at,
      v.current_share_price,
      v.total_shares AS vault_total_shares,
      v.total_assets AS vault_total_assets,
      (p.shares * v.current_share_price / 1e18)::NUMERIC AS equity_value_raw,
      ((p.shares * v.current_share_price / 1e18)
        + p.total_redeem_assets_for_receiver
        - p.total_deposit_assets_after_total_fees)::NUMERIC AS position_pnl_raw,
      -- Calculate raw assets from curve (before fees)
      CASE
        WHEN p.shares = 0 THEN 0::NUMERIC
        -- Linear curve (curve_id = 1): rawAssets = shares * totalAssets / totalShares
        WHEN p.curve_id = 1 THEN
          CASE WHEN v.total_shares > 0
            THEN (p.shares * v.total_assets / v.total_shares)::NUMERIC
            ELSE 0::NUMERIC
          END
        -- Offset Progressive curve (curve_id = 2): Quadratic bonding curve
        -- s = totalShares + OFFSET, sNext = s - shares
        -- area = (s^2 - sNext^2) / 1e18, rawAssets = area * HALF_SLOPE / 1e18
        WHEN p.curve_id = 2 THEN
          (
            SELECT (
              (
                -- sSquared (rounds down)
                ((v.total_shares + 30000000000000000000::NUMERIC) * (v.total_shares + 30000000000000000000::NUMERIC) / 1e18)::NUMERIC
                -
                -- sNextSquared (rounds up)
                (((v.total_shares + 30000000000000000000::NUMERIC - p.shares) * (v.total_shares + 30000000000000000000::NUMERIC - p.shares) + 1e18 - 1) / 1e18)::NUMERIC
              )
              * 50000000000000000::NUMERIC / 1e18  -- HALF_SLOPE
            )::NUMERIC
          )
        ELSE 0::NUMERIC
      END AS raw_assets_from_curve
    FROM position p
    JOIN vault v ON p.term_id = v.term_id AND p.curve_id = v.curve_id
    WHERE p.term_id = p_term_id
      AND (p_curve_id IS NULL OR p.curve_id = p_curve_id)
  ),
  -- Apply fees to get redeemable assets
  vault_positions_with_fees AS (
    SELECT
      vp.*,
      -- Protocol fee (1.25%, rounds up): (rawAssets * 125 + 9999) / 10000
      ((vp.raw_assets_from_curve * 125 + 9999) / 10000)::NUMERIC AS protocol_fee,
      -- Exit fee (0.75%, rounds up): (rawAssets * 75 + 9999) / 10000
      -- Note: Exit fee is conditional on default vault having >= 1e18 shares, but we conservatively always apply it
      ((vp.raw_assets_from_curve * 75 + 9999) / 10000)::NUMERIC AS exit_fee
    FROM vault_positions vp
  ),
  vault_positions_final AS (
    SELECT
      vpf.*,
      -- redeemable_assets = rawAssets - protocolFee - exitFee
      GREATEST(vpf.raw_assets_from_curve - vpf.protocol_fee - vpf.exit_fee, 0)::NUMERIC AS redeemable_assets_raw
    FROM vault_positions_with_fees vpf
  ),
  account_metrics AS (
    SELECT
      vp.account_id,
      COUNT(*) AS total_position_count,
      COUNT(*) FILTER (WHERE vp.shares > 0) AS active_position_count,
      COUNT(*) FILTER (WHERE vp.position_pnl_raw > 0) AS winning_positions,
      COUNT(*) FILTER (WHERE vp.position_pnl_raw < 0) AS losing_positions,
      SUM(vp.total_deposits_raw)::NUMERIC AS total_deposits_raw,
      SUM(vp.total_redemptions_raw)::NUMERIC AS total_redemptions_raw,
      SUM(vp.equity_value_raw)::NUMERIC AS current_equity_value_raw,
      SUM(vp.position_pnl_raw)::NUMERIC AS total_pnl_raw,
      SUM(vp.position_pnl_raw) FILTER (WHERE vp.shares = 0)::NUMERIC AS realized_pnl_raw,
      SUM(vp.position_pnl_raw) FILTER (WHERE vp.shares > 0)::NUMERIC AS unrealized_pnl_raw,
      MAX(vp.position_pnl_raw) AS best_trade_pnl_raw,
      MIN(vp.position_pnl_raw) AS worst_trade_pnl_raw,
      -- Sum of redeemable assets for all open positions (shares > 0)
      SUM(vp.redeemable_assets_raw) FILTER (WHERE vp.shares > 0)::NUMERIC AS redeemable_assets_raw,
      MIN(vp.position_created_at) AS first_position_at,
      MAX(vp.position_updated_at) AS last_activity_at
    FROM vault_positions_final vp
    GROUP BY vp.account_id
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
        (am.total_pnl_raw * 100.0 / NULLIF(am.total_deposits_raw - am.total_redemptions_raw, 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS pnl_pct,
      am.total_pnl_raw AS pnl_change_raw,
      am.total_position_count,
      am.active_position_count,
      am.winning_positions,
      am.losing_positions,
      COALESCE(
        (am.winning_positions * 100.0 / NULLIF(am.total_position_count, 0))::NUMERIC(10, 2),
        0::NUMERIC(10, 2)
      ) AS win_rate,
      am.total_deposits_raw,
      am.total_redemptions_raw,
      (am.total_deposits_raw + am.total_redemptions_raw)::NUMERIC AS total_volume_raw,
      am.current_equity_value_raw,
      am.best_trade_pnl_raw,
      am.worst_trade_pnl_raw,
      COALESCE(am.redeemable_assets_raw, 0) AS redeemable_assets_raw,
      am.first_position_at,
      am.last_activity_at
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
        WHEN 'newest' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.first_position_at ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.first_position_at DESC NULLS LAST) END
        WHEN 'most_improved' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.pnl_change_raw ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.pnl_change_raw DESC NULLS LAST) END
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
    -- Redeemable assets (previewRedeem value after fees)
    r.redeemable_assets_raw,
    ROUND(r.redeemable_assets_raw / 1e18, 4)::NUMERIC(30, 4) AS redeemable_assets_formatted,
    -- Timestamps
    r.first_position_at,
    r.last_activity_at
  FROM ranked r
  ORDER BY r.rank
  LIMIT v_limit OFFSET v_offset;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION get_vault_leaderboard IS 'Returns a leaderboard for a specific vault (term_id) with optional curve_id filter, sorting options, and redeemable_assets calculated using previewRedeem math with fees.';

-- ========================================
-- INDEXES FOR PERFORMANCE
-- ========================================

-- Index on position for account aggregation
CREATE INDEX IF NOT EXISTS idx_position_account_shares
  ON position(account_id)
  WHERE shares > 0;

-- Composite index for vault joins
CREATE INDEX IF NOT EXISTS idx_position_vault_account
  ON position(term_id, curve_id, account_id);
