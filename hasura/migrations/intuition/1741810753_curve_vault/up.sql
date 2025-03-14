-- Track the curve_vault table in Hasura

-- This migration tracks the curve_vault table that was created in the indexer migrations
-- The table definition is in indexer-and-cache-migrations/1739844300_create_curve_vault.up.sql

-- Create the curve_vault table if it doesn't exist
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Create the curve_vault table if it doesn't exist
CREATE TABLE IF NOT EXISTS public.curve_vault (
    id NUMERIC PRIMARY KEY,
    atom_id NUMERIC,
    triple_id NUMERIC,
    curve_number NUMERIC NOT NULL,
    total_shares NUMERIC NOT NULL DEFAULT 0,
    current_share_price NUMERIC NOT NULL DEFAULT 0,
    position_count INTEGER NOT NULL DEFAULT 0,
    CHECK (atom_id IS NOT NULL OR triple_id IS NOT NULL),
    CHECK (atom_id IS NULL OR triple_id IS NULL),
    CONSTRAINT curve_number_check CHECK (curve_number > 1)
);

-- Create indexes for faster lookups if they don't exist
CREATE INDEX IF NOT EXISTS curve_vault_atom_id_idx ON public.curve_vault(atom_id) WHERE atom_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS curve_vault_triple_id_idx ON public.curve_vault(triple_id) WHERE triple_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS curve_vault_composite_idx ON public.curve_vault(atom_id, triple_id, curve_number);

-- Add foreign key constraints if the referenced tables exist
DO $$
BEGIN
    -- Check if atom table exists before adding foreign key constraint
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'public' AND table_name = 'atom') THEN
        -- Add foreign key constraint for atom_id if it doesn't exist
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.table_constraints 
            WHERE constraint_schema = 'public' 
            AND table_name = 'curve_vault' 
            AND constraint_name = 'curve_vault_atom_id_fkey'
        ) THEN
            ALTER TABLE public.curve_vault 
            ADD CONSTRAINT curve_vault_atom_id_fkey 
            FOREIGN KEY (atom_id) REFERENCES public.atom(id);
        END IF;
    END IF;

    -- Check if triple table exists before adding foreign key constraint
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'public' AND table_name = 'triple') THEN
        -- Add foreign key constraint for triple_id if it doesn't exist
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.table_constraints 
            WHERE constraint_schema = 'public' 
            AND table_name = 'curve_vault' 
            AND constraint_name = 'curve_vault_triple_id_fkey'
        ) THEN
            ALTER TABLE public.curve_vault 
            ADD CONSTRAINT curve_vault_triple_id_fkey 
            FOREIGN KEY (triple_id) REFERENCES public.triple(id);
        END IF;
    END IF;
END
$$;

-- Add comments for better documentation
COMMENT ON TABLE public.curve_vault IS 'Table to track multiple vaults per atom/triple (vaults 2-N)';
COMMENT ON COLUMN public.curve_vault.id IS 'Unique identifier for the curve vault';
COMMENT ON COLUMN public.curve_vault.atom_id IS 'Reference to the atom this vault belongs to (if applicable)';
COMMENT ON COLUMN public.curve_vault.triple_id IS 'Reference to the triple this vault belongs to (if applicable)';
COMMENT ON COLUMN public.curve_vault.curve_number IS 'The curve number (2, 3, 4, etc.) where 1 is the original vault in the vault table';
COMMENT ON COLUMN public.curve_vault.total_shares IS 'Total shares in this vault';
COMMENT ON COLUMN public.curve_vault.current_share_price IS 'Current share price in this vault';
COMMENT ON COLUMN public.curve_vault.position_count IS 'Number of positions in this vault'; 