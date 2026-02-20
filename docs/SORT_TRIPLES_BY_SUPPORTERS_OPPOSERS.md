# Sorting triples by number of supporters / opposers

**Goal:** Sort triples by supporter count and/or opposer count. **Constraint:** No Rust changes; SQL + Hasura only.

---

## Data model (current)

- **Supporters** = positions on the **triple** vaults (`position.term_id = triple.term_id`).
- **Opposers** = positions on the **counter-triple** vaults (`position.term_id = triple.counter_term_id`).
- **vault** already has `position_count` per (term_id, curve_id), maintained by existing triggers on `position`.
- **triple_term** has `total_position_count` = sum of `vault.position_count` for **both** term_id and counter_term_id; it does **not** split supporter vs opposer.

So we can derive supporter/opposer counts from **vault.position_count** without touching the `position` table or adding new triggers on it:

- **supporter_count** = `SUM(vault.position_count)` where `vault.term_id = triple_term.term_id`
- **opposer_count** = `SUM(vault.position_count)` where `vault.term_id = triple_term.counter_term_id`

Same source of truth as `total_position_count`, just split by side.

---

## Option A: Add columns to `triple_term` + extend existing vault sync (recommended)

**Idea:** Add `supporter_count` and `opposer_count` to `triple_term`, and set them in the **existing** logic that already updates `triple_term` from `vault` (no new trigger on `position`).

**Steps:**

1. **Migration**
   - Add to `triple_term`:
     - `supporter_count BIGINT NOT NULL DEFAULT 0`
     - `opposer_count BIGINT NOT NULL DEFAULT 0`
   - Optional: indexes for sort, e.g. `CREATE INDEX ... ON triple_term (supporter_count DESC);`.

2. **Extend existing triggers** that update `triple_term` from vault:
   - In **update_triple_term_totals** (trigger on `triple_vault`): when setting `total_position_count`, also set  
     `supporter_count = COALESCE((SELECT SUM(position_count) FROM vault WHERE term_id = term_id_val), 0)`,  
     `opposer_count = COALESCE((SELECT SUM(position_count) FROM vault WHERE term_id = counter_term_id_val), 0)`.
   - In **update_triple_vault_from_vault** (trigger on `vault`): in the block that updates `triple_term`, add the same two SETs (using `triple_term.term_id` and `triple_term.counter_term_id` in the subqueries).

   So whenever vault (or triple_vault) changes, `triple_term` already gets updated; you only add two more columns to that update. No new trigger on `position`.

3. **Backfill**
   - One-time:
     ```sql
     UPDATE triple_term tt
     SET
       supporter_count = COALESCE((SELECT SUM(position_count) FROM vault WHERE term_id = tt.term_id), 0),
       opposer_count   = COALESCE((SELECT SUM(position_count) FROM vault WHERE term_id = tt.counter_term_id), 0);
     ```

4. **Hasura**
   - Expose `supporter_count` and `opposer_count` on `triple_term`.
   - Sort via `triple_term`, e.g. `triples(order_by: { triple_term: { supporter_count: desc } })` or `triple_terms(order_by: { supporter_count: desc })`.

**Pros:** Reuses `vault.position_count`, no new trigger on `position`, indexable, consistent with `total_position_count`. **Cons:** Two new columns and a small change to two existing trigger functions.

---

## Option B: View (no schema or trigger changes)

**Idea:** A view that exposes `triple_term` (or triple) plus supporter/opposer counts from `vault`.

- **supporter_count** = `(SELECT COALESCE(SUM(position_count), 0) FROM vault WHERE term_id = triple_term.term_id)`
- **opposer_count** = `(SELECT COALESCE(SUM(position_count), 0) FROM vault WHERE term_id = triple_term.counter_term_id)`

Track the view in Hasura and sort on it.

**Pros:** No migration, no trigger changes. **Cons:** No index on the counts; sort can be slower on large data.

---

## Option C: Hasura custom function

**Idea:** A PostgreSQL function that returns triples (or triple_term–like rows) with `supporter_count` and `opposer_count` from `vault`, tracked as a Hasura function.

**Pros:** No new columns. **Cons:** Less convenient for generic `order_by` and pagination than a table/view with columns.

---

## Recommendation

- **Preferred:** **Option A** — add `supporter_count` and `opposer_count` to `triple_term`, derive them from `vault.position_count` in the existing vault → triple_term sync, backfill once, then sort in Hasura. No new trigger on `position`; you only extend the current aggregation logic.
