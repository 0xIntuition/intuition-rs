# hasura-migrations:3.1.2

**Date:** 2026-02-27

## Fix: Negative supporter/opposer counts on triples

### Problem

Portal displays negative supporter counts (-1, and -2 in aggregate views) when sorting claims by fewest supporters on `/explore/triples`. The negative values appear in both list views and triple detail pages.

**Affected tables:** `vault.position_count`, `triple_term.supporter_count`, `predicate_object.supporter_count`, `subject_predicate.supporter_count`

**Scale:** 2106 vaults affected total — 78 with `position_count = -1` (where the position was subsequently redeemed, pushing the count negative) and 2028 undercounted by 1 (where the position was never redeemed, so the count stayed at 0 instead of the correct 1). The negative values cascaded to `supporter_count` in `triple_term`, `predicate_object` (min: -1), and `subject_predicate` (min: -2, from two affected triples summing).

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

#### 2. Data backfill (fixes all existing mismatched values)

Recalculates `position_count` for all 2106 vaults where the stored count doesn't match the actual number of positions with `shares > 0`. Runs in batches of 200 to avoid statement timeouts from cascade triggers. The existing `AFTER UPDATE` trigger on `vault` (`update_triple_vault_from_vault`) automatically cascades the fix to `triple_term`, `predicate_object`, and `subject_predicate`.

### Migrations

#### `1771526406000_fix_vault_position_count_on_insert`

- `recalculate_vault_position_count_on_insert()` — BEFORE INSERT trigger function on `vault` that calculates `position_count` from existing positions
- `vault_recalculate_position_count_on_insert` — trigger binding
- Data fix for the 78 vaults with `position_count < 0`

#### `1771526407000_fix_vault_position_count_undercount`

- Batched data fix (200 rows per batch) for all remaining ~2028 vaults where `position_count` is undercounted but not negative
- Runs in a `DO` block loop to stay within statement timeout limits, since each vault UPDATE cascades through `update_triple_vault_from_vault` triggers

### Impact

- **Scope:** `vault`, `triple_term`, `predicate_object`, `subject_predicate` tables. No Rust code changes.
- **Safety:** The BEFORE INSERT trigger only runs on vault creation (not updates). The COUNT query is scoped to a specific `(term_id, curve_id)` pair, so performance impact is negligible. The data backfill runs in batches of 200 to stay within statement timeout limits.
- **Expected result:** All 2106 mismatched `position_count` values corrected, with cascading fixes to `supporter_count` / `opposer_count` in `triple_term`, `predicate_object`, and `subject_predicate`. Future vault creations will initialize with the correct count even when positions are created before the vault.
