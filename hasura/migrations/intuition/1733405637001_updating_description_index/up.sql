DROP INDEX IF EXISTS idx_person_description;
DROP INDEX IF EXISTS idx_organization_description;

-- First, ensure the pg_trgm extension is enabled
CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX idx_person_description ON person USING gin (description gin_trgm_ops);
CREATE INDEX idx_organization_description ON organization USING gin (description gin_trgm_ops);
