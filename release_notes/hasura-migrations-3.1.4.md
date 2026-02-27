# hasura-migrations:3.1.4

**Date:** 2026-02-27

## Fix: Undercounted vault position_count (follow-up to 3.1.2)

### Problem

The 3.1.2 migration fixed 78 vaults with negative `position_count`, but the same race condition also caused ~2028 additional vaults to be undercounted by 1 without going negative. These are cases where the position was created before the vault (lost increment) but was never redeemed, so `position_count` stayed at 0 instead of the correct 1.

This means Portal was showing 0 supporters for triples that actually had 1 active supporter.

See `hasura-migrations-3.1.2.md` for the full root cause analysis.

### Fix

Batched recalculation of `vault.position_count` from actual positions for all remaining mismatched vaults. Uses `SET LOCAL statement_timeout = '30min'` (scoped to the migration transaction only) to accommodate the cascade trigger overhead across environments with varying dataset sizes. Cascade triggers remain enabled throughout, so live position changes are never missed.

### Migration

`1771526407000_fix_vault_position_count_undercount`

### Changes

- `infrastructure/hasura/migrations/intuition/1771526407000_fix_vault_position_count_undercount/up.sql`
  - `SET LOCAL statement_timeout = '30min'` — transaction-scoped timeout increase, cannot affect other sessions
  - `DO` block that loops in batches of 100, finding vaults where `position_count` differs from the actual count of positions with `shares > 0`, and corrects them. Cascade triggers (`update_triple_vault_from_vault` -> `triple_term` -> `predicate_object` / `subject_predicate`) fire normally for each batch.
- `infrastructure/hasura/migrations/intuition/1771526407000_fix_vault_position_count_undercount/down.sql`
  - No-op (data-only migration)

### Impact

- **Scope:** Same tables as 3.1.2 — `vault`, `triple_term`, `predicate_object`, `subject_predicate`. No Rust code changes.
- **Safety:** Data-only migration. Cascade triggers stay enabled — live position changes are processed normally. The timeout increase is `SET LOCAL` (transaction-scoped, not session or global).
- **Expected result:** All remaining undercounted vaults corrected, with cascade triggers propagating the fix to all aggregate tables. Works across environments regardless of the number of affected rows.
