-- Add position_with_value view for efficient querying of position values
-- All monetary values are normalized to ETH (divided by 1e18)

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
  END AS pnl_pct
FROM position p
JOIN vault v ON v.term_id = p.term_id AND v.curve_id = p.curve_id;
