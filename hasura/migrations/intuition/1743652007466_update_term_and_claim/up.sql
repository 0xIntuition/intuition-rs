-- Add new columns to term table
ALTER TABLE term ADD COLUMN total_assets NUMERIC(78, 0);
ALTER TABLE term ADD COLUMN total_market_cap NUMERIC(78, 0);

-- Add term_id and curve_id to signal table
ALTER TABLE signal ADD COLUMN term_id NUMERIC(78, 0);
ALTER TABLE signal ADD COLUMN curve_id NUMERIC(78, 0);

-- Fix share_price_change table relations
DO $$ 
BEGIN
    -- Check if share_price_change table exists
    IF EXISTS (SELECT 1 FROM pg_tables WHERE tablename = 'share_price_change') THEN
        IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'share_price_change_term_id_fkey') THEN
            ALTER TABLE share_price_change DROP CONSTRAINT share_price_change_term_id_fkey;
        END IF;
        
        ALTER TABLE share_price_change ADD CONSTRAINT share_price_change_term_fkey 
            FOREIGN KEY (term_id) REFERENCES term(id);
    END IF;
    
    -- Check if share_price_change_curve table exists
    IF EXISTS (SELECT 1 FROM pg_tables WHERE tablename = 'share_price_change_curve') THEN
        IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'share_price_change_curve_term_id_fkey') THEN
            ALTER TABLE share_price_change_curve DROP CONSTRAINT share_price_change_curve_term_id_fkey;
        END IF;
        
        ALTER TABLE share_price_change_curve ADD CONSTRAINT share_price_change_curve_term_fkey 
            FOREIGN KEY (term_id) REFERENCES term(id);
    END IF;
END $$;

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