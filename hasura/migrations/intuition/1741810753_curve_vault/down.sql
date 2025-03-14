-- Remove comments
COMMENT ON COLUMN public.curve_vault.position_count IS NULL;
COMMENT ON COLUMN public.curve_vault.current_share_price IS NULL;
COMMENT ON COLUMN public.curve_vault.total_shares IS NULL;
COMMENT ON COLUMN public.curve_vault.curve_number IS NULL;
COMMENT ON COLUMN public.curve_vault.triple_id IS NULL;
COMMENT ON COLUMN public.curve_vault.atom_id IS NULL;
COMMENT ON TABLE public.curve_vault IS NULL;

-- Drop foreign key constraints if they exist
ALTER TABLE IF EXISTS public.curve_vault DROP CONSTRAINT IF EXISTS curve_vault_atom_id_fkey;
ALTER TABLE IF EXISTS public.curve_vault DROP CONSTRAINT IF EXISTS curve_vault_triple_id_fkey;

-- Drop indexes
DROP INDEX IF EXISTS public.curve_vault_triple_id_idx;
DROP INDEX IF EXISTS public.curve_vault_atom_id_idx;

-- Drop the curve_vault table
DROP TABLE IF EXISTS public.curve_vault; 