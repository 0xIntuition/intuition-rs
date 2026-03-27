-- Backfill missing term rows for atoms and triples.
--
-- Root cause: Commit 4aefeb0 (March 10) added Vault::ensure_exists() to the
-- Deposited event handler, which pre-creates vault rows. When SharePriceChanged
-- later arrives and finds the vault already exists, it skips get_or_create_vault()
-- — the ONLY code path that creates Term rows. Result: 715 atoms/triples have
-- vault, position, signal, and event rows but no term row.
--
-- Fix: Reconstruct term rows from atom/triple + vault data.
-- Idempotent: WHERE NOT EXISTS guard — safe to run on any environment.

-- Step 1: Backfill missing atom terms
INSERT INTO term (id, type, atom_id, triple_id, total_assets, total_market_cap, created_at, updated_at)
SELECT
    a.term_id                AS id,
    'Atom'                   AS type,
    a.term_id                AS atom_id,
    NULL                     AS triple_id,
    COALESCE(vs.total_assets, 0) AS total_assets,
    COALESCE(vs.total_assets, 0) AS total_market_cap,
    a.created_at             AS created_at,
    NOW()                    AS updated_at
FROM atom a
LEFT JOIN LATERAL (
    SELECT SUM(v.total_assets) AS total_assets
    FROM vault v
    WHERE v.term_id = a.term_id
) vs ON true
WHERE NOT EXISTS (SELECT 1 FROM term t WHERE t.id = a.term_id);

-- Step 2: Backfill missing triple terms
INSERT INTO term (id, type, atom_id, triple_id, total_assets, total_market_cap, created_at, updated_at)
SELECT
    tr.term_id               AS id,
    'Triple'                 AS type,
    NULL                     AS atom_id,
    tr.term_id               AS triple_id,
    COALESCE(vs.total_assets, 0) AS total_assets,
    COALESCE(vs.total_assets, 0) AS total_market_cap,
    tr.created_at            AS created_at,
    NOW()                    AS updated_at
FROM triple tr
LEFT JOIN LATERAL (
    SELECT SUM(v.total_assets) AS total_assets
    FROM vault v
    WHERE v.term_id = tr.term_id
) vs ON true
WHERE NOT EXISTS (SELECT 1 FROM term t WHERE t.id = tr.term_id);
