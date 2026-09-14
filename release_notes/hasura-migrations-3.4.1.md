# hasura-migrations-3.4.1

**Date:** 2026-09-14

**Image:** `ghcr.io/0xintuition/hasura-migrations:3.4.1`
**Previous:** `3.4.0` (built 2026-09-10)

## Fix: btree indexes on `description` columns block atoms with long descriptions

### Problem

`idx_thing_description`, `idx_person_description` and `idx_organization_description` are plain
btree indexes on free-text columns. PostgreSQL btree tuples are capped at 2704 bytes, so a
`thing` with a long, poorly compressible description makes the resolver's upsert fail:

```
Failed to upsert data for thing: error returned from database: index row size 3416 exceeds
btree version 4 maximum 2704 for index "idx_thing_description"
```

The atom then stays `Pending` / `Unknown` with its JSON already stored in `json_object`
(6 atoms on mainnet-nested-triples as of 2026-09-14; found while re-resolving the atoms from
`docs/atom-resolver-race-investigation-2026-09-14.md`).

### Fix

New migration `1789420000000_drop_description_btree_indexes` drops the three indexes.
A btree cannot serve the `_ilike '%…%'` searches the API runs on these columns; full-text
search already goes through `term_text` (`idx_term_text_title_fts` / `idx_term_text_description_fts`).

`pg_stat_user_indexes` before the change (scans since the last pod restart, ~5 days):
mainnet-nested-triples 139 / 462 / 2430 (thing / person / organization),
mainnet-next 3 / 0 / 12. If a query is found that relied on them, replace with an expression
index that is bounded in size (e.g. `left(description, 1000)`) rather than the raw column.

### Deployment notes

- Runs in seconds (`thing` has ~145k rows); no table rewrite.
- After deploy, re-enqueue the atoms that were blocked by the index (they are `Failed` or
  `Pending` with an `ipfs://` data and an existing `json_object` row):

  ```sql
  select term_id from atom a
  where a.resolving_status in ('Pending','Failed') and a.type='Unknown'
    and exists (select 1 from json_object j where j.id = a.term_id);
  ```
  then `XADD resolver_stream * body '{"message":{"Atom":"<term_id>"}}'` on the environment's
  Redis (see `docs/atom-resolver-race-2026-09-14/export-stuck-atom-ids.sql`). Once
  consumer 3.0.70 is deployed, the resolver backfill sweep does this by itself for atoms
  younger than 24 h.
