DROP INDEX IF EXISTS base_sepolia_backend.idx_thing_description;
CREATE INDEX idx_thing_description_hash ON base_sepolia_backend.thing USING hash (description);
