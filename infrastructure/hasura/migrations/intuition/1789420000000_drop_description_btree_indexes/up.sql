-- Plain btree indexes on free-text `description` columns hit PostgreSQL's btree tuple
-- size limit (2704 bytes on 8 KB pages) for long descriptions. When that happens the
-- resolver's `thing` upsert fails with
--   "index row size 3416 exceeds btree version 4 maximum 2704 for index idx_thing_description"
-- and the atom stays `Pending` / `Unknown` forever (6 atoms on mainnet as of 2026-09-14).
--
-- These indexes cannot serve the `_ilike '%…%'` searches the API runs; full-text search
-- goes through `term_text` (`idx_term_text_*_fts`, migration 1768474894667).
DROP INDEX IF EXISTS idx_thing_description;
DROP INDEX IF EXISTS idx_person_description;
DROP INDEX IF EXISTS idx_organization_description;
