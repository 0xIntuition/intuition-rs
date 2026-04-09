# Ops: Fix `position_cumulative_hourly` refresh job

**Date:** 2026-03-12

## Problem

The TimescaleDB `add_job` scheduler in version 2.20.1 has a bug where user-defined jobs fail with:

```
cache lookup failed for function 0
```

This affects the `refresh_position_cumulative_hourly` job on **both** environments:

| Environment | Namespace | DB StatefulSet | Job ID | Total Failures |
|---|---|---|---|---|
| mainnet-next | `intuition-mainnet-next` | `intuition-mainnet-next-timescale-db-0` | 1031 | all runs |
| mainnet-nested-triples | `intuition-mainnet-nested-triples` | `intuition-mainnet-nested-triples-timescale-db-0` | 1027 | all runs |

**Impact:** `position_cumulative_hourly` stops updating, causing the PnL leaderboard (`get_pnl_leaderboard_period`) to return stale data. The table falls behind by ~1 hour per hour the job is broken.

**Root cause:** TimescaleDB 2.20.1 background worker stores function OID 0 instead of the actual OID when registering user-defined jobs via `add_job()`. This persists across pod restarts, function renames, and `DROP`/`CREATE` cycles. The function works perfectly when called directly — only the scheduler resolution is broken.

## Fix: Kubernetes CronJob

Create a K8s CronJob that runs every hour and calls the refresh function directly on both databases, bypassing the broken TimescaleDB scheduler.

### Step 1: Clean up broken TimescaleDB jobs

Connect to each database and remove the broken jobs and leftover function variants:

**mainnet-next (port-forward to the DB pod in `intuition-mainnet-next`):**
```sql
-- Remove broken job
SELECT delete_job(1031);

-- Clean up function variants (keep only one)
DROP FUNCTION IF EXISTS refresh_position_cumulative_hourly(JSONB);
DROP FUNCTION IF EXISTS refresh_pos_cumul_hourly(JSONB);

-- Recreate canonical function
CREATE OR REPLACE FUNCTION refresh_position_cumulative_hourly(config JSONB)
RETURNS VOID LANGUAGE plpgsql AS $$
DECLARE
  v_last_bucket TIMESTAMPTZ;
BEGIN
  SELECT MAX(bucket) INTO v_last_bucket FROM position_cumulative_hourly;
  IF v_last_bucket IS NULL THEN RETURN; END IF;

  INSERT INTO position_cumulative_hourly (
    bucket, account_id, term_id, curve_id,
    cumulative_shares, cumulative_assets_in, cumulative_assets_out,
    cumulative_shares_in, cumulative_shares_out
  )
  SELECT
    pch.bucket, pch.account_id, pch.term_id, pch.curve_id,
    prev.cumulative_shares     + SUM(pch.shares_delta_period) OVER w,
    prev.cumulative_assets_in  + SUM(pch.assets_in_period)    OVER w,
    prev.cumulative_assets_out + SUM(pch.assets_out_period)   OVER w,
    prev.cumulative_shares_in  + SUM(pch.shares_in_period)    OVER w,
    prev.cumulative_shares_out + SUM(pch.shares_out_period)   OVER w
  FROM position_change_hourly pch
  JOIN LATERAL (
    SELECT cumulative_shares, cumulative_assets_in, cumulative_assets_out,
           cumulative_shares_in, cumulative_shares_out
    FROM position_cumulative_hourly existing
    WHERE existing.account_id = pch.account_id
      AND existing.term_id    = pch.term_id
      AND existing.curve_id   = pch.curve_id
    ORDER BY existing.bucket DESC
    LIMIT 1
  ) prev ON true
  WHERE pch.bucket > v_last_bucket
  WINDOW w AS (
    PARTITION BY pch.account_id, pch.term_id, pch.curve_id
    ORDER BY pch.bucket
    ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
  )
  ON CONFLICT DO NOTHING;
END;
$$;
```

**mainnet-nested-triples (port-forward to the DB pod in `intuition-mainnet-nested-triples`):**
```sql
-- Remove broken job
SELECT delete_job(1027);

-- Same function cleanup and recreate as above
DROP FUNCTION IF EXISTS refresh_position_cumulative_hourly(JSONB);

CREATE OR REPLACE FUNCTION refresh_position_cumulative_hourly(config JSONB)
RETURNS VOID LANGUAGE plpgsql AS $$
-- (same body as above)
$$;
```

### Step 2: Catch up stale data

Before the CronJob takes over, manually refresh both databases to current:

```sql
-- Run on each DB — repeat until max(bucket) is within 1 hour of now
SELECT refresh_position_cumulative_hourly(NULL::JSONB);
SELECT max(bucket) FROM position_cumulative_hourly;
```

As of 2026-03-12:
- mainnet-next (5436): last bucket `2026-03-12 11:00` (already caught up)
- mainnet-nested-triples (5437): last bucket `2026-03-11 18:00` (needs catch-up)

### Step 3: Deploy K8s CronJob

Create a CronJob that runs hourly (e.g., at minute 5 past the hour, after the cagg refresh completes) and calls the function on both databases.

**Required secrets/config:**
- DB connection string for mainnet-next TimescaleDB
- DB connection string for mainnet-nested-triples TimescaleDB
- Both use: user `intuition_admin`, database `storage`, port `5432` (internal cluster port)

**Internal service hostnames (verify these):**
- `intuition-mainnet-next-timescale-db.intuition-mainnet-next.svc.cluster.local`
- `intuition-mainnet-nested-triples-timescale-db.intuition-mainnet-nested-triples.svc.cluster.local`

**SQL to execute on each DB:**
```sql
SELECT refresh_position_cumulative_hourly(NULL::JSONB);
```

**CronJob schedule:** `5 * * * *` (5 minutes past every hour)

**Container image:** Any image with `psql` (e.g., `postgres:16-alpine`)

**Failure handling:** The function is idempotent (uses `ON CONFLICT DO NOTHING`), so retries are safe.

### Step 4: Verify

After the first CronJob run, check both databases:

```sql
-- Should show Success
SELECT last_run_status FROM timescaledb_information.job_stats WHERE job_id = <id>;

-- max(bucket) should be within ~1 hour of current time
SELECT max(bucket) FROM position_cumulative_hourly;
```

## Additional issues found on mainnet-nested-triples (5437/5439)

During investigation, the following data issues were found and fixed on the nested-triples environment:

1. **`position_cumulative_hourly` had 3x duplicate rows** — the backfill migration ran 3 times. Fixed by dropping and rebuilding the table from `position_change_hourly`.

2. **`share_price_change` was missing 10 days of data** (Feb 16–26) — the indexer started later on this environment. Fixed by copying 425,501 rows from mainnet-next. The `share_price_change_stats_hourly` cagg was then refreshed for the backfilled range.

3. **`curve_config` had wrong parameters** — `half_slope` was 20x too high, `offset` was 60x too low, `contract_address` was empty. This caused `redeemable_assets` in `position_with_value` to be ~17x inflated. Fixed with:
   ```sql
   UPDATE curve_config
   SET slope = 100000000000000000,
       half_slope = 50000000000000000,
       "offset" = 30000000000000000000,
       contract_address = '0x6E35cF57A41fA15eA0EaE9C33e751b01A784Fe7e'
   WHERE curve_id = 2;
   ```

## Long-term fix

Upgrade TimescaleDB past 2.20.1 to a version where `add_job` correctly resolves user-defined function OIDs. This would eliminate the need for the external CronJob. Plan this as a separate maintenance window with proper testing, as upgrades may affect compressed chunks and continuous aggregates.
