-- Drop the trigger
DROP TRIGGER IF EXISTS vault_term_totals_trigger ON vault;

-- Drop the function
DROP FUNCTION IF EXISTS update_term_totals(); 