-- Rollback ProtocolFeeAccrued event support

-- Remove protocol_fee_accrued_id column from event table
ALTER TABLE event DROP COLUMN IF EXISTS protocol_fee_accrued_id;

-- Drop protocol_fee_accrued table and indexes
DROP TABLE IF EXISTS protocol_fee_accrued;

-- Note: PostgreSQL does not support removing values from enums.
-- The 'ProtocolFeeAccrued' value will remain in the event_type enum.
