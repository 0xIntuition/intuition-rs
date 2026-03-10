# hasura-migrations-3.2.1

**Date:** 2026-03-09

### Migration `1771526421000_definitive_position_count_fix`

**Problem:** The consumer processes up to 10 events concurrently. When a `Deposited` event creates a position before `SharePriceChanged` creates the vault, the old `increment_vault_position_count()` trigger fires an UPDATE against a non-existent vault row — the increment is permanently lost. The BEFORE INSERT trigger on vault can't see uncommitted positions due to MVCC. This caused recurring undercounts that reappeared after every manual fix.

**Fix — two-part strategy:**

1. **Replaced 4 incremental triggers with a single full-recalculate trigger** (`sync_vault_position_count()`). On every position INSERT/UPDATE OF shares/DELETE, it does `COUNT(*)` from the position table rather than `+= 1` / `-= 1`. If the vault doesn't exist yet, the UPDATE is a no-op (the BEFORE INSERT trigger on vault handles that case).

2. **Runs `fix_wrong_position_counts()`** to correct any existing mismatches.

**Triggers replaced:**
- `increment_vault_position_count()` → dropped
- `reopen_vault_position_count()` → dropped
- `decrement_vault_position_count()` → dropped
- `delete_vault_position_count()` → dropped
- `sync_vault_position_count()` → new (covers all 4 cases)

**Cascade chain unchanged:** vault UPDATE → `triple_vault` → `triple_term` → `predicate_object` / `subject_predicate` continues to work as-is.

**Paired with:** `consumer:3.0.64` which pre-creates a vault stub before inserting positions, ensuring the vault row always exists when the trigger fires.
