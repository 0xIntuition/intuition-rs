# hasura-migrations-3.2.3

**Date:** 2026-03-09

## Performance: Leaderboard query optimization (2.5 min → sub-10s)

### Migration `1771526422000_prefilter_protocol_accounts_and_shortcircuit`

**Quick win 1 — Pre-filter protocol accounts:** Moves the `ProtocolVault`/`AtomWallet` account exclusion from the `enriched` CTE (late) into `_tmp_active_accounts` (early), eliminating ~43% of accounts before the expensive position data step.

**Quick win 2 — Short-circuit pre-period scan:** Detects when `p_start_date` predates all cagg data and skips the unbounded history scan. Adds a lower bound (`bucket >= v_bucket_start`) to the LATERAL when no pre-period data exists, turning a full-history scan into a tight range scan.

**No output changes** — same results, faster execution.

---

### Migration `1771526423000_add_position_cumulative_hourly`

Creates `position_cumulative_hourly`, a hypertable storing running totals of shares, deposits, and redemptions per `(account_id, term_id, curve_id)` at hourly granularity.

**Table columns:** `bucket, account_id, term_id, curve_id, cumulative_shares, cumulative_assets_in, cumulative_assets_out, cumulative_shares_in, cumulative_shares_out`

**Includes:**
- Initial backfill from `position_change_hourly` (window function over all historical data)
- Hourly refresh job (`refresh_position_cumulative_hourly`) scheduled 5 minutes after the cagg refresh
- Compression policy (30-day retention for uncompressed data)
- Sparse storage — only rows where activity occurred

---

### Migration `1771526424000_use_cumulative_snapshot_in_leaderboard`

Rewrites `get_pnl_leaderboard_period` Step 2 to use `position_cumulative_hourly` instead of the LATERAL full-history scan over `position_change_daily`.

**Before:** LATERAL joins 126K accounts against 3.1M cagg rows, scanning all historical buckets per account to compute `shares_at_start`. Cost: ~72 seconds.

**After:** Two indexed point-in-time lookups per `(account, term, curve)`:
- `snap_end`: latest cumulative at `p_end_date`
- `snap_start`: latest cumulative before `p_start_date`
- Period deltas = `snap_end - snap_start`

**No output changes** — all downstream CTEs (`prices_at_start/end`, `position_period_metrics`, `position_pnl`, `account_metrics`, `enriched`, `ranked`) and the final SELECT are unchanged. The `_tmp_position_data` temp table produces identical columns and semantics.
