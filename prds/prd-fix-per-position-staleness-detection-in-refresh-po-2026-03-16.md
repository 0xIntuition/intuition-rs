# Fix Per-Position Staleness Detection in `refresh_position_cumulative_hourly`

## Goal
Rewrite the `refresh_position_cumulative_hourly` SQL function so that positions with gaps in activity have their cumulative data correctly updated when activity resumes. The current function silently drops data for any position whose new activity falls before the global `MAX(bucket)`, causing stale leaderboard data and incorrect PnL calculations.

## Context
The `position_cumulative_hourly` table stores running totals of shares, deposits, and redemptions per `(account_id, term_id, curve_id)` at hourly granularity. It is the primary data source for `get_pnl_leaderboard_period` (Step 2) and drives all PnL, equity, and volume calculations surfaced in the product.

A Kubernetes CronJob calls `refresh_position_cumulative_hourly` every hour (at `:05`) to append new rows from the `position_change_hourly` continuous aggregate. The function currently:
1. Computes a single `v_last_bucket = MAX(bucket)` across the **entire** `position_cumulative_hourly` table.
2. Filters `position_change_hourly` with `WHERE pch.bucket > v_last_bucket`.

This assumes all positions advance in lockstep. In reality, positions have independent timelines — a position can go inactive for months and then resume. When it resumes, its new `position_change_hourly` rows have buckets earlier than the global max, so the `WHERE` clause skips them entirely. On the current production database, **39 positions** were affected, with the worst case being **134 days behind**.

The existing CronJob manifest and schedule do not need to change — only the SQL function body requires a rewrite.

### Current performance baseline

The current function completes in **1-5 seconds** per hourly run. Any rewrite must not regress this significantly.

- `position_cumulative_hourly`: hypertable, ~3.15M rows, index on `(account_id, term_id, curve_id, bucket DESC)`, compression after 30 days
- `position_change_hourly`: continuous aggregate (hypertable), ~3.15M rows
- ~3.14M unique `(account_id, term_id, curve_id)` position keys
- `mainnet-next` has a default `statement_timeout` of 60 seconds

## Deliverable
A new Hasura migration that replaces `refresh_position_cumulative_hourly` with a version that detects and processes stale positions **per `(account_id, term_id, curve_id)`** rather than relying on a single global max bucket.

The new function must:
- Preserve the existing global-max fast path for the common case (all positions are current).
- Detect stale and new positions **only when new data exists**, without scanning the full `position_cumulative_hourly` table in the common case.
- For each stale position key `(account_id, term_id, curve_id)`, find its latest bucket in `position_cumulative_hourly` and process only newer rows from `position_change_hourly`.
- Compute running cumulative totals starting from the position's existing last cumulative row.
- Insert the missing rows into `position_cumulative_hourly`.
- **Handle brand-new positions** that have rows in `position_change_hourly` but zero rows in `position_cumulative_hourly` (created after the initial backfill). These must be bootstrapped with their full cumulative history using window functions, identical to the original backfill logic in migration `1771526427000`. Bootstrap at most **100 new positions per run** to stay within the timeout budget; the rest will be caught in subsequent runs.

Additionally, include a one-time backfill step in the migration to repair the 39 (or more, by the time this ships) currently-stale positions, as well as any positions with zero cumulative rows.

## Acceptance Criteria

### Correctness
- [ ] New migration contains a `CREATE OR REPLACE FUNCTION refresh_position_cumulative_hourly(config JSONB)` that detects and processes stale positions individually rather than relying on a global `MAX(bucket)`
- [ ] A position that was inactive for N hours/days and then resumes activity gets its cumulative rows computed and inserted on the next refresh run
- [ ] A brand-new position (exists in `position_change_hourly` but has zero rows in `position_cumulative_hourly`) gets bootstrapped with its full cumulative history on the next refresh run (batched, up to 100 per run)
- [ ] The function remains idempotent — running it twice in a row produces no duplicate rows (`ON CONFLICT DO NOTHING` for the incremental function)
- [ ] The one-time migration backfill uses `ON CONFLICT (...) DO UPDATE SET ...` to authoritatively correct any previously-incorrect rows from partial prior runs
- [ ] The migration includes a backfill query that repairs all currently-stale positions AND bootstraps any positions with zero cumulative rows
- [ ] After deploying the migration and running the function once, the staleness healthcheck query (see Monitoring section) returns 0
- [ ] The `get_pnl_leaderboard_period` function returns correct PnL data for accounts that had gap-then-resume activity patterns

