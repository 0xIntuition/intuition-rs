-- Drop all foreign key constraints with CASCADE
DO $$ 
BEGIN
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'deposit_vault_id_fkey') THEN
        ALTER TABLE deposit DROP CONSTRAINT deposit_vault_id_fkey CASCADE;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'redemption_vault_id_fkey') THEN
        ALTER TABLE redemption DROP CONSTRAINT redemption_vault_id_fkey CASCADE;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'position_vault_id_fkey') THEN
        ALTER TABLE position DROP CONSTRAINT position_vault_id_fkey CASCADE;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'claim_vault_id_fkey') THEN
        ALTER TABLE claim DROP CONSTRAINT claim_vault_id_fkey CASCADE;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'claim_counter_vault_id_fkey') THEN
        ALTER TABLE claim DROP CONSTRAINT claim_counter_vault_id_fkey CASCADE;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'share_price_changed_curve_term_id_fkey') THEN
        ALTER TABLE share_price_changed_curve DROP CONSTRAINT share_price_changed_curve_term_id_fkey CASCADE;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'triple_vault_id_fkey') THEN
        ALTER TABLE triple DROP CONSTRAINT triple_vault_id_fkey CASCADE;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'triple_counter_vault_id_fkey') THEN
        ALTER TABLE triple DROP CONSTRAINT triple_counter_vault_id_fkey CASCADE;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'atom_vault_id_fkey') THEN
        ALTER TABLE atom DROP CONSTRAINT atom_vault_id_fkey CASCADE;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'vault_pkey') THEN
        ALTER TABLE vault DROP CONSTRAINT vault_pkey CASCADE;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'atom_pkey') THEN
        ALTER TABLE atom DROP CONSTRAINT atom_pkey CASCADE;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'triple_pkey') THEN
        ALTER TABLE triple DROP CONSTRAINT triple_pkey CASCADE;
    END IF;
END $$;

-- Create term_type enum
CREATE TYPE term_type AS ENUM ('Atom', 'Triple');

-- Create term table
CREATE TABLE term (
    id NUMERIC(78, 0) PRIMARY KEY,
    type term_type NOT NULL,
    atom_id NUMERIC(78, 0),
    triple_id NUMERIC(78, 0)
);

-- Add term_id column
ALTER TABLE vault ADD COLUMN term_id NUMERIC(78, 0);

-- Migrate data from atom_id/triple_id to term_id
UPDATE vault 
SET term_id = COALESCE(atom_id, triple_id)
WHERE atom_id IS NOT NULL OR triple_id IS NOT NULL;

-- Insert data into term table
INSERT INTO term (id, type, atom_id, triple_id)
SELECT 
    COALESCE(atom_id, triple_id) as id,
    CASE 
        WHEN atom_id IS NOT NULL THEN 'Atom'::term_type
        ELSE 'Triple'::term_type
    END as type,
    atom_id,
    triple_id
FROM vault
WHERE atom_id IS NOT NULL OR triple_id IS NOT NULL;

-- Add indexes for term table
CREATE INDEX idx_term_id ON term(id);
CREATE INDEX idx_term_type ON term(type);
CREATE INDEX idx_term_atom_id ON term(atom_id);
CREATE INDEX idx_term_triple_id ON term(triple_id);

-- Drop old columns
ALTER TABLE vault DROP COLUMN atom_id;
ALTER TABLE vault DROP COLUMN triple_id;
ALTER TABLE vault DROP COLUMN id;

-- Make term_id and curve_id NOT NULL
ALTER TABLE vault ALTER COLUMN term_id SET NOT NULL;

-- Add composite primary key
ALTER TABLE vault ADD PRIMARY KEY (term_id, curve_id);

-- Create indexes
CREATE INDEX idx_vault_term_id ON vault(term_id);
CREATE INDEX idx_curve_id ON vault(curve_id);

-- Rename vault_id to term_id in all tables
ALTER TABLE deposit RENAME COLUMN vault_id TO term_id;
ALTER TABLE redemption RENAME COLUMN vault_id TO term_id;
ALTER TABLE position RENAME COLUMN vault_id TO term_id;
ALTER TABLE claim RENAME COLUMN vault_id TO term_id;
ALTER TABLE claim RENAME COLUMN counter_vault_id TO counter_term_id;
ALTER TABLE triple RENAME COLUMN vault_id TO term_id;
ALTER TABLE triple RENAME COLUMN counter_vault_id TO counter_term_id;
ALTER TABLE atom RENAME COLUMN vault_id TO term_id;

