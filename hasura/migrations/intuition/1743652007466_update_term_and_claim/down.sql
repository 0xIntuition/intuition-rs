-- Remove position_id from claim table
ALTER TABLE claim DROP CONSTRAINT claim_position_fkey;
ALTER TABLE claim DROP COLUMN position_id;

-- Restore removed columns to claim table
ALTER TABLE claim ADD COLUMN triple_id NUMERIC(78, 0);
ALTER TABLE claim ADD COLUMN subject_id NUMERIC(78, 0);
ALTER TABLE claim ADD COLUMN predicate_id NUMERIC(78, 0);
ALTER TABLE claim ADD COLUMN object_id NUMERIC(78, 0);
ALTER TABLE claim ADD COLUMN shares NUMERIC(78, 0);
ALTER TABLE claim ADD COLUMN counter_shares NUMERIC(78, 0);
ALTER TABLE claim ADD COLUMN term_id NUMERIC(78, 0);
ALTER TABLE claim ADD COLUMN curve_id NUMERIC(78, 0);
ALTER TABLE claim ADD COLUMN counter_term_id NUMERIC(78, 0);
ALTER TABLE claim ADD COLUMN counter_curve_id NUMERIC(78, 0);

-- Remove term_id and curve_id from signal table
ALTER TABLE signal DROP CONSTRAINT signal_vault_fkey;
ALTER TABLE signal DROP CONSTRAINT signal_term_fkey;
ALTER TABLE signal DROP COLUMN curve_id;
ALTER TABLE signal DROP COLUMN term_id;

-- Restore share_price_changed table relations
ALTER TABLE share_price_changed_curve DROP CONSTRAINT share_price_changed_curve_term_fkey;
ALTER TABLE share_price_changed_curve ADD CONSTRAINT share_price_changed_curve_term_id_fkey 
    FOREIGN KEY (term_id) REFERENCES vault(id);

-- Remove new columns from term table
ALTER TABLE term DROP COLUMN total_assets;
ALTER TABLE term DROP COLUMN total_theoretical_value_locked; 