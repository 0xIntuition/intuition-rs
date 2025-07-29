-- Add new fields to position table
ALTER TABLE position 
ADD COLUMN total_deposit_assets_after_total_fees NUMERIC(78, 0) NOT NULL DEFAULT 0,
ADD COLUMN total_redeem_assets_for_receiver NUMERIC(78, 0) NOT NULL DEFAULT 0;
