-- Remove new fields from position table
ALTER TABLE position 
DROP COLUMN total_deposit_assets_after_total_fees,
DROP COLUMN total_redeem_assets_for_receiver;