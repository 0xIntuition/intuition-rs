-- Fix pnl_leaderboard_entry type mismatch:
-- The leaderboard functions were updated (in migration 1768475500000) to return
-- _raw/_formatted column pairs and redeemable_assets, but the pnl_leaderboard_entry
-- composite type was never updated to match. This causes a runtime error:
--   "Returned type numeric(30,4) does not match expected type numeric(20,4) in column 8"
--
-- Fix: DROP the old 22-column type, CREATE the new 34-column type, then re-create
-- all 4 functions that depend on it.
-- NOTE: DROP CASCADE is required because the functions reference the type.

-- ========================================
-- 0. RECREATE pnl_leaderboard_entry TYPE
-- ========================================

DROP TABLE IF EXISTS pnl_leaderboard_entry CASCADE;

CREATE TABLE pnl_leaderboard_entry (
  rank BIGINT,
  account_id TEXT,
  account_label TEXT,
  account_image TEXT,
  -- PnL values: raw (wei) and formatted (ETH, 4 decimals)
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
  -- Volume values: raw (wei) and formatted (ETH, 4 decimals)
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
  -- Redeemable assets
  redeemable_assets_raw NUMERIC,
  redeemable_assets_formatted NUMERIC(30, 4),
  -- Timestamps
  first_position_at TIMESTAMPTZ,
  last_activity_at TIMESTAMPTZ
);

