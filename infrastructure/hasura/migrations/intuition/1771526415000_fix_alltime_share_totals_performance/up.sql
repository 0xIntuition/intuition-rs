-- Fix: replace share_totals CTE with LATERAL join for all-time functions
--
-- The share_totals CTE in 1771526414000 did a full unfiltered GROUP BY over
-- the entire position_change_daily continuous aggregate. When p_term_id is NULL
-- (default for get_pnl_leaderboard), this scans every row before joining.
--
-- Fix: use LEFT JOIN LATERAL so the planner can seek per-position via
-- idx_pcd_account_term_bucket instead of materializing a massive CTE.

-- ========================================
-- 1. get_pnl_leaderboard (all-time)
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
  v_bucket_start TIMESTAMPTZ;
  v_bucket_end TIMESTAMPTZ;
  v_limit INTEGER;
  v_offset INTEGER;
BEGIN
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

  v_bucket_start := date_trunc('day', v_start_time);
  v_bucket_end := date_trunc('day', v_end_time);

  RETURN QUERY
  WITH
  active_accounts AS (
    SELECT DISTINCT pc.account_id
    FROM position_change_daily pc
    WHERE pc.bucket >= v_bucket_start
      AND pc.bucket <= v_bucket_end
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
  ),
  period_pnl_change AS (
    SELECT
      pc.account_id,
      SUM(COALESCE(pc.assets_out_period, 0) - COALESCE(pc.assets_in_period, 0))::NUMERIC AS period_realized_change_raw
    FROM position_change_daily pc
    WHERE pc.bucket >= v_bucket_start
      AND pc.bucket <= v_bucket_end
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
      (TRUNC(p.shares * v.current_share_price / 1000000000000000000::NUMERIC) + p.total_redeem_assets_for_receiver - p.total_deposit_assets_after_total_fees)::NUMERIC AS position_pnl_raw,
      COALESCE(st.total_shares_acquired, 0) AS total_shares_acquired,
      COALESCE(st.total_shares_redeemed, 0) AS total_shares_redeemed,
      -- Per-share cost basis realized PnL
      CASE
        WHEN p.shares <= 0 THEN
          (TRUNC(p.shares * v.current_share_price / 1000000000000000000::NUMERIC)
            + p.total_redeem_assets_for_receiver
            - p.total_deposit_assets_after_total_fees)::NUMERIC
        WHEN p.total_redeem_assets_for_receiver <= 0 THEN 0::NUMERIC
        ELSE (p.total_redeem_assets_for_receiver - TRUNC(
          p.total_deposit_assets_after_total_fees * COALESCE(st.total_shares_redeemed, 0)
          / NULLIF(COALESCE(st.total_shares_acquired, 0), 0)
        ))::NUMERIC
      END AS realized_pnl
    FROM position p
    JOIN vault v ON p.term_id = v.term_id AND p.curve_id = v.curve_id
    LEFT JOIN LATERAL (
      SELECT
        SUM(pc.shares_in_period)::NUMERIC AS total_shares_acquired,
        SUM(pc.shares_out_period)::NUMERIC AS total_shares_redeemed
      FROM position_change_daily pc
      WHERE pc.account_id = p.account_id
        AND pc.term_id = p.term_id
        AND pc.curve_id = p.curve_id
    ) st ON TRUE
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
      SUM(cp.realized_pnl)::NUMERIC AS realized_pnl_raw,
      SUM(cp.position_pnl_raw - cp.realized_pnl)::NUMERIC AS unrealized_pnl_raw,
      MAX(cp.position_pnl_raw) AS best_trade_pnl_raw,
      MIN(cp.position_pnl_raw) AS worst_trade_pnl_raw,
      MIN(cp.position_created_at) AS first_position_at,
      MAX(cp.position_updated_at) AS last_activity_at,
      SUM(CASE
        WHEN cp.shares <= 0 THEN cp.total_deposits_raw
        WHEN cp.total_redemptions_raw <= 0 THEN 0
        ELSE TRUNC(cp.total_deposits_raw * cp.total_shares_redeemed
             / NULLIF(cp.total_shares_acquired, 0))
      END)::NUMERIC AS denominator_closed,
      SUM(CASE
        WHEN cp.shares <= 0 THEN 0
        WHEN cp.total_redemptions_raw <= 0 THEN cp.total_deposits_raw
        ELSE TRUNC(cp.total_deposits_raw * cp.shares
             / NULLIF(cp.total_shares_acquired, 0))
      END)::NUMERIC AS denominator_open
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
      COALESCE(
        (am.total_pnl_raw * 100.0 / NULLIF(am.total_deposits_raw, 0))::NUMERIC(20, 4),
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
      CASE
        WHEN p_sort_by IN ('total_pnl', 'pnl') THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_pnl_raw ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_pnl_raw DESC NULLS LAST) END
        WHEN p_sort_by IN ('pnl_pct', 'roi') THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.pnl_pct ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.pnl_pct DESC NULLS LAST) END
        WHEN p_sort_by = 'win_rate' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.win_rate ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.win_rate DESC NULLS LAST) END
        WHEN p_sort_by IN ('total_volume', 'volume') THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_volume_raw ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_volume_raw DESC NULLS LAST) END
        WHEN p_sort_by IN ('position_count', 'positions') THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_position_count ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_position_count DESC NULLS LAST) END
        WHEN p_sort_by = 'newest' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.first_position_at ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.first_position_at DESC NULLS LAST) END
        WHEN p_sort_by = 'most_improved' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.pnl_change_raw ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.pnl_change_raw DESC NULLS LAST) END
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
-- 2. get_vault_leaderboard (all-time)
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
      COALESCE(st.total_shares_acquired, 0) AS total_shares_acquired,
      COALESCE(st.total_shares_redeemed, 0) AS total_shares_redeemed,
      -- Per-share cost basis realized PnL
      CASE
        WHEN p.shares <= 0 THEN
          (TRUNC(p.shares * v.current_share_price / 1000000000000000000::NUMERIC)
            + p.total_redeem_assets_for_receiver
            - p.total_deposit_assets_after_total_fees)::NUMERIC
        WHEN p.total_redeem_assets_for_receiver <= 0 THEN 0::NUMERIC
        ELSE (p.total_redeem_assets_for_receiver - TRUNC(
          p.total_deposit_assets_after_total_fees * COALESCE(st.total_shares_redeemed, 0)
          / NULLIF(COALESCE(st.total_shares_acquired, 0), 0)
        ))::NUMERIC
      END AS realized_pnl,
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
    LEFT JOIN LATERAL (
      SELECT
        SUM(pc.shares_in_period)::NUMERIC AS total_shares_acquired,
        SUM(pc.shares_out_period)::NUMERIC AS total_shares_redeemed
      FROM position_change_daily pc
      WHERE pc.account_id = p.account_id
        AND pc.term_id = p.term_id
        AND pc.curve_id = p.curve_id
    ) st ON TRUE
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
      SUM(vp.realized_pnl)::NUMERIC AS realized_pnl_raw,
      SUM(vp.position_pnl_raw - vp.realized_pnl)::NUMERIC AS unrealized_pnl_raw,
      MAX(vp.position_pnl_raw) AS best_trade_pnl_raw,
      MIN(vp.position_pnl_raw) AS worst_trade_pnl_raw,
      SUM(vp.redeemable_assets_raw) FILTER (WHERE vp.shares > 0)::NUMERIC AS redeemable_assets_raw,
      MIN(vp.position_created_at) AS first_position_at,
      MAX(vp.position_updated_at) AS last_activity_at,
      SUM(CASE
        WHEN vp.shares <= 0 THEN vp.total_deposits_raw
        WHEN vp.total_redemptions_raw <= 0 THEN 0
        ELSE TRUNC(vp.total_deposits_raw * vp.total_shares_redeemed
             / NULLIF(vp.total_shares_acquired, 0))
      END)::NUMERIC AS denominator_closed,
      SUM(CASE
        WHEN vp.shares <= 0 THEN 0
        WHEN vp.total_redemptions_raw <= 0 THEN vp.total_deposits_raw
        ELSE TRUNC(vp.total_deposits_raw * vp.shares
             / NULLIF(vp.total_shares_acquired, 0))
      END)::NUMERIC AS denominator_open
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
      COALESCE(
        (COALESCE(am.realized_pnl_raw, 0) * 100.0 / NULLIF(am.denominator_closed, 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS realized_pnl_pct,
      COALESCE(
        (COALESCE(am.unrealized_pnl_raw, 0) * 100.0 / NULLIF(am.denominator_open, 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS unrealized_pnl_pct,
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
$$ LANGUAGE plpgsql STABLE;
