-- Add position_with_value view for efficient querying of position values

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
  p.shares * v.current_share_price AS theoretical_value
FROM position p
JOIN vault v ON v.term_id = p.term_id AND v.curve_id = p.curve_id;
