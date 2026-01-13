-- Rollback: Remove type discriminator column from term_text table

-- Drop the check constraint
ALTER TABLE term_text
DROP CONSTRAINT IF EXISTS check_term_text_type;

-- Drop the index
DROP INDEX IF EXISTS idx_term_text_type;

-- Drop the type column
ALTER TABLE term_text
DROP COLUMN IF EXISTS type;
