-- PnL Leaderboard Functions
-- Season 2 Leaderboard feature for ranking accounts by PnL metrics

-- ========================================
-- RETURN TYPE TABLES FOR HASURA TRACKING
-- ========================================

CREATE TABLE IF NOT EXISTS pnl_leaderboard_entry (
  rank BIGINT,
  account_id TEXT,
  account_label TEXT,
  account_image TEXT,
  total_pnl NUMERIC,
  realized_pnl NUMERIC,
  unrealized_pnl NUMERIC,
  pnl_pct NUMERIC(20, 4),
  pnl_change NUMERIC,
  total_position_count BIGINT,
  active_position_count BIGINT,
  winning_positions BIGINT,
  losing_positions BIGINT,
  win_rate NUMERIC(10, 2),
  total_deposits NUMERIC,
  total_redemptions NUMERIC,
  total_volume NUMERIC,
  current_equity_value NUMERIC,
  best_trade_pnl NUMERIC,
  worst_trade_pnl NUMERIC,
  first_position_at TIMESTAMPTZ,
  last_activity_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS account_pnl_rank (
  rank BIGINT,
  total_accounts BIGINT,
  percentile NUMERIC(10, 4),
  account_id TEXT,
  account_label TEXT,
  account_image TEXT,
  total_pnl NUMERIC,
  pnl_pct NUMERIC(20, 4),
  win_rate NUMERIC(10, 2),
  total_position_count BIGINT,
  total_volume NUMERIC
);

CREATE TABLE IF NOT EXISTS pnl_leaderboard_stats (
  total_traders BIGINT,
  total_pnl_sum NUMERIC,
  avg_pnl NUMERIC,
  median_pnl NUMERIC,
  total_volume NUMERIC,
  avg_volume NUMERIC,
  profitable_traders BIGINT,
  unprofitable_traders BIGINT,
  profitable_pct NUMERIC(10, 2)
);

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
BEGIN
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
  -- Only get positions for active accounts (OPTIMIZED - avoids scanning all 3M+ positions)
  current_positions AS (
    SELECT
      p.account_id,
      p.term_id,
      p.curve_id,
      p.shares,
      p.total_deposit_assets_after_total_fees AS total_deposits,
      p.total_redeem_assets_for_receiver AS total_redemptions,
      p.created_at AS position_created_at,
      p.updated_at AS position_updated_at,
      v.current_share_price,
      (p.shares * v.current_share_price / 1e18)::NUMERIC AS equity_value,
      ((p.shares * v.current_share_price / 1e18) + p.total_redeem_assets_for_receiver - p.total_deposit_assets_after_total_fees)::NUMERIC AS position_pnl
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
      COUNT(*) FILTER (WHERE cp.position_pnl > 0) AS winning_positions,
      COUNT(*) FILTER (WHERE cp.position_pnl < 0) AS losing_positions,
      SUM(cp.total_deposits)::NUMERIC AS total_deposits,
      SUM(cp.total_redemptions)::NUMERIC AS total_redemptions,
      SUM(cp.equity_value)::NUMERIC AS current_equity_value,
      SUM(cp.position_pnl)::NUMERIC AS total_pnl,
      SUM(cp.position_pnl) FILTER (WHERE cp.shares = 0)::NUMERIC AS realized_pnl,
      SUM(cp.position_pnl) FILTER (WHERE cp.shares > 0)::NUMERIC AS unrealized_pnl,
      MAX(cp.position_pnl) AS best_trade_pnl,
      MIN(cp.position_pnl) AS worst_trade_pnl,
      MIN(cp.position_created_at) AS first_position_at,
      MAX(cp.position_updated_at) AS last_activity_at
    FROM current_positions cp
    GROUP BY cp.account_id
    HAVING COUNT(DISTINCT (cp.term_id, cp.curve_id)) >= p_min_positions
      AND (SUM(cp.total_deposits) + SUM(cp.total_redemptions)) >= p_min_volume
  ),
  enriched AS (
    SELECT
      am.account_id,
      a.label AS account_label,
      a.image AS account_image,
      am.total_pnl,
      COALESCE(am.realized_pnl, 0) AS realized_pnl,
      COALESCE(am.unrealized_pnl, 0) AS unrealized_pnl,
      CASE WHEN (am.total_deposits - am.total_redemptions) > 0
        THEN ((am.total_pnl * 100.0) / (am.total_deposits - am.total_redemptions))::NUMERIC(20, 4)
        ELSE 0::NUMERIC(20, 4)
      END AS pnl_pct,
      am.total_pnl AS pnl_change,
      am.total_position_count,
      am.active_position_count,
      am.winning_positions,
      am.losing_positions,
      CASE WHEN am.total_position_count > 0
        THEN ((am.winning_positions * 100.0) / am.total_position_count)::NUMERIC(10, 2)
        ELSE 0::NUMERIC(10, 2)
      END AS win_rate,
      am.total_deposits,
      am.total_redemptions,
      (am.total_deposits + am.total_redemptions)::NUMERIC AS total_volume,
      am.current_equity_value,
      am.best_trade_pnl,
      am.worst_trade_pnl,
      am.first_position_at,
      am.last_activity_at
    FROM account_metrics am
    JOIN account a ON am.account_id = a.id
    WHERE NOT p_exclude_protocol_accounts OR a.type NOT IN ('ProtocolVault', 'AtomWallet')
  ),
  ranked AS (
    SELECT e.*,
      CASE p_sort_by
        WHEN 'total_pnl' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_pnl ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_pnl DESC NULLS LAST) END
        WHEN 'pnl_pct' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.pnl_pct ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.pnl_pct DESC NULLS LAST) END
        WHEN 'win_rate' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.win_rate ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.win_rate DESC NULLS LAST) END
        WHEN 'total_volume' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_volume ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_volume DESC NULLS LAST) END
        ELSE ROW_NUMBER() OVER (ORDER BY e.total_pnl DESC NULLS LAST)
      END AS rank
    FROM enriched e
  )
  SELECT r.rank, r.account_id, r.account_label, r.account_image, r.total_pnl, r.realized_pnl, r.unrealized_pnl, r.pnl_pct, r.pnl_change, r.total_position_count, r.active_position_count, r.winning_positions, r.losing_positions, r.win_rate, r.total_deposits, r.total_redemptions, r.total_volume, r.current_equity_value, r.best_trade_pnl, r.worst_trade_pnl, r.first_position_at, r.last_activity_at
  FROM ranked r
  ORDER BY r.rank
  LIMIT p_limit OFFSET p_offset;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION get_pnl_leaderboard IS 'Returns a ranked leaderboard of accounts by PnL metrics with configurable time filters, sorting options, and filters for minimum positions/volume.';

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
DECLARE
  v_total_accounts BIGINT;
