create extension if not exists ai cascade;

SELECT ai.create_vectorizer(
   'atom'::regclass,
   destination => 'atom_label_embeddings',
   embedding => ai.embedding_openai('text-embedding-3-small', 768),
   chunking => ai.chunking_recursive_character_text_splitter('label')
);