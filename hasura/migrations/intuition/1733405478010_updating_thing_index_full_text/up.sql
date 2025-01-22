DROP INDEX IF EXISTS idx_thing_description;
-- First, ensure the pg_trgm extension is enabled
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Then, create a GIN index on the description column
CREATE INDEX idx_thing_description_gin ON thing USING gin (description gin_trgm_ops);