BEGIN
  -- Get total count of accounts in leaderboard
  SELECT COUNT(*) INTO v_total_accounts
  FROM get_pnl_leaderboard(
    p_limit := 1000000,
    p_offset := 0,
    p_time_filter := p_time_filter,
    p_sort_by := p_sort_by,
    p_term_id := p_term_id
  );

  -- Find the specific account's rank and data
  RETURN QUERY
  SELECT
    l.rank,
    v_total_accounts,
    ((v_total_accounts - l.rank + 1) * 100.0 / GREATEST(v_total_accounts, 1))::NUMERIC(10, 4),
    l.account_id,
    l.account_label,
    l.account_image,
    l.total_pnl,
    l.pnl_pct,
    l.win_rate,
    l.total_position_count,
    l.total_volume
  FROM get_pnl_leaderboard(
    p_limit := 1000000,
    p_offset := 0,
    p_time_filter := p_time_filter,
    p_sort_by := p_sort_by,
    p_term_id := p_term_id
  ) l
  WHERE l.account_id = p_account_id;
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
    COUNT(*)::BIGINT,
    SUM(l.total_pnl),
    AVG(l.total_pnl),
    PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY l.total_pnl)::NUMERIC,
    SUM(l.total_volume),
    AVG(l.total_volume),
    COUNT(*) FILTER (WHERE l.total_pnl > 0)::BIGINT,
    COUNT(*) FILTER (WHERE l.total_pnl <= 0)::BIGINT,
    (COUNT(*) FILTER (WHERE l.total_pnl > 0) * 100.0 / GREATEST(COUNT(*), 1))::NUMERIC(10, 2)
  FROM get_pnl_leaderboard(
    p_limit := 1000000,
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
BEGIN
  -- This is a specialized wrapper that filters to a specific vault
  RETURN QUERY
  WITH vault_positions AS (
    SELECT
      p.account_id,
      p.term_id,
      p.curve_id,
      p.shares,
      p.total_deposit_assets_after_total_fees AS total_deposits,
      p.total_redeem_assets_for_receiver AS total_redemptions,
      p.created_at AS position_created_at,
      p.updated_at AS position_updated_at,
      v.current_share_price,
      (p.shares * v.current_share_price / 1e18)::NUMERIC AS equity_value,
      ((p.shares * v.current_share_price / 1e18)
        + p.total_redeem_assets_for_receiver
        - p.total_deposit_assets_after_total_fees)::NUMERIC AS position_pnl
    FROM position p
    JOIN vault v ON p.term_id = v.term_id AND p.curve_id = v.curve_id
    WHERE p.term_id = p_term_id
      AND (p_curve_id IS NULL OR p.curve_id = p_curve_id)
  ),
  account_metrics AS (
    SELECT
      vp.account_id,
      COUNT(*) AS total_position_count,
      COUNT(*) FILTER (WHERE vp.shares > 0) AS active_position_count,
      COUNT(*) FILTER (WHERE vp.position_pnl > 0) AS winning_positions,
      COUNT(*) FILTER (WHERE vp.position_pnl < 0) AS losing_positions,
      SUM(vp.total_deposits)::NUMERIC AS total_deposits,
      SUM(vp.total_redemptions)::NUMERIC AS total_redemptions,
      SUM(vp.equity_value)::NUMERIC AS current_equity_value,
      SUM(vp.position_pnl)::NUMERIC AS total_pnl,
      SUM(vp.position_pnl) FILTER (WHERE vp.shares = 0)::NUMERIC AS realized_pnl,
      SUM(vp.position_pnl) FILTER (WHERE vp.shares > 0)::NUMERIC AS unrealized_pnl,
      MAX(vp.position_pnl) AS best_trade_pnl,
      MIN(vp.position_pnl) AS worst_trade_pnl,
      MIN(vp.position_created_at) AS first_position_at,
      MAX(vp.position_updated_at) AS last_activity_at
    FROM vault_positions vp
    GROUP BY vp.account_id
  ),
  enriched AS (
    SELECT
      am.account_id,
      a.label AS account_label,
      a.image AS account_image,
      am.total_pnl,
      COALESCE(am.realized_pnl, 0) AS realized_pnl,
      COALESCE(am.unrealized_pnl, 0) AS unrealized_pnl,
      CASE
        WHEN (am.total_deposits - am.total_redemptions) > 0
        THEN ((am.total_pnl * 100.0) / (am.total_deposits - am.total_redemptions))::NUMERIC(20, 4)
        ELSE 0::NUMERIC(20, 4)
      END AS pnl_pct,
      am.total_pnl AS pnl_change, -- No previous period for vault-specific
      am.total_position_count,
      am.active_position_count,
      am.winning_positions,
      am.losing_positions,
      CASE
        WHEN am.total_position_count > 0
        THEN ((am.winning_positions * 100.0) / am.total_position_count)::NUMERIC(10, 2)
        ELSE 0::NUMERIC(10, 2)
      END AS win_rate,
      am.total_deposits,
      am.total_redemptions,
      (am.total_deposits + am.total_redemptions)::NUMERIC AS total_volume,
      am.current_equity_value,
      am.best_trade_pnl,
      am.worst_trade_pnl,
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
        WHEN 'total_pnl' THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_pnl ASC NULLS LAST)
            ELSE ROW_NUMBER() OVER (ORDER BY e.total_pnl DESC NULLS LAST)
          END
        WHEN 'pnl_pct' THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.pnl_pct ASC NULLS LAST)
            ELSE ROW_NUMBER() OVER (ORDER BY e.pnl_pct DESC NULLS LAST)
          END
        WHEN 'win_rate' THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.win_rate ASC NULLS LAST)
            ELSE ROW_NUMBER() OVER (ORDER BY e.win_rate DESC NULLS LAST)
          END
        WHEN 'total_volume' THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_volume ASC NULLS LAST)
            ELSE ROW_NUMBER() OVER (ORDER BY e.total_volume DESC NULLS LAST)
          END
        ELSE
          ROW_NUMBER() OVER (ORDER BY e.total_pnl DESC NULLS LAST)
      END AS rank
    FROM enriched e
  )
  SELECT
    r.rank,
    r.account_id,
    r.account_label,
    r.account_image,
    r.total_pnl,
    r.realized_pnl,
    r.unrealized_pnl,
    r.pnl_pct,
    r.pnl_change,
    r.total_position_count,
    r.active_position_count,
    r.winning_positions,
    r.losing_positions,
    r.win_rate,
    r.total_deposits,
    r.total_redemptions,
    r.total_volume,
    r.current_equity_value,
    r.best_trade_pnl,
    r.worst_trade_pnl,
    r.first_position_at,
    r.last_activity_at
  FROM ranked r
  ORDER BY r.rank
  LIMIT p_limit
  OFFSET p_offset;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION get_vault_leaderboard IS 'Returns a leaderboard for a specific vault (term_id) with optional curve_id filter. Optimized for vault-specific queries.';

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
