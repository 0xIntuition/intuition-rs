-- Drop existing foreign key constraints if they exist
DO $$ 
BEGIN
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'event_atom_id_fkey') THEN
        ALTER TABLE event DROP CONSTRAINT event_atom_id_fkey;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'event_triple_id_fkey') THEN
        ALTER TABLE event DROP CONSTRAINT event_triple_id_fkey;
    END IF;
END $$;

-- Add new foreign key constraints referencing term table
ALTER TABLE event ADD CONSTRAINT event_atom_id_fkey 
    FOREIGN KEY (atom_id) REFERENCES term(id);

ALTER TABLE event ADD CONSTRAINT event_triple_id_fkey 
    FOREIGN KEY (triple_id) REFERENCES term(id); 