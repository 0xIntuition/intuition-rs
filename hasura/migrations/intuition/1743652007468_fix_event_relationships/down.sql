-- Drop the new foreign key constraints
DO $$ 
BEGIN
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'event_atom_id_fkey') THEN
        ALTER TABLE event DROP CONSTRAINT event_atom_id_fkey;
    END IF;
    
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'event_triple_id_fkey') THEN
        ALTER TABLE event DROP CONSTRAINT event_triple_id_fkey;
    END IF;
END $$; 