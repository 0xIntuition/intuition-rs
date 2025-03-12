-- Create curve_vault table to track multiple vaults per atom/triple
CREATE TABLE IF NOT EXISTS public.curve_vault (
    id NUMERIC PRIMARY KEY,
    atom_id NUMERIC REFERENCES public.atom(id),
    triple_id NUMERIC REFERENCES public.triple(id),
    vault_number INTEGER NOT NULL,
    total_shares NUMERIC NOT NULL DEFAULT 0,
    current_share_price NUMERIC NOT NULL DEFAULT 0,
    position_count INTEGER NOT NULL DEFAULT 0,
    CHECK (atom_id IS NOT NULL OR triple_id IS NOT NULL),
    CHECK (atom_id IS NULL OR triple_id IS NULL),
    CONSTRAINT vault_number_check CHECK (vault_number > 1)
);

-- Create indexes for faster lookups
CREATE INDEX IF NOT EXISTS curve_vault_atom_id_idx ON public.curve_vault(atom_id);
CREATE INDEX IF NOT EXISTS curve_vault_triple_id_idx ON public.curve_vault(triple_id);