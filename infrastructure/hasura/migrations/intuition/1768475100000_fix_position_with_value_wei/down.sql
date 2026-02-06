-- Revert position_with_value to previous ETH-normalized version

CREATE OR REPLACE VIEW public.position_with_value AS
SELECT
  p.id,
  p.account_id,
  p.term_id,
  p.curve_id,
  p.shares,
  p.total_deposit_assets_after_total_fees,
  p.total_redeem_assets_for_receiver,
  p.block_number,
  p.log_index,
  p.transaction_hash,
  p.transaction_index,
  p.created_at,
  p.updated_at,
  -- Theoretical value (equity) in ETH
  (p.shares * v.current_share_price / 1e36)::NUMERIC AS theoretical_value,
  -- PnL = equity_value + redemptions - deposits (all in ETH)
  ((p.shares * v.current_share_price / 1e36) + (p.total_redeem_assets_for_receiver / 1e18) - (p.total_deposit_assets_after_total_fees / 1e18))::NUMERIC AS pnl,
  -- PnL percentage (ROI)
  CASE
    WHEN (p.total_deposit_assets_after_total_fees - p.total_redeem_assets_for_receiver) > 0
    THEN (((p.shares * v.current_share_price / 1e36) + (p.total_redeem_assets_for_receiver / 1e18) - (p.total_deposit_assets_after_total_fees / 1e18)) * 100.0
          / ((p.total_deposit_assets_after_total_fees - p.total_redeem_assets_for_receiver) / 1e18))::NUMERIC(20, 4)
    ELSE 0::NUMERIC(20, 4)
  END AS pnl_pct,
  -- Redeemable assets (previewRedeem value after fees) in ETH
  CASE
    WHEN p.shares = 0 THEN 0::NUMERIC
    -- Linear curve (curve_id = 1): rawAssets = shares * totalAssets / totalShares
    WHEN p.curve_id = 1 THEN
      GREATEST(
        (p.shares * v.total_assets / NULLIF(v.total_shares, 0))
        - ((p.shares * v.total_assets / NULLIF(v.total_shares, 0)) * 125 + 9999) / 10000  -- protocol fee 1.25%
        - ((p.shares * v.total_assets / NULLIF(v.total_shares, 0)) * 75 + 9999) / 10000   -- exit fee 0.75%
      , 0) / 1e18
    -- Offset Progressive curve (curve_id = 2): Quadratic bonding curve
    WHEN p.curve_id = 2 THEN
      (SELECT
        GREATEST(
          raw_assets - ((raw_assets * 125 + 9999) / 10000) - ((raw_assets * 75 + 9999) / 10000)
        , 0) / 1e18
      FROM (
        SELECT (
          (
            ((v.total_shares + 30000000000000000000::NUMERIC) * (v.total_shares + 30000000000000000000::NUMERIC) / 1e18)::NUMERIC
            -
            (((v.total_shares + 30000000000000000000::NUMERIC - p.shares) * (v.total_shares + 30000000000000000000::NUMERIC - p.shares) + 1e18 - 1) / 1e18)::NUMERIC
          )
          * 50000000000000000::NUMERIC / 1e18
        )::NUMERIC AS raw_assets
      ) curve_calc)
    ELSE 0::NUMERIC
  END AS redeemable_assets
FROM position p
JOIN vault v ON v.term_id = p.term_id AND v.curve_id = p.curve_id;
