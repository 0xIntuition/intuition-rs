-- Add curve_config and fee_config tables to replace hardcoded bonding curve and fee constants.
-- Previously hardcoded values were ALL WRONG:
--   OFFSET:       3e19 (actual: 5e17)
--   HALF_SLOPE:   5e16 (actual: 1e18)
--   protocolFee:  125  (actual: 100)
--   exitFee:      75   (actual: 100)
--   feeThreshold: 1e18 (actual: 1e17)

-- ========================================
-- 1. CREATE CONFIG TABLES
-- ========================================

CREATE TABLE IF NOT EXISTS curve_config (
  curve_id NUMERIC(78, 0) PRIMARY KEY,
  name TEXT NOT NULL,
  curve_type TEXT NOT NULL CHECK (curve_type IN ('linear', 'offset_progressive')),
  slope NUMERIC(78, 0),
  half_slope NUMERIC(78, 0),
  "offset" NUMERIC(78, 0),
  contract_address TEXT,
  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS fee_config (
  id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
  protocol_fee NUMERIC NOT NULL,
  exit_fee NUMERIC NOT NULL,
  entry_fee NUMERIC NOT NULL,
  fee_denominator NUMERIC NOT NULL,
  fee_threshold NUMERIC(78, 0) NOT NULL,
  default_curve_id NUMERIC(78, 0) NOT NULL DEFAULT 1,
  created_at TIMESTAMPTZ DEFAULT NOW(),
  updated_at TIMESTAMPTZ DEFAULT NOW()
);

INSERT INTO curve_config (curve_id, name, curve_type, slope, half_slope, "offset", contract_address)
VALUES
  (1, 'Linear', 'linear', NULL, NULL, NULL, NULL),
  (2, 'Offset Progressive', 'offset_progressive',
    2000000000000000000,   -- SLOPE  = 2.0 (UD60x18)
    1000000000000000000,   -- HALF_SLOPE = 1.0 (UD60x18)
    500000000000000000,    -- OFFSET = 0.5 (UD60x18)
    NULL)                  -- contract_address is set by the indexer per environment
ON CONFLICT (curve_id) DO NOTHING;

INSERT INTO fee_config (id, protocol_fee, exit_fee, entry_fee, fee_denominator, fee_threshold, default_curve_id)
VALUES (1, 100, 100, 100, 10000, 100000000000000000, 1)
ON CONFLICT (id) DO NOTHING;

-- ========================================
-- 2. RECREATE position_with_value VIEW
-- ========================================

DROP VIEW IF EXISTS public.position_with_value;

CREATE VIEW public.position_with_value AS
SELECT
  base.*,
  -- PnL uses mark-to-market theoretical value (share price) not redeemable_assets
  (base.theoretical_value + base.total_redeem_assets_for_receiver
    - base.total_deposit_assets_after_total_fees)::NUMERIC AS pnl,
  -- pnl_pct = pnl / total_deposits * 100 (standard ROI)
  CASE
    WHEN base.total_deposit_assets_after_total_fees > 0
    THEN ((base.theoretical_value + base.total_redeem_assets_for_receiver
           - base.total_deposit_assets_after_total_fees) * 100.0
          / base.total_deposit_assets_after_total_fees)::NUMERIC(20, 4)
    ELSE 0::NUMERIC(20, 4)
  END AS pnl_pct
FROM (
  SELECT
    p.id, p.account_id, p.term_id, p.curve_id, p.shares,
    p.total_deposit_assets_after_total_fees,
    p.total_redeem_assets_for_receiver,
    p.block_number, p.log_index, p.transaction_hash,
    p.transaction_index, p.created_at, p.updated_at,
    -- Theoretical value: shares * share_price / 1e18 (kept for reference, in wei)
    TRUNC(p.shares * v.current_share_price / 1000000000000000000::NUMERIC)::NUMERIC AS theoretical_value,
    -- Redeemable assets with conditional exit fee (in wei)
    CASE
      WHEN p.shares = 0 THEN 0::NUMERIC
      -- Linear curve: rawAssets = shares * totalAssets / totalShares
      WHEN cc.curve_type = 'linear' THEN
        (SELECT GREATEST(
          raw_a
          - TRUNC((raw_a * fc.protocol_fee + fc.fee_denominator - 1) / fc.fee_denominator)
          - CASE
              WHEN (v_default.total_shares - p.shares) >= fc.fee_threshold
              THEN TRUNC((raw_a * fc.exit_fee + fc.fee_denominator - 1) / fc.fee_denominator)
              ELSE 0
            END
        , 0)::NUMERIC
        FROM (SELECT TRUNC(p.shares * v.total_assets
                      / NULLIF(v.total_shares, 0))::NUMERIC AS raw_a) calc)
      -- Offset Progressive curve: Quadratic bonding curve
      WHEN cc.curve_type = 'offset_progressive' THEN
        (SELECT GREATEST(
          raw_a
          - TRUNC((raw_a * fc.protocol_fee + fc.fee_denominator - 1) / fc.fee_denominator)
          - CASE
              WHEN COALESCE(v_default.total_shares, 0) >= fc.fee_threshold
              THEN TRUNC((raw_a * fc.exit_fee + fc.fee_denominator - 1) / fc.fee_denominator)
              ELSE 0
            END
        , 0)::NUMERIC
        FROM (SELECT TRUNC(
          (
            TRUNC((v.total_shares + cc."offset")
             * (v.total_shares + cc."offset")
             / 1000000000000000000::NUMERIC)
            -
            TRUNC(((v.total_shares + cc."offset" - p.shares)
              * (v.total_shares + cc."offset" - p.shares)
              + 999999999999999999::NUMERIC)
             / 1000000000000000000::NUMERIC)
          ) * cc.half_slope / 1000000000000000000::NUMERIC
        )::NUMERIC AS raw_a) calc)
      ELSE 0::NUMERIC
    END AS redeemable_assets
  FROM position p
  JOIN vault v ON v.term_id = p.term_id AND v.curve_id = p.curve_id
  LEFT JOIN curve_config cc ON p.curve_id = cc.curve_id
  CROSS JOIN fee_config fc
  LEFT JOIN vault v_default ON v_default.term_id = p.term_id AND v_default.curve_id = fc.default_curve_id
) base;

-- ========================================
-- 3. VAULT-SPECIFIC ALL-TIME LEADERBOARD
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
      -- Calculate raw assets from curve (before fees)
      CASE
        WHEN p.shares = 0 THEN 0::NUMERIC
        -- Linear curve: rawAssets = shares * totalAssets / totalShares
        WHEN cc.curve_type = 'linear' THEN
          CASE WHEN v.total_shares > 0
            THEN TRUNC(p.shares * v.total_assets / v.total_shares)
            ELSE 0::NUMERIC
          END
        -- Offset Progressive curve: Quadratic bonding curve
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
  -- Apply fees to get redeemable assets
  vault_positions_with_fees AS (
    SELECT
      vp.*,
      -- Protocol fee (rounds up): ceil(rawAssets * fee / denominator)
      TRUNC((vp.raw_assets_from_curve * f.protocol_fee + f.fee_denominator - 1) / f.fee_denominator)::NUMERIC AS protocol_fee,
      -- Exit fee (rounds up): conditional on default vault having >= threshold shares
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
-- 4. VAULT-SPECIFIC PERIOD LEADERBOARD
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
  fc AS (SELECT * FROM fee_config LIMIT 1),

  -- Get accounts with activity in the period for this vault
  active_accounts AS (
    SELECT DISTINCT pc.account_id
    FROM position_change_daily pc
    WHERE pc.bucket >= p_start_date
      AND pc.bucket <= p_end_date
      AND pc.term_id = p_term_id
      AND (p_curve_id IS NULL OR pc.curve_id = p_curve_id)
  ),

  -- Preferred prices at period START (latest price at or before start)
  prices_before_start AS (
    SELECT DISTINCT ON (term_id, curve_id)
      term_id, curve_id, share_price, total_assets, total_shares
    FROM share_price_change
    WHERE term_id = p_term_id
      AND (p_curve_id IS NULL OR curve_id = p_curve_id)
      AND block_timestamp <= v_start_timestamp
    ORDER BY term_id, curve_id, block_timestamp DESC, log_index DESC
  ),
  -- Fallback prices for vaults with no price before start (earliest known price)
  prices_earliest AS (
    SELECT DISTINCT ON (term_id, curve_id)
      term_id, curve_id, share_price, total_assets, total_shares
    FROM share_price_change
    WHERE term_id = p_term_id
      AND (p_curve_id IS NULL OR curve_id = p_curve_id)
    ORDER BY term_id, curve_id, block_timestamp ASC, log_index ASC
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
      ON pss.account_id = pse.account_id
      AND pss.term_id = pse.term_id
      AND pss.curve_id = pse.curve_id
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

  -- Calculate period PnL and redeemable assets at period end
  position_pnl AS (
    SELECT
      ppm.*,
      (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits)::NUMERIC AS period_total_pnl,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits) > 0 THEN 1 ELSE 0 END AS is_winning,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits) < 0 THEN 1 ELSE 0 END AS is_losing,
      -- Calculate raw assets from bonding curve at period end
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

  -- Apply fees to get redeemable assets
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

  -- Aggregate by account
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
