-- Update references from atom.id to atom.term_id
-- This migration updates all foreign key references to atom.id to point to atom.term_id instead

-- First, drop existing foreign key constraints
ALTER TABLE account DROP CONSTRAINT IF EXISTS fk_account_atom;
ALTER TABLE triple DROP CONSTRAINT IF EXISTS triple_subject_id_fkey;
ALTER TABLE triple DROP CONSTRAINT IF EXISTS triple_predicate_id_fkey;
ALTER TABLE triple DROP CONSTRAINT IF EXISTS triple_object_id_fkey;
ALTER TABLE predicate_object DROP CONSTRAINT IF EXISTS predicate_object_predicate_id_fkey;
ALTER TABLE predicate_object DROP CONSTRAINT IF EXISTS predicate_object_object_id_fkey;
ALTER TABLE atom_value DROP CONSTRAINT IF EXISTS atom_value_atom_id_fkey;
ALTER TABLE atom_value DROP CONSTRAINT IF EXISTS atom_value_thing_id_fkey;
ALTER TABLE atom_value DROP CONSTRAINT IF EXISTS atom_value_person_id_fkey;
ALTER TABLE atom_value DROP CONSTRAINT IF EXISTS atom_value_organization_id_fkey;
ALTER TABLE atom_value DROP CONSTRAINT IF EXISTS atom_value_book_id_fkey;

-- Add new columns to vault table if they don't exist
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'vault' AND column_name = 'total_assets') THEN
        ALTER TABLE vault ADD COLUMN total_assets NUMERIC(78, 0);
    END IF;
    
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'vault' AND column_name = 'market_cap') THEN
        ALTER TABLE vault ADD COLUMN market_cap NUMERIC(78, 0);
    END IF;
END $$;

-- Add new foreign key constraints referencing atom.term_id
ALTER TABLE account ADD CONSTRAINT fk_account_atom 
    FOREIGN KEY (atom_id) REFERENCES atom(term_id);

ALTER TABLE triple ADD CONSTRAINT triple_subject_id_fkey 
    FOREIGN KEY (subject_id) REFERENCES atom(term_id);

ALTER TABLE triple ADD CONSTRAINT triple_predicate_id_fkey 
    FOREIGN KEY (predicate_id) REFERENCES atom(term_id);

ALTER TABLE triple ADD CONSTRAINT triple_object_id_fkey 
    FOREIGN KEY (object_id) REFERENCES atom(term_id);

ALTER TABLE predicate_object ADD CONSTRAINT predicate_object_predicate_id_fkey 
    FOREIGN KEY (predicate_id) REFERENCES atom(term_id);

ALTER TABLE predicate_object ADD CONSTRAINT predicate_object_object_id_fkey 
    FOREIGN KEY (object_id) REFERENCES atom(term_id);

ALTER TABLE atom_value ADD CONSTRAINT atom_value_atom_id_fkey 
    FOREIGN KEY (id) REFERENCES atom(term_id);

ALTER TABLE atom_value ADD CONSTRAINT atom_value_thing_id_fkey 
    FOREIGN KEY (thing_id) REFERENCES thing(id);

ALTER TABLE atom_value ADD CONSTRAINT atom_value_person_id_fkey 
    FOREIGN KEY (person_id) REFERENCES person(id);

ALTER TABLE atom_value ADD CONSTRAINT atom_value_organization_id_fkey 
    FOREIGN KEY (organization_id) REFERENCES organization(id);

ALTER TABLE atom_value ADD CONSTRAINT atom_value_book_id_fkey 
    FOREIGN KEY (book_id) REFERENCES book(id);

-- Update indexes to reference term_id instead of id
DROP INDEX IF EXISTS idx_atom_value_atom;
CREATE INDEX idx_atom_value_atom ON atom_value(id);

DROP INDEX IF EXISTS idx_atom_value_thing;
CREATE INDEX idx_atom_value_thing ON atom_value(thing_id);

DROP INDEX IF EXISTS idx_atom_value_person;
CREATE INDEX idx_atom_value_person ON atom_value(person_id);

DROP INDEX IF EXISTS idx_atom_value_organization;
CREATE INDEX idx_atom_value_organization ON atom_value(organization_id);

DROP INDEX IF EXISTS idx_atom_value_book;
CREATE INDEX idx_atom_value_book ON atom_value(book_id); 