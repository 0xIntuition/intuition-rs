-- Test script for verifying search_term with similarity scores (US-009)
-- This demonstrates that similarity scores can be accessed by querying term_embeddings directly

\echo '========================================='
\echo 'US-009: Verify Similarity Scores'
\echo '========================================='
\echo ''

\echo 'Test 1: Search for "person" with similarity scores'
\echo '---------------------------------------------------'
SELECT
    t.id,
    t.type,
    tt.title,
    (te.embedding <=> ai.openai_embed('text-embedding-3-small', 'person', dimensions=>768)) as similarity_score
FROM term_embeddings te
JOIN term t ON te.id = t.id
JOIN term_text tt ON t.id = tt.id
ORDER BY similarity_score ASC
LIMIT 5;

\echo ''
\echo 'Test 2: Search for "Alice" with similarity scores'
\echo '--------------------------------------------------'
SELECT
    t.id,
    t.type,
    tt.title,
    (te.embedding <=> ai.openai_embed('text-embedding-3-small', 'Alice', dimensions=>768)) as similarity_score
FROM term_embeddings te
JOIN term t ON te.id = t.id
JOIN term_text tt ON t.id = tt.id
ORDER BY similarity_score ASC
LIMIT 5;

\echo ''
\echo 'Test 3: Verify results are ranked by similarity (ascending score = more similar)'
\echo '--------------------------------------------------------------------------------'
WITH ranked_results AS (
    SELECT
        t.id,
        t.type,
        tt.title,
        (te.embedding <=> ai.openai_embed('text-embedding-3-small', 'person Alice', dimensions=>768)) as similarity_score,
        ROW_NUMBER() OVER (ORDER BY (te.embedding <=> ai.openai_embed('text-embedding-3-small', 'person Alice', dimensions=>768))) as rank
    FROM term_embeddings te
    JOIN term t ON te.id = t.id
    JOIN term_text tt ON t.id = tt.id
)
SELECT rank, type, title, similarity_score
FROM ranked_results
ORDER BY rank;

\echo ''
\echo 'Test 4: Verify that search_term function returns results in the correct order'
\echo '-----------------------------------------------------------------------------'
\echo 'Note: search_term orders by distance (lower = more similar)'
SELECT
    ROW_NUMBER() OVER () as rank,
    s.type,
    tt.title
FROM search_term('person') s
JOIN term_text tt ON s.id = tt.id
LIMIT 5;

\echo ''
\echo 'Test 5: Compare direct similarity query vs search_term function'
\echo '----------------------------------------------------------------'
\echo 'Both should return results in the same order (most similar first)'
\echo ''
\echo 'Direct query with similarity:'
SELECT
    t.type,
    LEFT(tt.title, 40) as title,
    ROUND((te.embedding <=> ai.openai_embed('text-embedding-3-small', 'person', dimensions=>768))::numeric, 4) as score
FROM term_embeddings te
JOIN term t ON te.id = t.id
JOIN term_text tt ON t.id = tt.id
ORDER BY score ASC;

\echo ''
\echo 'search_term function (implicit ordering):'
SELECT
    s.type,
    LEFT(tt.title, 40) as title
FROM search_term('person') s
JOIN term_text tt ON s.id = tt.id;

\echo ''
\echo '========================================='
\echo 'Summary'
\echo '========================================='
\echo 'Similarity scores are accessible by:'
\echo '1. Joining term_embeddings with term table'
\echo '2. Using the <=> operator (cosine distance) with ai.openai_embed()'
\echo '3. Lower scores indicate higher similarity'
\echo ''
\echo 'The search_term function returns results ordered by similarity,'
\echo 'but does not expose the similarity score in its return type.'
\echo ''
