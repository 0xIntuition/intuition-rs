-- Fix Caip10

ALTER TABLE atom_value ADD COLUMN caip10_id NUMERIC(78, 0) REFERENCES caip10(id);

-- Add JsonObject

ALTER TYPE atom_type ADD VALUE 'JsonObject';

CREATE TABLE json_object (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  data JSONB NOT NULL
);

ALTER TABLE atom_value ADD COLUMN json_object_id NUMERIC(78, 0) REFERENCES json_object(id);

-- Add TextObject

ALTER TYPE atom_type ADD VALUE 'TextObject';

CREATE TABLE text_object (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  data TEXT NOT NULL
);

ALTER TABLE atom_value ADD COLUMN text_object_id NUMERIC(78, 0) REFERENCES text_object(id);

-- Add ByteObject

ALTER TYPE atom_type ADD VALUE 'ByteObject';

CREATE TABLE byte_object (
  id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
  data BYTEA NOT NULL
);

ALTER TABLE atom_value ADD COLUMN byte_object_id NUMERIC(78, 0) REFERENCES byte_object(id);
