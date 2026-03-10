# hasura-migrations-3.2.6

**Date:** 2026-03-10

## Performance: Leaderboard query optimization (1:30 → target sub-10s)

### Migration `1771526425000_use_cagg_for_price_lookups`

Rewrites `get_pnl_leaderboard_period` to eliminate the two remaining bottlenecks: the DISTINCT ON sort over `position_cumulative_hourly` (30s) and the full scan of `share_price_change` for price lookups (56s).

**Fix 1 — Composite index on `position_cumulative_hourly`:**
Adds `(account_id, term_id, curve_id, bucket DESC)` index. Step 2 now materializes distinct vaults first (`_tmp_vaults`), then does two `LATERAL ... ORDER BY bucket DESC LIMIT 1` index seeks per vault — one for snap_end, one for snap_start. Replaces the previous `DISTINCT ON` sort over all matching rows.

**Fix 2 — Index on `share_price_change_stats_hourly` cagg:**
Adds `(term_id, curve_id, bucket DESC)` index on the existing hourly continuous aggregate.

**Fix 3 — Price lookups via cagg instead of raw table:**
Replaces the `prices_at_start/end` CTEs (which scanned 371K raw `share_price_change` rows) with lookups against `share_price_change_stats_hourly` (303K rows, already materialized by TimescaleDB). For the current incomplete hour, a targeted patch queries only the ~50-100 vaults with very recent price changes from the raw table, preserving exact values.

**No output changes** — same results, dramatically faster execution. The cagg+patch approach ensures prices match the raw table exactly, including for the current hour.

### Benchmark (testnet-next, 221K accounts, 2025-01-01 → now)

| Step | Before | After |
|------|--------|-------|
| Active accounts | 2s | 2s |
| Position data | 30s | ~3s (index seeks) |
| Price lookups | 56s | ~1s (cagg + patch) |
| Aggregation + ranking | ~2s | ~2s |
| **Total** | **~90s** | **~8s** |
