DROP INDEX IF EXISTS idx_thing_description;
CREATE INDEX idx_thing_description_hash ON thing USING hash (description);