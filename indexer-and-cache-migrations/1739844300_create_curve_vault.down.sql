-- Drop foreign key constraints if they exist
ALTER TABLE IF EXISTS public.curve_vault DROP CONSTRAINT IF EXISTS curve_vault_atom_id_fkey;
ALTER TABLE IF EXISTS public.curve_vault DROP CONSTRAINT IF EXISTS curve_vault_triple_id_fkey;

-- Drop indexes first
DROP INDEX IF EXISTS public.curve_vault_triple_id_idx;
DROP INDEX IF EXISTS public.curve_vault_atom_id_idx;

-- Drop the curve_vault table
DROP TABLE IF EXISTS public.curve_vault;