# hasura-migrations:3.1.2

**Date:** 2026-02-27

## Fix: Negative supporter/opposer counts on triples

### Problem

Portal displays negative supporter counts (-1, and -2 in aggregate views) when sorting claims by fewest supporters on `/explore/triples`. The negative values appear in both list views and triple detail pages.

**Affected tables:** `vault.position_count`, `triple_term.supporter_count`, `predicate_object.supporter_count`, `subject_predicate.supporter_count`

**Scale:** 78 vaults with `position_count = -1`, cascading to negative `supporter_count` in `triple_term`, `predicate_object` (min: -1), and `subject_predicate` (min: -2, from two affected triples summing).

**Root cause:** Event ordering race condition between `Deposited` and `SharePriceChanged` events in the decoded consumer.

For a deposit, the blockchain emits events in this order within the same block:

1. `Deposited` (log_index N)
2. `SharePriceChanged` (log_index N+1)

The consumer processes them sequentially:

1. **Deposited handler** creates a position with `shares > 0`. The `AFTER INSERT` trigger on `position` fires and tries to increment `vault.position_count`, but the vault row doesn't exist yet (it's created by the next event). The trigger retries 5 times, fails, logs a WARNING, and the **increment is lost**.

2. **SharePriceChanged handler** creates the vault with `position_count = 0` (doesn't know about the lost increment).

3. Later, when the position is **redeemed** (shares -> 0), the `AFTER UPDATE` trigger fires and successfully decrements `vault.position_count` from 0 to -1.

The negative `vault.position_count` then propagates through existing triggers: `vault` -> `triple_term.supporter_count` / `opposer_count` -> `predicate_object` / `subject_predicate` aggregates.

### Fix

#### 1. BEFORE INSERT trigger on vault (prevents future occurrences)

New function `recalculate_vault_position_count_on_insert()` fires before a vault row is written. It overrides the incoming `position_count` with the actual count of existing positions that have `shares > 0` for that `(term_id, curve_id)` pair.

This compensates for any lost increments from positions that were created before their vault. For `ON CONFLICT DO UPDATE` cases (vault already exists), the trigger fires but its changes are discarded since the UPDATE path is taken — which is correct since the vault already has a valid `position_count`.

#### 2. Data backfill (fixes existing negative values)

Updates all 78 vaults with `position_count < 0` to the correct value calculated from actual positions. The existing `AFTER UPDATE` trigger on `vault` (`update_triple_vault_from_vault`) automatically cascades the fix to `triple_term`, `predicate_object`, and `subject_predicate`.

### Migration

`1771526406000_fix_vault_position_count_on_insert`

### Changes

- `infrastructure/hasura/migrations/intuition/1771526406000_fix_vault_position_count_on_insert/up.sql`
  - `recalculate_vault_position_count_on_insert()` — BEFORE INSERT trigger function on `vault` that calculates `position_count` from existing positions
  - `vault_recalculate_position_count_on_insert` — trigger binding
  - Data fix UPDATE for all vaults with `position_count < 0`

- `infrastructure/hasura/migrations/intuition/1771526406000_fix_vault_position_count_on_insert/down.sql`
  - Drops the trigger and function

### Impact

- **Scope:** `vault`, `triple_term`, `predicate_object`, `subject_predicate` tables. No Rust code changes.
- **Safety:** The BEFORE INSERT trigger only runs on vault creation (not updates). The COUNT query is scoped to a specific `(term_id, curve_id)` pair, so performance impact is negligible. The data fix UPDATE targets only the 78 affected rows and cascades via existing triggers.
- **Expected result:** All negative `supporter_count` / `opposer_count` / `position_count` values corrected to 0. Future vault creations will initialize with the correct count even when positions are created before the vault.
