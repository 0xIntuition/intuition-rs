# hasura-migrations:3.1.3

**Date:** 2026-02-27

## Fix: Undercounted vault position_count (follow-up to 3.1.2)

### Problem

The 3.1.2 migration fixed 78 vaults with negative `position_count`, but the same race condition also caused ~2028 additional vaults to be undercounted by 1 without going negative. These are cases where the position was created before the vault (lost increment) but was never redeemed, so `position_count` stayed at 0 instead of the correct 1.

This means Portal was showing 0 supporters for triples that actually had 1 active supporter.

See `hasura-migrations-3.1.2.md` for the full root cause analysis.

### Fix

Batched data backfill that recalculates `vault.position_count` from actual positions for all remaining mismatched vaults. Runs in batches of 200 to stay within statement timeout limits, since each vault UPDATE cascades through `update_triple_vault_from_vault` triggers to `triple_term`, `predicate_object`, and `subject_predicate`.

### Migration

`1771526407000_fix_vault_position_count_undercount`

### Changes

- `infrastructure/hasura/migrations/intuition/1771526407000_fix_vault_position_count_undercount/up.sql`
  - `DO` block that loops in batches of 200, finding vaults where `position_count` differs from the actual count of positions with `shares > 0`, and corrects them
- `infrastructure/hasura/migrations/intuition/1771526407000_fix_vault_position_count_undercount/down.sql`
  - No-op (data-only migration)

### Impact

- **Scope:** Same tables as 3.1.2 — `vault`, `triple_term`, `predicate_object`, `subject_predicate`. No Rust code changes.
- **Safety:** Data-only migration. Each batch of 200 updates commits independently. Cascade triggers update aggregate tables automatically.
- **Expected result:** ~2028 additional vaults corrected from `position_count = 0` to `position_count = 1`, with cascading fixes to `supporter_count` in aggregate tables.
