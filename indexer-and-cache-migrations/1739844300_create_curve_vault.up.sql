-- Create curve_vault table to track multiple vaults per atom/triple
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

-- Create indexes for faster lookups
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