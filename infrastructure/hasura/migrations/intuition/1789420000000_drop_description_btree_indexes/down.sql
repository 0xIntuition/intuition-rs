-- Re-creating these will fail if any row has a description longer than the btree
-- tuple limit; that is the reason they were dropped.
CREATE INDEX IF NOT EXISTS idx_thing_description ON thing(description);
CREATE INDEX IF NOT EXISTS idx_person_description ON person(description);
CREATE INDEX IF NOT EXISTS idx_organization_description ON organization(description);