-- Drop id columns from atom and triple
ALTER TABLE atom DROP COLUMN id;
ALTER TABLE triple DROP COLUMN id;

-- Make term_id NOT NULL and primary key in atom and triple
ALTER TABLE atom ALTER COLUMN term_id SET NOT NULL;
ALTER TABLE triple ALTER COLUMN term_id SET NOT NULL;
ALTER TABLE atom ADD PRIMARY KEY (term_id);
ALTER TABLE triple ADD PRIMARY KEY (term_id);

-- Add new columns for composite key references
ALTER TABLE deposit ADD COLUMN curve_id NUMERIC(78, 0);
ALTER TABLE redemption ADD COLUMN curve_id NUMERIC(78, 0);
ALTER TABLE position ADD COLUMN curve_id NUMERIC(78, 0);
ALTER TABLE claim ADD COLUMN curve_id NUMERIC(78, 0);
ALTER TABLE claim ADD COLUMN counter_curve_id NUMERIC(78, 0);

-- Update the new columns with curve_id values
UPDATE deposit SET curve_id = 1;
UPDATE redemption SET curve_id = 1;
UPDATE position SET curve_id = 1;
UPDATE claim SET curve_id = 1, counter_curve_id = 1;

-- Make the new columns NOT NULL
ALTER TABLE deposit ALTER COLUMN curve_id SET NOT NULL;
ALTER TABLE redemption ALTER COLUMN curve_id SET NOT NULL;
ALTER TABLE position ALTER COLUMN curve_id SET NOT NULL;
ALTER TABLE claim ALTER COLUMN curve_id SET NOT NULL;
ALTER TABLE claim ALTER COLUMN counter_curve_id SET NOT NULL;

-- Add foreign key constraints to term table
ALTER TABLE vault ADD CONSTRAINT vault_term_fkey 
    FOREIGN KEY (term_id) REFERENCES term(id);
ALTER TABLE atom ADD CONSTRAINT atom_term_fkey 
    FOREIGN KEY (term_id) REFERENCES term(id);
ALTER TABLE triple ADD CONSTRAINT triple_term_fkey 
    FOREIGN KEY (term_id) REFERENCES term(id);
ALTER TABLE deposit ADD CONSTRAINT deposit_term_fkey 
    FOREIGN KEY (term_id) REFERENCES term(id);
ALTER TABLE redemption ADD CONSTRAINT redemption_term_fkey 
    FOREIGN KEY (term_id) REFERENCES term(id);
ALTER TABLE position ADD CONSTRAINT position_term_fkey 
    FOREIGN KEY (term_id) REFERENCES term(id);
ALTER TABLE claim ADD CONSTRAINT claim_term_fkey 
    FOREIGN KEY (term_id) REFERENCES term(id);
ALTER TABLE claim ADD CONSTRAINT claim_counter_term_fkey 
    FOREIGN KEY (counter_term_id) REFERENCES term(id);

-- Add new foreign key constraints for vault references
ALTER TABLE deposit ADD CONSTRAINT deposit_vault_fkey 
    FOREIGN KEY (term_id, curve_id) REFERENCES vault(term_id, curve_id);
ALTER TABLE redemption ADD CONSTRAINT redemption_vault_fkey 
    FOREIGN KEY (term_id, curve_id) REFERENCES vault(term_id, curve_id);
ALTER TABLE position ADD CONSTRAINT position_vault_fkey 
    FOREIGN KEY (term_id, curve_id) REFERENCES vault(term_id, curve_id);
ALTER TABLE claim ADD CONSTRAINT claim_vault_fkey 
    FOREIGN KEY (term_id, curve_id) REFERENCES vault(term_id, curve_id);
ALTER TABLE claim ADD CONSTRAINT claim_counter_vault_fkey 
    FOREIGN KEY (counter_term_id, counter_curve_id) REFERENCES vault(term_id, curve_id);

-- Create a temporary table to store position counts
CREATE TEMPORARY TABLE temp_position_counts AS
SELECT term_id, curve_id, COUNT(*) as count
FROM position
GROUP BY term_id, curve_id;

-- Update vault position counts
UPDATE vault v
SET position_count = COALESCE(t.count, 0)
FROM temp_position_counts t
WHERE v.term_id = t.term_id AND v.curve_id = t.curve_id;

-- Drop the temporary table
DROP TABLE temp_position_counts;
