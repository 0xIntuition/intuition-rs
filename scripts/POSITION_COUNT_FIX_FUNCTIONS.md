# Position Count Fix Functions

Three PostgreSQL functions are available to fix position counts in the database. A position is considered active when `shares > 0`.

## Functions Overview

1. **`fix_all_position_counts()`** - Recalculates ALL position counts across the entire database
2. **`fix_wrong_position_counts()`** - Only fixes position counts that are incorrect (more efficient)
3. **`fix_position_counts_for_term(p_term_id TEXT)`** - Fixes position counts for a specific term_id

## Prerequisites

Ensure the migration has been run:
```bash
psql -h localhost -p 5435 -U <username> -d storage \
  -f infrastructure/hasura/migrations/intuition/1729947331638_fix_position_count_functions/up.sql
```

**Note:** Replace `<username>` with your database username. You may need to set `PGPASSWORD` environment variable or use a `.pgpass` file.

## Usage

### 1. Fix All Position Counts

**When to use:** Full database recalculation, maintenance windows, or after data migration.

**Command:**
```bash
PGPASSWORD='<password>' psql -h localhost -p 5435 -U <username> -d storage \
  -c "SELECT * FROM fix_all_position_counts();"
```

**Output:**
```
 vault_updated | triple_vault_updated | triple_term_updated | predicate_object_updated | subject_predicate_updated | stats_updated 
---------------+----------------------+---------------------+--------------------------+---------------------------+---------------
        288276 |                61337 |                60461 |                      7039 |                     49346 |             1
```

**What it does:**
- Updates `vault.position_count` for all vaults
- Updates `triple_vault.position_count` for all triple_vaults
- Updates `triple_term.total_position_count` for all triple_terms
- Updates `predicate_object.total_position_count` for all predicate_object pairs
- Updates `subject_predicate.total_position_count` for all subject_predicate pairs
- Updates `stats.total_positions` (id=0)

**Note:** This can take a while on large databases and may cause deadlocks if run during high traffic.

---

### 2. Fix Only Wrong Position Counts

**When to use:** Regular maintenance, fixing drift without full recalculation.

**Command:**
```bash
PGPASSWORD='<password>' psql -h localhost -p 5435 -U <username> -d storage \
  -c "SELECT * FROM fix_wrong_position_counts();"
```

**Output:**
```
 vault_updated | triple_vault_updated | triple_term_updated | predicate_object_updated | subject_predicate_updated | stats_updated 
---------------+----------------------+---------------------+--------------------------+---------------------------+---------------
           452 |                  351 |                 244 |                         7 |                        24 |             1
```

**What it does:**
- Only updates rows where the position count is incorrect
- More efficient than `fix_all_position_counts()` as it skips correct rows
- Same updates as function #1, but only for mismatched rows

**Recommended:** Use this for regular maintenance instead of the full fix.

---

### 3. Fix Position Counts for a Specific Term ID

**When to use:** Fixing a specific term_id after a deposit/redemption issue, or targeted fixes.

**Command:**
```bash
PGPASSWORD='<password>' psql -h localhost -p 5435 -U <username> -d storage \
  -c "SELECT * FROM fix_position_counts_for_term('0x0875660252df712b34da0ed38b42dc8a69a62143f7fcc8c638993d8cab34c17c');"
```

**Output:**
```
 vault_updated | triple_vault_updated | triple_term_updated | predicate_object_updated | subject_predicate_updated 
---------------+----------------------+---------------------+--------------------------+---------------------------
             1 |                    1 |                   1 |                        1 |                         1
```

**What it does:**
- Updates `vault.position_count` for vaults with this term_id
- Updates `triple_vault.position_count` for triple_vaults that include this term_id (as term_id or counter_term_id)
- Updates `triple_term.total_position_count` for triple_terms that include this term_id
- Updates `predicate_object.total_position_count` for affected predicate_object pairs
- Updates `subject_predicate.total_position_count` for affected subject_predicate pairs

**Note:** This function does NOT update `stats.total_positions` (use function #1 or #2 for that).

---

## Understanding the Output

All functions return a table showing how many rows were updated in each table:

- **vault_updated**: Number of vault rows updated
- **triple_vault_updated**: Number of triple_vault rows updated
- **triple_term_updated**: Number of triple_term rows updated
- **predicate_object_updated**: Number of predicate_object rows updated
- **subject_predicate_updated**: Number of subject_predicate rows updated
- **stats_updated**: Number of stats rows updated (only in functions #1 and #2)

## How Position Counts Work

### Position Definition
A position is **active** when `shares > 0`. When a user fully redeems, `shares` becomes 0 and the position is considered closed.

### Count Aggregation Rules

1. **`vault.position_count`**: Count of positions with `shares > 0` for that specific `(term_id, curve_id)`

2. **`triple_vault.position_count`**: Count of positions with `shares > 0` for both `term_id` AND `counter_term_id` for that specific `curve_id`

3. **`triple_term.total_position_count`**: Sum of `vault.position_count` for both `term_id` AND `counter_term_id` across ALL curves

4. **`predicate_object.total_position_count`**: Sum of `triple_term.total_position_count` for all triples matching that `(predicate_id, object_id)` pair

5. **`subject_predicate.total_position_count`**: Sum of `triple_term.total_position_count` for all triples matching that `(subject_id, predicate_id)` pair

6. **`stats.total_positions`**: Total count of all positions with `shares > 0` in the entire database

## Verification

After running a fix function, you can verify the counts are correct:

```bash
# Check vault counts
PGPASSWORD='<password>' psql -h localhost -p 5435 -U <username> -d storage \
  -c "SELECT v.term_id, v.curve_id, v.position_count as stored, 
      COUNT(p.*) as actual 
      FROM vault v 
      LEFT JOIN position p ON p.term_id = v.term_id AND p.curve_id = v.curve_id AND p.shares > 0 
      WHERE v.term_id = '0x0875660252df712b34da0ed38b42dc8a69a62143f7fcc8c638993d8cab34c17c'
      GROUP BY v.term_id, v.curve_id, v.position_count;"
```

## Troubleshooting

### Deadlock Errors
If you get deadlock errors when running `fix_all_position_counts()`, try:
1. Run during low-traffic periods
2. Use `fix_wrong_position_counts()` instead (more efficient)
3. Fix specific term_ids one at a time using `fix_position_counts_for_term()`

### Function Not Found
If you get "function does not exist" error:
```bash
# Run the migration to create the functions
psql -h localhost -p 5435 -U <username> -d storage \
  -f infrastructure/hasura/migrations/intuition/1729947331638_fix_position_count_functions/up.sql
```

## Best Practices

1. **Regular Maintenance**: Run `fix_wrong_position_counts()` periodically (e.g., daily/weekly)
2. **After Issues**: If a specific term_id has wrong counts, use `fix_position_counts_for_term()`
3. **Full Recalculation**: Only use `fix_all_position_counts()` during maintenance windows or after major data migrations
4. **Monitor**: Check the output to see how many rows were updated - if numbers are high, investigate the root cause

## Authentication

All commands use placeholders `<username>` and `<password>`. Replace these with your actual database credentials. For better security:

- Use environment variables: `export PGPASSWORD='<password>'` then omit `PGPASSWORD='<password>'` from commands
- Use a `.pgpass` file: Create `~/.pgpass` with format `hostname:port:database:username:password`
- Use connection strings: `psql postgresql://username:password@localhost:5435/storage`

