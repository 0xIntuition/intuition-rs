-- Drop new foreign key constraints
DO $$ 
BEGIN
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'deposit_vault_fkey') THEN
        ALTER TABLE deposit DROP CONSTRAINT deposit_vault_fkey;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'redemption_vault_fkey') THEN
        ALTER TABLE redemption DROP CONSTRAINT redemption_vault_fkey;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'position_vault_fkey') THEN
        ALTER TABLE position DROP CONSTRAINT position_vault_fkey;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'claim_vault_fkey') THEN
        ALTER TABLE claim DROP CONSTRAINT claim_vault_fkey;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'claim_counter_vault_fkey') THEN
        ALTER TABLE claim DROP CONSTRAINT claim_counter_vault_fkey;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'atom_pkey') THEN
        ALTER TABLE atom DROP CONSTRAINT atom_pkey;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'triple_pkey') THEN
        ALTER TABLE triple DROP CONSTRAINT triple_pkey;
    END IF;
END $$;

-- Drop junction tables
DROP TABLE IF EXISTS atom_vault;
DROP TABLE IF EXISTS triple_vault;

-- Drop new curve_id columns
DO $$ 
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'deposit' AND column_name = 'curve_id') THEN
        ALTER TABLE deposit DROP COLUMN curve_id;
    END IF;
    
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'redemption' AND column_name = 'curve_id') THEN
        ALTER TABLE redemption DROP COLUMN curve_id;
    END IF;
    
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'position' AND column_name = 'curve_id') THEN
        ALTER TABLE position DROP COLUMN curve_id;
    END IF;
    
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'claim' AND column_name = 'curve_id') THEN
        ALTER TABLE claim DROP COLUMN curve_id;
    END IF;
    
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'claim' AND column_name = 'counter_curve_id') THEN
        ALTER TABLE claim DROP COLUMN counter_curve_id;
    END IF;
END $$;

-- Rename term_id back to vault_id
DO $$ 
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'deposit' AND column_name = 'term_id') THEN
        ALTER TABLE deposit RENAME COLUMN term_id TO vault_id;
    END IF;
    
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'redemption' AND column_name = 'term_id') THEN
        ALTER TABLE redemption RENAME COLUMN term_id TO vault_id;
    END IF;
    
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'position' AND column_name = 'term_id') THEN
        ALTER TABLE position RENAME COLUMN term_id TO vault_id;
    END IF;
    
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'claim' AND column_name = 'term_id') THEN
        ALTER TABLE claim RENAME COLUMN term_id TO vault_id;
    END IF;
    
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'claim' AND column_name = 'counter_term_id') THEN
        ALTER TABLE claim RENAME COLUMN counter_term_id TO counter_vault_id;
    END IF;
    
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'triple' AND column_name = 'term_id') THEN
        ALTER TABLE triple RENAME COLUMN term_id TO vault_id;
    END IF;
    
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'triple' AND column_name = 'counter_term_id') THEN
        ALTER TABLE triple RENAME COLUMN counter_term_id TO counter_vault_id;
    END IF;
    
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'atom' AND column_name = 'term_id') THEN
        ALTER TABLE atom RENAME COLUMN term_id TO vault_id;
    END IF;
END $$;

-- Drop indexes
DO $$ 
BEGIN
    IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'idx_vault_term_id') THEN
        DROP INDEX idx_vault_term_id;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'idx_curve_id') THEN
        DROP INDEX idx_curve_id;
    END IF;
END $$;

-- Drop composite primary key
DO $$ 
BEGIN
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'vault_pkey') THEN
        ALTER TABLE vault DROP CONSTRAINT vault_pkey;
    END IF;
END $$;

-- Add back original columns
ALTER TABLE vault ADD COLUMN id NUMERIC(78, 0);
ALTER TABLE vault ADD COLUMN atom_id NUMERIC(78, 0);
ALTER TABLE vault ADD COLUMN triple_id NUMERIC(78, 0);

-- Add back original primary key
ALTER TABLE vault ADD PRIMARY KEY (id);

-- Migrate data back
UPDATE vault 
SET atom_id = term_id 
WHERE term_id IS NOT NULL;

UPDATE vault 
SET triple_id = term_id 
WHERE term_id IS NOT NULL;

-- Drop term_id
ALTER TABLE vault DROP COLUMN term_id;

-- Add back id columns to atom and triple
ALTER TABLE atom ADD COLUMN id NUMERIC(78, 0);
ALTER TABLE triple ADD COLUMN id NUMERIC(78, 0);

-- Set id values from term_id
UPDATE atom SET id = term_id;
UPDATE triple SET id = term_id;

-- Make id NOT NULL and primary key
ALTER TABLE atom ALTER COLUMN id SET NOT NULL;
ALTER TABLE triple ALTER COLUMN id SET NOT NULL;
ALTER TABLE atom ADD PRIMARY KEY (id);
ALTER TABLE triple ADD PRIMARY KEY (id);

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
ALTER TABLE atom ADD CONSTRAINT atom_vault_id_fkey
    FOREIGN KEY (vault_id) REFERENCES vault(id);
