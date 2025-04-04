-- Remove position_id from claim table
ALTER TABLE claim DROP CONSTRAINT claim_position_fkey;
ALTER TABLE claim DROP COLUMN position_id;

-- Remove term_id from signal table
ALTER TABLE signal DROP CONSTRAINT signal_term_fkey;
ALTER TABLE signal DROP COLUMN term_id;

-- Remove new columns from term table
ALTER TABLE term DROP COLUMN total_assets;
ALTER TABLE term DROP COLUMN total_theoretical_value_locked; 