-- ========================================
-- 1. GLOBAL ALL-TIME LEADERBOARD
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
  -- Input validation for limits
  v_limit := LEAST(GREATEST(COALESCE(p_limit, 100), 1), 10000);
  v_offset := GREATEST(COALESCE(p_offset, 0), 0);

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
  active_accounts AS (
    SELECT DISTINCT pc.account_id
    FROM position_change_daily pc
    WHERE pc.bucket >= v_start_time
      AND pc.bucket <= v_end_time
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
  ),
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
      TRUNC(p.shares * v.current_share_price / 1000000000000000000::NUMERIC) AS equity_value_raw,
      (TRUNC(p.shares * v.current_share_price / 1000000000000000000::NUMERIC) + p.total_redeem_assets_for_receiver - p.total_deposit_assets_after_total_fees)::NUMERIC AS position_pnl_raw
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
      SUM(cp.position_pnl_raw) FILTER (WHERE cp.shares <= 0)::NUMERIC AS realized_pnl_raw,
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
      -- PnL percentage: pnl / total_deposits * 100 (standard ROI)
      COALESCE(
        (am.total_pnl_raw * 100.0 / NULLIF(am.total_deposits_raw, 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS pnl_pct,
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
    NULL::NUMERIC AS redeemable_assets_raw,
    NULL::NUMERIC(30, 4) AS redeemable_assets_formatted,
    r.first_position_at,
    r.last_activity_at
  FROM ranked r
  ORDER BY r.rank
  LIMIT v_limit OFFSET v_offset;
END;
$$ LANGUAGE plpgsql STABLE;

-- ========================================
-- 2. VAULT-SPECIFIC ALL-TIME LEADERBOARD
--    Uses fee_config and curve_config tables
-- ========================================

CREATE OR REPLACE FUNCTION get_vault_leaderboard(
  p_term_id TEXT,
  p_curve_id NUMERIC DEFAULT NULL,
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
  IF p_term_id IS NULL OR p_term_id = '' THEN
    RETURN;
  END IF;

  v_limit := LEAST(GREATEST(COALESCE(p_limit, 100), 1), 10000);
  v_offset := GREATEST(COALESCE(p_offset, 0), 0);

  RETURN QUERY
  WITH
  fc AS (SELECT * FROM fee_config LIMIT 1),
  vault_positions AS (
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
      TRUNC(p.shares * v.current_share_price / 1000000000000000000::NUMERIC) AS equity_value_raw,
      (TRUNC(p.shares * v.current_share_price / 1000000000000000000::NUMERIC)
        + p.total_redeem_assets_for_receiver
        - p.total_deposit_assets_after_total_fees)::NUMERIC AS position_pnl_raw,
      CASE
        WHEN p.shares = 0 THEN 0::NUMERIC
        WHEN cc.curve_type = 'linear' THEN
          CASE WHEN v.total_shares > 0
            THEN TRUNC(p.shares * v.total_assets / v.total_shares)
            ELSE 0::NUMERIC
          END
        WHEN cc.curve_type = 'offset_progressive' THEN
          (
            SELECT TRUNC(
              (
                TRUNC((v.total_shares + cc."offset") * (v.total_shares + cc."offset") / 1000000000000000000::NUMERIC)
                -
                TRUNC(((v.total_shares + cc."offset" - p.shares) * (v.total_shares + cc."offset" - p.shares) + 999999999999999999::NUMERIC) / 1000000000000000000::NUMERIC)
              )
              * cc.half_slope / 1000000000000000000::NUMERIC
            )
          )
        ELSE 0::NUMERIC
      END AS raw_assets_from_curve,
      v_default.total_shares AS default_vault_total_shares
    FROM position p
    JOIN vault v ON p.term_id = v.term_id AND p.curve_id = v.curve_id
    LEFT JOIN curve_config cc ON p.curve_id = cc.curve_id
    LEFT JOIN vault v_default ON p.term_id = v_default.term_id AND v_default.curve_id = (SELECT default_curve_id FROM fc)
    WHERE p.term_id = p_term_id
      AND (p_curve_id IS NULL OR p.curve_id = p_curve_id)
  ),
  vault_positions_with_fees AS (
    SELECT
      vp.*,
      TRUNC((vp.raw_assets_from_curve * f.protocol_fee + f.fee_denominator - 1) / f.fee_denominator)::NUMERIC AS protocol_fee,
      CASE
        WHEN vp.curve_id = f.default_curve_id AND (vp.default_vault_total_shares - vp.shares) >= f.fee_threshold
          THEN TRUNC((vp.raw_assets_from_curve * f.exit_fee + f.fee_denominator - 1) / f.fee_denominator)
        WHEN vp.curve_id <> f.default_curve_id AND COALESCE(vp.default_vault_total_shares, 0) >= f.fee_threshold
          THEN TRUNC((vp.raw_assets_from_curve * f.exit_fee + f.fee_denominator - 1) / f.fee_denominator)
        ELSE 0
      END::NUMERIC AS exit_fee
    FROM vault_positions vp
    CROSS JOIN fc f
  ),
  vault_positions_final AS (
    SELECT
      vpf.*,
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
      SUM(vp.position_pnl_raw) FILTER (WHERE vp.shares <= 0)::NUMERIC AS realized_pnl_raw,
      SUM(vp.position_pnl_raw) FILTER (WHERE vp.shares > 0)::NUMERIC AS unrealized_pnl_raw,
      MAX(vp.position_pnl_raw) AS best_trade_pnl_raw,
      MIN(vp.position_pnl_raw) AS worst_trade_pnl_raw,
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
        (am.total_pnl_raw * 100.0 / NULLIF(am.total_deposits_raw, 0))::NUMERIC(20, 4),
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

-- ========================================
-- 3. GLOBAL PERIOD LEADERBOARD
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

  RETURN QUERY
  WITH
  active_accounts AS (
    SELECT DISTINCT pc.account_id
    FROM position_change_daily pc
    WHERE pc.bucket >= p_start_date
      AND pc.bucket <= p_end_date
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
  ),

  prices_before_start AS (
    SELECT DISTINCT ON (term_id, curve_id)
      term_id, curve_id, share_price, total_assets, total_shares
    FROM share_price_change
    WHERE block_timestamp <= v_start_timestamp
    ORDER BY term_id, curve_id, block_timestamp DESC, log_index DESC
  ),
  prices_earliest AS (
    SELECT DISTINCT ON (term_id, curve_id)
      term_id, curve_id, share_price, total_assets, total_shares
    FROM share_price_change
    ORDER BY term_id, curve_id, block_timestamp ASC, log_index ASC
  ),

  prices_at_end AS (
    SELECT DISTINCT ON (term_id, curve_id)
      term_id, curve_id, share_price, total_assets, total_shares
    FROM share_price_change
    WHERE block_timestamp <= v_end_timestamp
    ORDER BY term_id, curve_id, block_timestamp DESC, log_index DESC
  ),

  position_state_start AS (
    SELECT
      pc.account_id, pc.term_id, pc.curve_id,
      SUM(pc.shares_delta_period)::NUMERIC AS shares_at_start,
      SUM(pc.assets_in_period)::NUMERIC AS cumulative_deposits_before,
      SUM(pc.assets_out_period)::NUMERIC AS cumulative_redemptions_before
    FROM position_change_daily pc
    WHERE pc.bucket < p_start_date
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
    GROUP BY pc.account_id, pc.term_id, pc.curve_id
  ),

  position_state_end AS (
    SELECT
      pc.account_id, pc.term_id, pc.curve_id,
      SUM(pc.shares_delta_period)::NUMERIC AS shares_at_end,
      SUM(pc.assets_in_period)::NUMERIC AS cumulative_deposits_through,
      SUM(pc.assets_out_period)::NUMERIC AS cumulative_redemptions_through
    FROM position_change_daily pc
    WHERE pc.bucket <= p_end_date
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
    GROUP BY pc.account_id, pc.term_id, pc.curve_id
  ),

  period_activity AS (
    SELECT
      pc.account_id, pc.term_id, pc.curve_id,
      SUM(pc.assets_in_period)::NUMERIC AS period_deposits,
      SUM(pc.assets_out_period)::NUMERIC AS period_redemptions,
      SUM(pc.shares_delta_period)::NUMERIC AS period_shares_delta
    FROM position_change_daily pc
    WHERE pc.bucket >= p_start_date
      AND pc.bucket <= p_end_date
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
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
      COALESCE(pbs.share_price, pe_earliest.share_price) AS price_at_start,
      COALESCE(pe.share_price, 0) AS price_at_end,
      TRUNC(COALESCE(pss.shares_at_start, 0) * COALESCE(pbs.share_price, pe_earliest.share_price) / 1000000000000000000::NUMERIC) AS equity_at_start,
      TRUNC(COALESCE(pse.shares_at_end, 0) * COALESCE(pe.share_price, 0) / 1000000000000000000::NUMERIC) AS equity_at_end,
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
    LEFT JOIN prices_at_end pe
      ON COALESCE(pss.term_id, pse.term_id, pa.term_id) = pe.term_id
      AND COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) = pe.curve_id
    WHERE COALESCE(pss.account_id, pse.account_id, pa.account_id) IN (SELECT account_id FROM active_accounts)
  ),

  position_pnl AS (
    SELECT
      ppm.*,
      (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits)::NUMERIC AS period_total_pnl,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits) > 0 THEN 1 ELSE 0 END AS is_winning,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits) < 0 THEN 1 ELSE 0 END AS is_losing
    FROM position_period_metrics ppm
  ),

  account_metrics AS (
    SELECT
      pp.account_id,
      COUNT(DISTINCT (pp.term_id, pp.curve_id)) FILTER (WHERE pp.had_activity OR pp.shares_at_end > 0) AS period_position_count,
      COUNT(DISTINCT (pp.term_id, pp.curve_id)) FILTER (WHERE pp.shares_at_end > 0) AS active_position_count,
      SUM(pp.is_winning) AS winning_positions,
      SUM(pp.is_losing) AS losing_positions,
      SUM(pp.period_deposits)::NUMERIC AS period_deposits_raw,
      SUM(pp.period_redemptions)::NUMERIC AS period_redemptions_raw,
      SUM(pp.period_total_pnl)::NUMERIC AS total_pnl_raw,
      SUM(pp.period_total_pnl) FILTER (WHERE pp.shares_at_end <= 0)::NUMERIC AS realized_pnl_raw,
      SUM(pp.period_total_pnl) FILTER (WHERE pp.shares_at_end > 0)::NUMERIC AS unrealized_pnl_raw,
      SUM(pp.equity_at_start)::NUMERIC AS equity_at_start,
      SUM(pp.equity_at_end)::NUMERIC AS equity_at_end,
      MAX(pp.period_total_pnl) AS best_trade_pnl_raw,
      MIN(pp.period_total_pnl) AS worst_trade_pnl_raw
    FROM position_pnl pp
    GROUP BY pp.account_id
    HAVING COUNT(DISTINCT (pp.term_id, pp.curve_id)) FILTER (WHERE pp.had_activity) >= GREATEST(p_min_positions, 1)
      AND (SUM(pp.period_deposits) + SUM(pp.period_redemptions)) >= COALESCE(p_min_volume * 1e18, 0)
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
          CASE WHEN am.equity_at_start > 0 THEN am.equity_at_start
               ELSE am.period_deposits_raw
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
      p_start_date AS first_position_at,
      p_end_date AS last_activity_at
    FROM account_metrics am
    JOIN account a ON am.account_id = a.id
    WHERE NOT p_exclude_protocol_accounts OR a.type NOT IN ('ProtocolVault', 'AtomWallet')
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
    NULL::NUMERIC AS redeemable_assets_raw,
    NULL::NUMERIC(30, 4) AS redeemable_assets_formatted,
    r.first_position_at,
    r.last_activity_at
  FROM ranked r
  ORDER BY r.rank
  LIMIT v_limit OFFSET v_offset;
END;
$$ LANGUAGE plpgsql STABLE;

-- ========================================
-- 4. VAULT-SPECIFIC PERIOD LEADERBOARD
--    Uses fee_config and curve_config tables
-- ========================================

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

  RETURN QUERY
  WITH
  fc AS (SELECT * FROM fee_config LIMIT 1),

  active_accounts AS (
    SELECT DISTINCT pc.account_id
    FROM position_change_daily pc
    WHERE pc.bucket >= p_start_date
      AND pc.bucket <= p_end_date
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

  position_state_start AS (
    SELECT
      pc.account_id, pc.term_id, pc.curve_id,
      SUM(pc.shares_delta_period)::NUMERIC AS shares_at_start,
      SUM(pc.assets_in_period)::NUMERIC AS cumulative_deposits_before,
      SUM(pc.assets_out_period)::NUMERIC AS cumulative_redemptions_before
    FROM position_change_daily pc
    WHERE pc.bucket < p_start_date
      AND pc.term_id = p_term_id
      AND (p_curve_id IS NULL OR pc.curve_id = p_curve_id)
    GROUP BY pc.account_id, pc.term_id, pc.curve_id
  ),

  position_state_end AS (
    SELECT
      pc.account_id, pc.term_id, pc.curve_id,
      SUM(pc.shares_delta_period)::NUMERIC AS shares_at_end,
      SUM(pc.assets_in_period)::NUMERIC AS cumulative_deposits_through,
      SUM(pc.assets_out_period)::NUMERIC AS cumulative_redemptions_through
    FROM position_change_daily pc
    WHERE pc.bucket <= p_end_date
      AND pc.term_id = p_term_id
      AND (p_curve_id IS NULL OR pc.curve_id = p_curve_id)
    GROUP BY pc.account_id, pc.term_id, pc.curve_id
  ),

  period_activity AS (
    SELECT
      pc.account_id, pc.term_id, pc.curve_id,
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

  position_period_metrics AS (
    SELECT
      COALESCE(pss.account_id, pse.account_id, pa.account_id) AS account_id,
      COALESCE(pss.term_id, pse.term_id, pa.term_id) AS term_id,
      COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) AS curve_id,
      COALESCE(pss.shares_at_start, 0) AS shares_at_start,
      COALESCE(pse.shares_at_end, 0) AS shares_at_end,
      COALESCE(pa.period_deposits, 0) AS period_deposits,
      COALESCE(pa.period_redemptions, 0) AS period_redemptions,
      COALESCE(pbs.share_price, pe_earliest.share_price) AS price_at_start,
      COALESCE(ve.share_price, 0) AS price_at_end,
      COALESCE(ve.total_assets, 0) AS vault_total_assets_end,
      COALESCE(ve.total_shares, 0) AS vault_total_shares_end,
      TRUNC(COALESCE(pss.shares_at_start, 0) * COALESCE(pbs.share_price, pe_earliest.share_price) / 1000000000000000000::NUMERIC) AS equity_at_start,
      TRUNC(COALESCE(pse.shares_at_end, 0) * COALESCE(ve.share_price, 0) / 1000000000000000000::NUMERIC) AS equity_at_end,
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
    WHERE COALESCE(pss.account_id, pse.account_id, pa.account_id) IN (SELECT account_id FROM active_accounts)
  ),

  position_pnl AS (
    SELECT
      ppm.*,
      (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits)::NUMERIC AS period_total_pnl,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits) > 0 THEN 1 ELSE 0 END AS is_winning,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits) < 0 THEN 1 ELSE 0 END AS is_losing,
      CASE
        WHEN ppm.shares_at_end = 0 THEN 0::NUMERIC
        WHEN cc.curve_type = 'linear' THEN
          CASE WHEN ppm.vault_total_shares_end > 0
            THEN TRUNC(ppm.shares_at_end * ppm.vault_total_assets_end / ppm.vault_total_shares_end)
            ELSE 0::NUMERIC
          END
        WHEN cc.curve_type = 'offset_progressive' THEN
          (
            SELECT TRUNC(
              (
                TRUNC((ppm.vault_total_shares_end + cc."offset") * (ppm.vault_total_shares_end + cc."offset") / 1000000000000000000::NUMERIC)
                -
                TRUNC(((ppm.vault_total_shares_end + cc."offset" - ppm.shares_at_end) * (ppm.vault_total_shares_end + cc."offset" - ppm.shares_at_end) + 999999999999999999::NUMERIC) / 1000000000000000000::NUMERIC)
              )
              * cc.half_slope / 1000000000000000000::NUMERIC
            )
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
      SUM(pf.period_total_pnl) FILTER (WHERE pf.shares_at_end <= 0)::NUMERIC AS realized_pnl_raw,
      SUM(pf.period_total_pnl) FILTER (WHERE pf.shares_at_end > 0)::NUMERIC AS unrealized_pnl_raw,
      SUM(pf.equity_at_start)::NUMERIC AS equity_at_start,
      SUM(pf.equity_at_end)::NUMERIC AS equity_at_end,
      MAX(pf.period_total_pnl) AS best_trade_pnl_raw,
      MIN(pf.period_total_pnl) AS worst_trade_pnl_raw,
      SUM(pf.redeemable_assets_raw) FILTER (WHERE pf.shares_at_end > 0)::NUMERIC AS redeemable_assets_raw
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
          CASE WHEN am.equity_at_start > 0 THEN am.equity_at_start
               ELSE am.period_deposits_raw
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
