-- Rollback migration for atom update term_text trigger
DROP TRIGGER IF EXISTS atom_update_term_text_trigger ON atom;
