# Ticket: TimescaleDB 2.20.1 user-defined job scheduler bug

## Parent Ticket

**Title:** TimescaleDB 2.20.1 `add_job` fails to resolve user-defined functions — `cache lookup failed for function 0`

**Priority:** High

**Affected environments:**
- `intuition-mainnet-next` (namespace: `intuition-mainnet-next`, pod: `intuition-mainnet-next-timescale-db-0`)
- `intuition-mainnet-nested-triples` (namespace: `intuition-mainnet-nested-triples`, pod: `intuition-mainnet-nested-triples-timescale-db-0`)

**Description:**

TimescaleDB 2.20.1's background worker job scheduler cannot execute user-defined functions registered via `add_job()`. Every execution fails immediately (~8ms) with:

```
cache lookup failed for function 0
```

The background worker stores OID `0` instead of the actual function OID when registering jobs. This is not recoverable through:
- Pod restarts (OID 0 is persisted in `_timescaledb_config.bgw_job`)
- Dropping and recreating the function (new OID assigned, but job still resolves to 0)
- Renaming the function
- Explicit schema qualification (`public.function_name`)
- Deleting the job and re-adding with `add_job()`

Built-in `policy_*` jobs (compression, cagg refresh) are unaffected — only user-defined functions fail.

**Impact:**

The `refresh_position_cumulative_hourly` job stopped updating the `position_cumulative_hourly` hypertable. This table provides point-in-time cumulative snapshots used by `get_pnl_leaderboard_period()` for period-based PnL calculations. Without hourly refreshes, leaderboard data goes stale at a rate of 1 hour per hour.

**Root cause:**

TimescaleDB 2.20.1 bug in the background worker's function resolution. The `bgw_job` catalog stores `proc_schema` and `proc_name` correctly, but the worker resolves the function OID to `0` at execution time.

**Acceptance criteria:**
- `position_cumulative_hourly` stays within 1 hour of current time on both environments
- Long-term: upgrade TimescaleDB to a version where `add_job()` correctly resolves user-defined functions
- Verify no data migration issues with compressed chunks and continuous aggregates before upgrading

---

## Sub-ticket: Quickfix

**Title:** Deploy K8s CronJob to refresh `position_cumulative_hourly` (workaround for TimescaleDB scheduler bug)

**Priority:** High

**Status:** Done

**Description:**

Deployed a Kubernetes CronJob that bypasses the broken TimescaleDB job scheduler by calling the refresh function directly via `psql` on a `5 * * * *` schedule (5 minutes past every hour, after the cagg refresh completes).

**What was done:**

1. **Cleaned up broken TimescaleDB jobs** on both environments:
   - `intuition-mainnet-next`: deleted job 1027 (`refresh_position_cumulative_hourly`, 26/26 failures)
   - `intuition-mainnet-nested-triples`: deleted job 1031 (`refresh_pos_cumul_hourly`, 3/3 failures)

2. **Ensured the `refresh_position_cumulative_hourly(JSONB)` function exists** on both databases. The function incrementally appends new rows from `position_change_hourly` using a LATERAL join against the latest existing cumulative row per `(account_id, term_id, curve_id)`. It is idempotent (`ON CONFLICT DO NOTHING`).

3. **Deployed a K8s CronJob** (schedule: `5 * * * *`) that runs `SELECT refresh_position_cumulative_hourly(NULL::JSONB)` on both databases:
   - `intuition-mainnet-next-timescale-db.intuition-mainnet-next.svc.cluster.local`
   - `intuition-mainnet-nested-triples-timescale-db.intuition-mainnet-nested-triples.svc.cluster.local`

4. **Manually caught up stale data** before CronJob deployment:
   - `intuition-mainnet-next`: was 18 hours behind, caught up to current
   - `intuition-mainnet-nested-triples`: was 18 hours behind, caught up to current

5. **Verified** with test jobs — both returned `Complete` and `max(bucket)` is at `2026-03-12 11:00:00+00` on both DBs.

**Additional data fixes applied to `intuition-mainnet-nested-triples` during investigation:**

| Issue | Impact | Fix |
|---|---|---|
| `position_cumulative_hourly` had 3x duplicate rows (9.4M vs expected 3.15M) | Leaderboard triple-counted deposits/redemptions | Dropped table, re-ran backfill migration |
| `share_price_change` missing 10 days of data (Feb 16–26) | Leaderboard couldn't find share prices at period start, wrong equity calculations | Copied 425,501 rows from mainnet-next, refreshed cagg |
| `curve_config` had wrong parameters (`half_slope` 20x too high, `offset` 60x too low) | `redeemable_assets` in `position_with_value` inflated ~17x | Updated to match mainnet-next values |

**Rollback plan:**

If the CronJob causes issues, delete it — the table will simply stop updating (same as current broken state). The function can also be called manually:

```sql
SELECT refresh_position_cumulative_hourly(NULL::JSONB);
```
