-- Remove the atom updated_at trigger

-- Drop the trigger
DROP TRIGGER IF EXISTS atom_updated_at_trigger ON atom;

-- Drop the function
DROP FUNCTION IF EXISTS update_atom_updated_at();
