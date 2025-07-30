-- Drop triggers
DROP TRIGGER IF EXISTS triple_vault_term_total_state_change_trigger ON triple_vault;
DROP TRIGGER IF EXISTS term_total_state_change_trigger ON term;

-- Drop functions
DROP FUNCTION IF EXISTS update_term_total_state_change_from_triple_vault();
DROP FUNCTION IF EXISTS update_term_total_state_change_from_term(); 