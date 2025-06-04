ALTER TABLE thing ADD CONSTRAINT thing_term_fkey 
    FOREIGN KEY (id) REFERENCES term(id);

CREATE TABLE term_text (
    id NUMERIC(78, 0) PRIMARY KEY NOT NULL,
    title TEXT,
    description TEXT
);

create extension if not exists ai cascade;
-- CREATE EXTENSION IF NOT EXISTS pgai CASCADE;

SELECT ai.create_vectorizer(
    'term_text'::regclass,
    destination => ai.destination_table('term_embeddings'),
    embedding => ai.embedding_openai('text-embedding-3-small', 768),
    loading => ai.loading_column('description'),
    formatting => ai.formatting_python_template('title: $title id: $id $chunk')
);


CREATE FUNCTION search_term (query text) RETURNS SETOF term LANGUAGE sql STABLE AS $$
    SELECT id, type, atom_id, triple_id, total_assets, total_market_cap FROM (
        SELECT 
            t.*,
            embedding <=> ai.openai_embed('text-embedding-3-small', query, dimensions=>768) as distance
        FROM term_embeddings
        LEFT JOIN term t ON term_embeddings.id = t.id
        ORDER BY distance
    ) s
$$;


-- Create or replace the trigger function
CREATE OR REPLACE FUNCTION update_term_text_function()
RETURNS TRIGGER AS $$
BEGIN
    -- For insert operations
    IF TG_OP = 'INSERT' THEN
        INSERT INTO term_text (id, title, description)
        VALUES (NEW.id, NEW.name, NEW.description);
    
    -- For update operations
    ELSIF TG_OP = 'UPDATE' THEN
        -- Only update if name or description changed
        IF NEW.name <> OLD.name OR NEW.description <> OLD.description OR 
           (OLD.name IS NULL AND NEW.name IS NOT NULL) OR 
           (OLD.description IS NULL AND NEW.description IS NOT NULL) THEN
            
            UPDATE term_text 
            SET title = NEW.name, 
                description = NEW.description
            WHERE id = NEW.id;
            
            -- If no row was updated, insert one
            IF NOT FOUND THEN
                INSERT INTO term_text (id, title, description)
                VALUES (NEW.id, NEW.name, NEW.description);
            END IF;
        END IF;
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create triggers for the thing table
CREATE TRIGGER thing_insert_update_term_text_trigger
AFTER INSERT OR UPDATE ON thing
FOR EACH ROW
EXECUTE FUNCTION update_term_text_function();

-- Create triggers for the person table
CREATE TRIGGER person_insert_update_term_text_trigger
AFTER INSERT OR UPDATE ON person
FOR EACH ROW
EXECUTE FUNCTION update_term_text_function();

-- Create triggers for the book table
CREATE TRIGGER book_insert_update_term_text_trigger
AFTER INSERT OR UPDATE ON book
FOR EACH ROW
EXECUTE FUNCTION update_term_text_function();

-- Create triggers for the organization table
CREATE TRIGGER organization_insert_update_term_text_trigger
AFTER INSERT OR UPDATE ON organization
FOR EACH ROW
EXECUTE FUNCTION update_term_text_function(); 