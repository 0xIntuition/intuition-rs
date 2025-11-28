# Position Count Accuracy Analysis

## Tables with Position Counts

1. **`stats`** - `total_positions` (INTEGER)
2. **`stats_hour`** - `total_positions` (INTEGER) 
3. **`vault`** - `position_count` (INTEGER NOT NULL)
4. **`triple_vault`** - `position_count` (BIGINT NOT NULL)
5. **`triple_term`** - `total_position_count` (BIGINT NOT NULL)
6. **`predicate_object`** - `total_position_count` (INTEGER NOT NULL DEFAULT 0)
7. **`subject_predicate`** - `total_position_count` (INTEGER NOT NULL DEFAULT 0)
8. **`position`** - The main position table (positions exist when `shares > 0`)

## Verification Results

### Summary Statistics
- **Active positions** (shares > 0): 3,111,102
- **Closed positions** (shares = 0): 709
- **Total positions**: 3,111,811
- **stats.total_positions**: 3,111,816 (id=0) - **Difference: +5**
- **vault.total_position_count** (sum): 3,110,650 - **Difference: -452**

### Issues Found

#### 1. `vault.position_count` ❌
- **Mismatches**: 452 vaults
- **Total difference**: -452 positions
- **Pattern**: Most are off by 1, many show 0 but have 1 actual position
- **Example**: 
  - `0x011c3a72fbf1aafcb9b1ed8f18721ae4181b74ff9635e235dfeac9aa71989f0b` curve_id=2: stored=515, actual=516

#### 2. `stats.total_positions` ⚠️
- **Stored**: 3,111,816
- **Actual active positions**: 3,111,102
- **Difference**: +714 (includes closed positions?)
- **Note**: Stats table counts all positions, not just active ones

#### 3. `triple_term.total_position_count` ❌
- **Mismatches**: 244 triple_terms
- **Total difference**: 7,495 positions
- **Pattern**: Mostly off by 1-5 positions
- **Example**:
  - `0x27e44b49dcee41230a4def6f1726add3338f3f324191387f2a35f0af3b8993b1`: stored=2341, actual=2336 (diff=-5)

#### 4. `triple_vault.position_count` ❌
- **Mismatches**: 597 triple_vaults
- **Total difference**: 7,844 positions
- **Pattern**: Similar to vault - many off by 1, some by 3+
- **Example**:
  - `0x007fd6d2e6371b70f02709e6b0c013290ac3255da30fd01b245364c2754c303a` curve_id=1: stored=2797, actual=2794 (diff=-3)

#### 5. `predicate_object.total_position_count` ❌
- **Mismatches**: 7 predicate_object pairs
- **Total difference**: 7 positions (all off by 1)
- **Pattern**: All are off by exactly 1

#### 6. `subject_predicate.total_position_count` ❌
- **Mismatches**: 24 subject_predicate pairs
- **Total difference**: 24 positions (all off by 1)
- **Pattern**: All are off by exactly 1

## Root Cause Analysis

The triggers that maintain these counts are:
- `increment_vault_position_count()` - fires on INSERT when shares > 0
- `reopen_vault_position_count()` - fires on UPDATE when shares go 0 → > 0
- `decrement_vault_position_count()` - fires on UPDATE when shares go > 0 → 0

**Potential issues:**
1. Race conditions in concurrent inserts/updates
2. Missing triggers for initial data load
3. Triggers not firing in certain edge cases
4. Manual updates bypassing triggers
5. Transaction rollbacks not reflected in counts

## Recommendations

1. **Fix `vault.position_count`**: Recalculate from actual positions
2. **Fix `triple_term.total_position_count`**: Recalculate from vault sums
3. **Fix `triple_vault.position_count`**: Recalculate from actual positions
4. **Fix `predicate_object` and `subject_predicate`**: Recalculate from triple_term
5. **Review trigger logic** for edge cases and race conditions
6. **Add constraints or checks** to prevent future drift

