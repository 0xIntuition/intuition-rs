-- Rollback: Remove assets column and triggers from position table

-- Drop triggers
DROP TRIGGER IF EXISTS trg_position_assets_on_position ON position;
DROP TRIGGER IF EXISTS trg_position_assets_on_vault ON vault;

-- Drop trigger functions
DROP FUNCTION IF EXISTS update_position_assets_on_position_change();
DROP FUNCTION IF EXISTS update_position_assets_on_vault_change();

-- Drop indexes
DROP INDEX IF EXISTS idx_position_assets;
DROP INDEX IF EXISTS idx_position_account_assets;

-- Drop column
ALTER TABLE position DROP COLUMN IF EXISTS assets;
