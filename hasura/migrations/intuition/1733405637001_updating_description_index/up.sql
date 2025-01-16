DROP INDEX IF EXISTS base_sepolia_backend.idx_person_description;
DROP INDEX IF EXISTS base_sepolia_backend.idx_organization_description;

-- First, ensure the pg_trgm extension is enabled
CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX idx_person_description ON base_sepolia_backend.person USING gin (description gin_trgm_ops);
CREATE INDEX idx_organization_description ON base_sepolia_backend.organization USING gin (description gin_trgm_ops);
