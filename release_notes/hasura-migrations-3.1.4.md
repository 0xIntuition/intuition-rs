# hasura-migrations:3.1.4

**Date:** 2026-02-27

## Fix: Undercounted vault position_count (follow-up to 3.1.2)

### Problem

The 3.1.2 migration fixed 78 vaults with negative `position_count`, but the same race condition also caused ~2028 additional vaults to be undercounted by 1 without going negative. These are cases where the position was created before the vault (lost increment) but was never redeemed, so `position_count` stayed at 0 instead of the correct 1.

This means Portal was showing 0 supporters for triples that actually had 1 active supporter.

See `hasura-migrations-3.1.2.md` for the full root cause analysis.

### Fix (3.1.4 — replaces 3.1.3 approach)

The 3.1.3 attempt used `SET LOCAL statement_timeout = '30min'` as a separate SQL statement before the `DO` block. This failed on mainnet-next because the Hasura Dockerfile applies migrations with `--no-transaction`, which sends each SQL statement as a separate `run_sql` API call through the connection pool. The `SET LOCAL` ran on one connection, the `DO` block on another — the timeout override was lost.

**3.1.4 fix:** Wraps the batched loop in a temporary PL/pgSQL function with a function-level `SET statement_timeout = '30min'` clause. PostgreSQL applies function-level GUC overrides when entering the function and reschedules the timeout timer, so this works regardless of `--no-transaction` and connection pooling. The function is created, called, and dropped within the same migration.

Cascade triggers remain enabled throughout, so live position changes are never missed.

### Migration

`1771526407000_fix_vault_position_count_undercount`

### Changes

- `infrastructure/hasura/migrations/intuition/1771526407000_fix_vault_position_count_undercount/up.sql`
  - `CREATE OR REPLACE FUNCTION _fix_vault_position_count_undercount()` with `SET statement_timeout = '30min'` — function-level GUC override that reschedules the timeout timer on entry, unaffected by `--no-transaction` or connection pooling
  - Function body loops in batches of 100, finding vaults where `position_count` differs from the actual count of positions with `shares > 0`, and corrects them. Cascade triggers (`update_triple_vault_from_vault` -> `triple_term` -> `predicate_object` / `subject_predicate`) fire normally for each batch.
  - `SELECT _fix_vault_position_count_undercount()` — executes the fix
  - `DROP FUNCTION _fix_vault_position_count_undercount()` — cleanup
- `infrastructure/hasura/migrations/intuition/1771526407000_fix_vault_position_count_undercount/down.sql`
  - No-op (data-only migration)

### Impact

- **Scope:** Same tables as 3.1.2 — `vault`, `triple_term`, `predicate_object`, `subject_predicate`. No Rust code changes.
- **Safety:** Data-only migration. Cascade triggers stay enabled — live position changes are processed normally. The 30-minute timeout is scoped to the function call only and reverts when the function returns. The function is dropped after use.
- **Expected result:** All remaining undercounted vaults corrected, with cascade triggers propagating the fix to all aggregate tables. Works across environments regardless of the number of affected rows or `statement_timeout` configuration.
