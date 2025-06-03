-- Remove term_id and curve_id from signal table
ALTER TABLE signal DROP CONSTRAINT signal_vault_fkey;
ALTER TABLE signal DROP CONSTRAINT signal_term_fkey;
ALTER TABLE signal DROP COLUMN curve_id;
ALTER TABLE signal DROP COLUMN term_id;

-- Restore share_price_change table relations
ALTER TABLE share_price_change DROP CONSTRAINT share_price_change_term_fkey;
ALTER TABLE share_price_change ADD CONSTRAINT share_price_change_term_id_fkey 
    FOREIGN KEY (term_id) REFERENCES vault(id);

-- Remove new columns from term table
ALTER TABLE term DROP COLUMN total_assets;
ALTER TABLE term DROP COLUMN total_market_cap; 