### Performance
- [ ] **Common case (zero stale positions):** the function short-circuits in under 5 seconds, comparable to the current 1-5 second baseline. No full GROUP BY or DISTINCT ON across either table
- [ ] **New data exists:** the function completes within 60 seconds on production data volumes, including per-position staleness detection and processing
- [ ] **One-time migration backfill:** allowed up to 10 minutes (`SET statement_timeout = '10min'` as the first statement in the migration file, before any DML)
- [ ] The function sets `SET LOCAL work_mem = '64MB'` for hash aggregates (same pattern as `get_pnl_leaderboard_period`)
- [ ] New position bootstraps are batched (max 100 per run) to prevent a burst of new positions from blowing the timeout

### Compatibility
- [ ] The function signature (`config JSONB`) and return type (`VOID`) remain unchanged so the existing K8s CronJob continues to work without modification
- [ ] `down.sql` restores the previous function definition
- [ ] Tested on testnet-next before mainnet-next deployment

### Monitoring
- [ ] The following healthcheck query is documented for **manual ops use and alerting only** (this query is too expensive to run inside the function itself — see Performance Constraints):
```sql
WITH cumul AS (
  SELECT DISTINCT ON (account_id, term_id, curve_id)
    account_id, term_id, curve_id, bucket as last_cum_bucket
  FROM position_cumulative_hourly
  ORDER BY account_id, term_id, curve_id, bucket DESC
),
cagg AS (
  SELECT account_id, term_id, curve_id, max(bucket) as last_cagg_bucket
  FROM position_change_hourly
  GROUP BY account_id, term_id, curve_id
)
SELECT count(*) as stale_positions
FROM cagg c
LEFT JOIN cumul cu ON c.account_id = cu.account_id AND c.term_id = cu.term_id AND c.curve_id = cu.curve_id
WHERE cu.last_cum_bucket IS NULL                                    -- missing entirely
   OR c.last_cagg_bucket > cu.last_cum_bucket + interval '2 hours'; -- behind
```
- [ ] **WARNING:** This query scans 3.15M rows in each table (GROUP BY + DISTINCT ON) and takes 20-60 seconds. It must NOT be used as the Phase 1 detection logic inside the refresh function. It is intended for periodic manual checks or low-frequency alerting only.
- [ ] Expected result after a healthy refresh: `stale_positions = 0`

## Dependencies
- The `position_change_hourly` continuous aggregate must be refreshed before this function runs (currently guaranteed by the CronJob scheduling at `:05`, five minutes after the cagg refresh).
- Migration must run on both `mainnet-next` and `mainnet-nested-triples` environments. The `mainnet-next` environment has a `statement_timeout` of 1 minute by default — the migration must `SET statement_timeout = '10min'` as its **first statement** (session-level, not `SET LOCAL`) before any DML, following the same pattern as migration `1771526427000`.

## Technical Notes

### Performance constraints (critical)

The single most important constraint: **the common case (zero stale positions) must not regress from the current 1-5 second baseline.** Any approach that scans the full `position_cumulative_hourly` or `position_change_hourly` table on every run is unacceptable — both tables have 3.15M rows and a full GROUP BY / DISTINCT ON takes 20-60 seconds, which exceeds the 60-second `statement_timeout` on mainnet-next.

### Three-phase approach (required)

The function must use a three-phase design:

**Phase 0 — Global fast path (preserves current performance):**

Keep the existing `MAX(bucket)` check as the first operation:

```sql
SELECT MAX(bucket) INTO v_global_last FROM position_cumulative_hourly;
SELECT MAX(bucket) INTO v_cagg_last FROM position_change_hourly;

IF v_cagg_last <= v_global_last THEN
  -- No new data at all. Process the existing global-max batch (same as current function).
  -- Then RETURN.
END IF;
```

Both are single-column MAX on the time dimension — index-only scans with chunk exclusion, under 1 second total. This handles the common case where all positions advance in lockstep.

When `v_cagg_last > v_global_last`, first process the global batch (same as the current function), then proceed to Phase 1 for per-position detection.

**Phase 1 — Detect stale positions (only runs when new data exists):**

After the global batch is processed, detect positions that were skipped. Build a temp table of stale positions:

1. **Gap-resume positions**: positions where `position_change_hourly` has rows newer than their individual latest `position_cumulative_hourly` bucket, but those rows were below the global max and therefore skipped.
2. **New positions**: positions that exist in `position_change_hourly` but have zero rows in `position_cumulative_hourly`.

This phase is more expensive (it must compare per-position max buckets), but it only runs when there is actually new data, not on every invocation. Even so, avoid scanning the full `position_cumulative_hourly` if possible — consider filtering to positions that had activity in `position_change_hourly` in the recent window (e.g., `bucket > v_global_last - interval '1 hour'`), then checking if those positions are stale.

