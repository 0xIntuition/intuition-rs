-- Drop indexes first
DROP INDEX IF EXISTS public.curve_vault_triple_id_idx;
DROP INDEX IF EXISTS public.curve_vault_atom_id_idx;

-- Drop the curve_vault table
DROP TABLE IF EXISTS public.curve_vault;