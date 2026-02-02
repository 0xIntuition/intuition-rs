-- Add type discriminator column to term_text table
-- This migration adds a 'type' column to distinguish between atoms and triples

-- Add type column with default value 'atom' for existing rows (if it doesn't already exist)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'term_text' AND column_name = 'type'
    ) THEN
        ALTER TABLE term_text ADD COLUMN type TEXT NOT NULL DEFAULT 'atom';
    END IF;
END $$;

-- Add index on type column for query performance
CREATE INDEX IF NOT EXISTS idx_term_text_type ON term_text(type);

-- Add check constraint to ensure type is either 'atom' or 'triple'
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'check_term_text_type'
    ) THEN
        ALTER TABLE term_text ADD CONSTRAINT check_term_text_type
        CHECK (type IN ('atom', 'triple'));
    END IF;
END $$;

-- Comment on the column to document its purpose
COMMENT ON COLUMN term_text.type IS 'Discriminator column to distinguish between atoms and triples. Valid values: atom, triple';
