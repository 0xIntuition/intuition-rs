-- Fix position_with_value view:
-- 1. Keep all monetary values in wei (no /1e18 normalization) for frontend bigint compatibility
-- 2. Use redeemable_assets (not theoretical_value) for pnl/pnl_pct calculations
-- 3. Add conditional exit fee logic matching contract behavior:
--    - Exit fee only charged when default vault (curve_id=1) has >= 1e18 shares remaining
--    - For curve_id=1: remaining = defaultVaultTotalShares - sharesToRedeem
--    - For curve_id=2+: remaining = defaultVaultTotalShares (unaffected by this redemption)
-- 4. Use NUMERIC integer arithmetic (TRUNC for floor-division) to match Solidity/BigInt rounding

CREATE OR REPLACE VIEW public.position_with_value AS
SELECT
  base.*,
  -- PnL uses redeemable_assets (what user actually gets) not theoretical_value
  (base.redeemable_assets + base.total_redeem_assets_for_receiver
    - base.total_deposit_assets_after_total_fees)::NUMERIC AS pnl,
  CASE
    WHEN (base.total_deposit_assets_after_total_fees - base.total_redeem_assets_for_receiver) > 0
    THEN ((base.redeemable_assets + base.total_redeem_assets_for_receiver
           - base.total_deposit_assets_after_total_fees) * 100.0
          / (base.total_deposit_assets_after_total_fees
             - base.total_redeem_assets_for_receiver))::NUMERIC(20, 4)
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
      -- Linear curve (curve_id = 1): rawAssets = shares * totalAssets / totalShares
      WHEN p.curve_id = 1 THEN
        (SELECT GREATEST(
          raw_a
          -- protocol fee 1.25% (rounds up): mulDivUp(raw, 125, 10000)
          - TRUNC((raw_a * 125 + 9999) / 10000)
          - CASE
              -- Exit fee only if default vault has >= 1e18 shares remaining after this redemption
              WHEN (v_default.total_shares - p.shares) >= 1000000000000000000::NUMERIC
              -- exit fee 0.75% (rounds up): mulDivUp(raw, 75, 10000)
              THEN TRUNC((raw_a * 75 + 9999) / 10000)
              ELSE 0
            END
        , 0)::NUMERIC
        FROM (SELECT TRUNC(p.shares * v.total_assets
                      / NULLIF(v.total_shares, 0))::NUMERIC AS raw_a) calc)
      -- Offset Progressive curve (curve_id = 2): Quadratic bonding curve
      -- offset = 30e18, halfSlope = 50e15
      -- rawAssets = ud60x18Mul(area, halfSlope) = TRUNC(area * halfSlope / 1e18)
      -- area = sSquared - sNextSquared
      -- sSquared = TRUNC(s * s / 1e18)  (rounds down)
      -- sNextSquared = TRUNC((sNext * sNext + 1e18 - 1) / 1e18)  (rounds up)
      WHEN p.curve_id = 2 THEN
        (SELECT GREATEST(
          raw_a
          -- protocol fee 1.25% (rounds up)
          - TRUNC((raw_a * 125 + 9999) / 10000)
          - CASE
              -- Exit fee only if default vault has >= 1e18 shares remaining
              -- For curve_id=2, default vault shares are unaffected by this redemption
              WHEN COALESCE(v_default.total_shares, 0) >= 1000000000000000000::NUMERIC
              -- exit fee 0.75% (rounds up)
              THEN TRUNC((raw_a * 75 + 9999) / 10000)
              ELSE 0
            END
        , 0)::NUMERIC
        FROM (SELECT TRUNC(
          (
            -- s^2 (rounds down: TRUNC of division)
            TRUNC((v.total_shares + 30000000000000000000::NUMERIC)
             * (v.total_shares + 30000000000000000000::NUMERIC)
             / 1000000000000000000::NUMERIC)
            -
            -- sNext^2 (rounds up: add 1e18-1 before TRUNC)
            TRUNC(((v.total_shares + 30000000000000000000::NUMERIC - p.shares)
              * (v.total_shares + 30000000000000000000::NUMERIC - p.shares)
              + 999999999999999999::NUMERIC)
             / 1000000000000000000::NUMERIC)
          ) * 50000000000000000::NUMERIC / 1000000000000000000::NUMERIC
        )::NUMERIC AS raw_a) calc)
      ELSE 0::NUMERIC
    END AS redeemable_assets
  FROM position p
  JOIN vault v ON v.term_id = p.term_id AND v.curve_id = p.curve_id
  LEFT JOIN vault v_default ON v_default.term_id = p.term_id AND v_default.curve_id = 1
) base;
