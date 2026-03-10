# hasura-migrations-3.2.7

**Date:** 2026-03-10

## Performance: Leaderboard price lookup optimization

### Migration `1771526426000_fix_use_cagg_for_price_lookups`

Fixes migration `1771526425000` which introduced a performance regression by replacing the working DISTINCT ON position data approach with a slower `_tmp_vaults` + LATERAL pattern (6:30 vs 1:30). This migration reverts Step 2 to the proven DISTINCT ON approach and keeps only the price lookup optimization.

**Fix 1 — Drop unnecessary index:** Removes `idx_pch_account_term_curve_bucket` on `position_cumulative_hourly` created by `1771526425000` — it wasn't helping and adds write overhead.

**Fix 2 — Index on `share_price_change_stats_hourly` cagg:** Adds `(term_id, curve_id, bucket DESC)` index on the existing hourly continuous aggregate for efficient DISTINCT ON lookups.

**Fix 3 — Price lookups via cagg instead of raw table:** Replaces the `prices_at_start/end` CTEs (which scanned 371K raw `share_price_change` rows) with lookups against `share_price_change_stats_hourly` (303K rows, already materialized by TimescaleDB). For the current incomplete hour, a targeted patch queries only the ~50-100 vaults with very recent price changes from the raw table, preserving exact values.

**No output changes** — same results, faster execution. The cagg+patch approach ensures prices match the raw table exactly, including for the current hour.

### Expected benchmark (testnet-next, 221K accounts, 2025-01-01 → now)

| Step | Before (1771526424000) | After |
|------|----------------------|-------|
| Active accounts | 2s | 2s |
| Position data (DISTINCT ON) | 30s | 30s |
| Price lookups | 56s | ~1s (cagg + patch) |
| Aggregation + ranking | ~2s | ~2s |
| **Total** | **~90s** | **~35s** |