If the stale positions temp table is empty, `RETURN`.

**Phase 2 — Process stale positions:**

- For **gap-resume positions**: use a `LATERAL` join or loop where each position drives its own time filter against `position_change_hourly`, enabling per-position chunk exclusion. Compute running totals from the position's last cumulative row.
- For **new positions** (batched, max 100): compute full cumulative history using window functions over all their `position_change_hourly` rows.

### Chunk exclusion (critical)

**DO NOT use `bucket > MIN(all stale last buckets)` as a shared outer filter.** If even one position has a gap of weeks/months, the MIN pushes the filter back across all chunks, negating chunk exclusion for every other position.

Instead, process stale positions with **per-position time filters**:

```sql
-- Option A: LATERAL join (set-based, preferred for small batches)
FROM stale_positions sp
JOIN LATERAL (
  SELECT * FROM position_change_hourly pch
  WHERE pch.account_id = sp.account_id
    AND pch.term_id    = sp.term_id
    AND pch.curve_id   = sp.curve_id
    AND pch.bucket     > sp.last_cum_bucket
  ORDER BY pch.bucket
) pch ON true

-- Option B: Loop (explicit, guaranteed per-position chunk exclusion)
FOR rec IN SELECT * FROM stale_positions LOOP
  INSERT INTO position_cumulative_hourly ...
  FROM position_change_hourly pch
  WHERE pch.account_id = rec.account_id
    AND pch.term_id    = rec.term_id
    AND pch.curve_id   = rec.curve_id
    AND pch.bucket     > rec.last_cum_bucket
  ...
END LOOP;
```

Each position gets its own time-bounded query, enabling TimescaleDB to exclude irrelevant chunks individually. For 0-5 stale positions per normal run, the overhead is trivial.

### LATERAL join `prev` lookup: correctness under partial failures

The LATERAL join that fetches the previous cumulative row must be bounded to prevent picking up rows from a prior failed/partial run:

```sql
JOIN LATERAL (
  SELECT ...
  FROM position_cumulative_hourly existing
  WHERE existing.account_id = pch.account_id
    AND existing.term_id    = pch.term_id
    AND existing.curve_id   = pch.curve_id
    AND existing.bucket    <= rec.last_cum_bucket  -- explicit upper bound
  ORDER BY existing.bucket DESC
  LIMIT 1
) prev ON true
```

Without the upper bound, a partial prior run that inserted some rows before failing would cause the LATERAL to find those partial rows as `prev`, potentially producing incorrect cumulative totals.

### Backfill conflict strategy

- **One-time migration backfill**: use `ON CONFLICT (account_id, term_id, curve_id, bucket) DO UPDATE SET cumulative_shares = EXCLUDED.cumulative_shares, ...` to authoritatively overwrite any previously-incorrect rows.
- **Incremental hourly function**: use `ON CONFLICT DO NOTHING` for idempotency (the normal case where rows are correct).

### New position batch limit

The hourly function must limit bootstrap of new positions to **100 per run**. If more than 100 new positions exist, process the first 100 and let subsequent runs handle the rest. This prevents a burst of new positions (e.g., after a backfill or migration) from exceeding the 60-second timeout.

```sql
SELECT * FROM new_positions ORDER BY ... LIMIT 100;
```

### Optional: `position_cumulative_state` summary table

For long-term scalability, consider maintaining a flat summary table:

```sql
CREATE TABLE position_cumulative_state (
  account_id TEXT NOT NULL,
  term_id    TEXT NOT NULL,
  curve_id   NUMERIC NOT NULL,
  last_bucket TIMESTAMPTZ NOT NULL,
  PRIMARY KEY (account_id, term_id, curve_id)
);
```

Updated via `ON CONFLICT DO UPDATE` whenever the refresh function writes rows. This eliminates the need for `DISTINCT ON` across the time-partitioned hypertable for staleness detection — Phase 1 becomes a simple hash join between two flat tables. This is not required for the initial implementation but should be considered if Phase 1 performance becomes a concern as data grows.

### Compression

Rows older than 30 days in `position_cumulative_hourly` are compressed. The backfill for positions 134+ days stale will insert into uncompressed chunks (current time), not retroactively into compressed ones, so no decompression is needed.

### Migration numbering

Follow the existing convention — next sequence number after `1771526429000`.

### Edge case: position with cumulative row but no cagg row

This should not happen in practice (cumulative is derived from the cagg), but the function should handle it gracefully — skip these positions, do not error.
