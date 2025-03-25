-- Add temporary string ID column to vault
ALTER TABLE vault ADD COLUMN temp_id TEXT;
UPDATE vault SET temp_id = CONCAT(id::text, '-', curve_id::text);

-- Drop foreign key constraints from ALL referencing tables
ALTER TABLE deposit DROP CONSTRAINT deposit_vault_id_fkey;
ALTER TABLE redemption DROP CONSTRAINT redemption_vault_id_fkey;
ALTER TABLE position DROP CONSTRAINT position_vault_id_fkey;
ALTER TABLE claim DROP CONSTRAINT claim_vault_id_fkey;
ALTER TABLE claim DROP CONSTRAINT claim_counter_vault_id_fkey;
ALTER TABLE share_price_changed DROP CONSTRAINT share_price_changed_term_id_fkey;
ALTER TABLE share_price_changed_curve DROP CONSTRAINT share_price_changed_curve_term_id_fkey;

-- Add temporary columns to referencing tables
ALTER TABLE deposit ADD COLUMN temp_vault_id TEXT;
ALTER TABLE redemption ADD COLUMN temp_vault_id TEXT;
ALTER TABLE position ADD COLUMN temp_vault_id TEXT;
ALTER TABLE claim ADD COLUMN temp_vault_id TEXT;
ALTER TABLE claim ADD COLUMN temp_counter_vault_id TEXT;
ALTER TABLE share_price_changed ADD COLUMN temp_term_id TEXT;
ALTER TABLE share_price_changed_curve ADD COLUMN temp_term_id TEXT;
ALTER TABLE triple ADD COLUMN temp_vault_id TEXT;
ALTER TABLE triple ADD COLUMN temp_counter_vault_id TEXT;

-- Update the temporary columns
UPDATE deposit SET temp_vault_id = CONCAT(vault_id::text, '-', '1');
UPDATE redemption SET temp_vault_id = CONCAT(vault_id::text, '-', '1');
UPDATE position SET temp_vault_id = CONCAT(vault_id::text, '-', '1');
UPDATE claim SET 
    temp_vault_id = CONCAT(vault_id::text, '-', '1'),
    temp_counter_vault_id = CONCAT(counter_vault_id::text, '-', '1');
UPDATE share_price_changed SET temp_term_id = CONCAT(term_id::text, '-', '1');
UPDATE share_price_changed_curve SET temp_term_id = CONCAT(term_id::text, '-', '1');
UPDATE triple SET 
    temp_vault_id = CONCAT(vault_id::text, '-', '1'),
    temp_counter_vault_id = CONCAT(counter_vault_id::text, '-', '1');

-- Drop old columns and rename temp columns
ALTER TABLE vault DROP CONSTRAINT vault_pkey CASCADE;
ALTER TABLE vault DROP COLUMN id;
ALTER TABLE vault RENAME COLUMN temp_id TO id;
ALTER TABLE vault ADD PRIMARY KEY (id);

ALTER TABLE deposit DROP COLUMN vault_id;
ALTER TABLE deposit RENAME COLUMN temp_vault_id TO vault_id;

ALTER TABLE redemption DROP COLUMN vault_id;
ALTER TABLE redemption RENAME COLUMN temp_vault_id TO vault_id;

ALTER TABLE position DROP COLUMN vault_id;
ALTER TABLE position RENAME COLUMN temp_vault_id TO vault_id;

ALTER TABLE claim DROP COLUMN vault_id;
ALTER TABLE claim DROP COLUMN counter_vault_id;
ALTER TABLE claim RENAME COLUMN temp_vault_id TO vault_id;
ALTER TABLE claim RENAME COLUMN temp_counter_vault_id TO counter_vault_id;

ALTER TABLE share_price_changed DROP COLUMN term_id;
ALTER TABLE share_price_changed RENAME COLUMN temp_term_id TO term_id;

ALTER TABLE share_price_changed_curve DROP COLUMN term_id;
ALTER TABLE share_price_changed_curve RENAME COLUMN temp_term_id TO term_id;

ALTER TABLE triple DROP COLUMN vault_id;
ALTER TABLE triple DROP COLUMN counter_vault_id;
ALTER TABLE triple RENAME COLUMN temp_vault_id TO vault_id;
ALTER TABLE triple RENAME COLUMN temp_counter_vault_id TO counter_vault_id;

-- Add back foreign key constraints
ALTER TABLE deposit ADD CONSTRAINT deposit_vault_id_fkey 
    FOREIGN KEY (vault_id) REFERENCES vault(id);
ALTER TABLE redemption ADD CONSTRAINT redemption_vault_id_fkey 
    FOREIGN KEY (vault_id) REFERENCES vault(id);
ALTER TABLE position ADD CONSTRAINT position_vault_id_fkey 
    FOREIGN KEY (vault_id) REFERENCES vault(id);
ALTER TABLE claim ADD CONSTRAINT claim_vault_id_fkey 
    FOREIGN KEY (vault_id) REFERENCES vault(id);
ALTER TABLE claim ADD CONSTRAINT claim_counter_vault_id_fkey 
    FOREIGN KEY (counter_vault_id) REFERENCES vault(id);
ALTER TABLE share_price_changed ADD CONSTRAINT share_price_changed_term_id_fkey
    FOREIGN KEY (term_id) REFERENCES vault(id);
ALTER TABLE share_price_changed_curve ADD CONSTRAINT share_price_changed_curve_term_id_fkey
    FOREIGN KEY (term_id) REFERENCES vault(id);
ALTER TABLE triple ADD CONSTRAINT triple_vault_id_fkey
    FOREIGN KEY (vault_id) REFERENCES vault(id);
ALTER TABLE triple ADD CONSTRAINT triple_counter_vault_id_fkey
    FOREIGN KEY (counter_vault_id) REFERENCES vault(id);

-- Add temporary column to atom
ALTER TABLE atom ADD COLUMN temp_vault_id TEXT;

-- Update the temporary column
UPDATE atom SET temp_vault_id = CONCAT(vault_id::text, '-', '1');

-- Drop old column and rename temp column
ALTER TABLE atom DROP COLUMN vault_id;
ALTER TABLE atom RENAME COLUMN temp_vault_id TO vault_id;

-- Add back foreign key constraint
ALTER TABLE atom ADD CONSTRAINT atom_vault_id_fkey
    FOREIGN KEY (vault_id) REFERENCES vault(id);