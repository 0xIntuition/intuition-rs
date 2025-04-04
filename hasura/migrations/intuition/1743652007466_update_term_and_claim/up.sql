-- Add new columns to term table
ALTER TABLE term ADD COLUMN total_assets NUMERIC(78, 0);
ALTER TABLE term ADD COLUMN total_theoretical_value_locked NUMERIC(78, 0);

-- Add position_id to claim table
ALTER TABLE claim ADD COLUMN position_id TEXT;

-- Add term_id and curve_id to signal table
ALTER TABLE signal ADD COLUMN term_id NUMERIC(78, 0);
ALTER TABLE signal ADD COLUMN curve_id NUMERIC(78, 0);

-- Migrate data to set term_id in signal table
UPDATE signal s
SET term_id = COALESCE(a.term_id, t.term_id)
FROM atom a
FULL OUTER JOIN triple t ON t.term_id = a.term_id
WHERE (s.atom_id IS NOT NULL AND s.atom_id = a.term_id)
   OR (s.triple_id IS NOT NULL AND s.triple_id = t.term_id);

-- Set curve_id to 1 for all signals (default curve)
UPDATE signal SET curve_id = 1;

-- Make term_id and curve_id NOT NULL after migration
ALTER TABLE signal ALTER COLUMN term_id SET NOT NULL;
ALTER TABLE signal ALTER COLUMN curve_id SET NOT NULL;

-- Add foreign key constraints
ALTER TABLE signal ADD CONSTRAINT signal_term_fkey 
    FOREIGN KEY (term_id) REFERENCES term(id);
ALTER TABLE signal ADD CONSTRAINT signal_vault_fkey 
    FOREIGN KEY (term_id, curve_id) REFERENCES vault(term_id, curve_id);

-- Migrate data to set position_id in claim table
UPDATE claim c
SET position_id = p.id
FROM position p
WHERE c.account_id = p.account_id 
AND c.term_id = p.term_id 
AND c.curve_id = p.curve_id;

-- Make position_id NOT NULL after migration
ALTER TABLE claim ALTER COLUMN position_id SET NOT NULL;

-- Add foreign key constraint for position_id
ALTER TABLE claim ADD CONSTRAINT claim_position_fkey 
    FOREIGN KEY (position_id) REFERENCES position(id);

-- Verify term relations
DO $$ 
BEGIN
    -- Verify triple term_id relation
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'triple_term_fkey'
    ) THEN
        RAISE EXCEPTION 'triple term_id relation is missing';
    END IF;

    -- Verify atom term_id relation
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'atom_term_fkey'
    ) THEN
        RAISE EXCEPTION 'atom term_id relation is missing';
    END IF;

    -- Verify position term_id relation
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'position_term_fkey'
    ) THEN
        RAISE EXCEPTION 'position term_id relation is missing';
    END IF;

    -- Verify vault term_id relation
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'vault_term_fkey'
    ) THEN
        RAISE EXCEPTION 'vault term_id relation is missing';
    END IF;

    -- Verify redemption term_id relation
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'redemption_term_fkey'
    ) THEN
        RAISE EXCEPTION 'redemption term_id relation is missing';
    END IF;

    -- Verify signal term_id relation
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'signal_term_fkey'
    ) THEN
        RAISE EXCEPTION 'signal term_id relation is missing';
    END IF;

    -- Verify deposit term_id relation
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'deposit_term_fkey'
    ) THEN
        RAISE EXCEPTION 'deposit term_id relation is missing';
    END IF;
END $$; 