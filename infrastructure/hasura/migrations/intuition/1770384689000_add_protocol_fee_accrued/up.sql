-- Add ProtocolFeeAccrued event support

-- Add new value to event_type enum
ALTER TYPE event_type ADD VALUE IF NOT EXISTS 'ProtocolFeeAccrued';

-- Create protocol_fee_accrued table
CREATE TABLE IF NOT EXISTS protocol_fee_accrued (
  id TEXT PRIMARY KEY NOT NULL,
  epoch NUMERIC(78, 0) NOT NULL,
  sender_id TEXT NOT NULL,
  amount NUMERIC(78, 0) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL
);

-- Add indexes
CREATE INDEX IF NOT EXISTS idx_protocol_fee_accrued_sender_id ON protocol_fee_accrued(sender_id);
CREATE INDEX IF NOT EXISTS idx_protocol_fee_accrued_epoch ON protocol_fee_accrued(epoch);
CREATE INDEX IF NOT EXISTS idx_protocol_fee_accrued_block_number ON protocol_fee_accrued(block_number);
CREATE INDEX IF NOT EXISTS idx_protocol_fee_accrued_created_at ON protocol_fee_accrued(created_at);

-- Add protocol_fee_accrued_id column to event table
ALTER TABLE event ADD COLUMN IF NOT EXISTS protocol_fee_accrued_id TEXT;
