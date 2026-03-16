# hasura-migrations-3.3.2

**Date:** 2026-03-11

## Bugfix: Missing start-of-period share prices causing inflated PnL

### Migration `1771526429000_add_start_price_fallback`

Fixes a critical bug where `get_pnl_leaderboard_period` returned massively inflated PnL numbers for any period starting before 2026-02-26.

**Impact:** The `share_price_change_stats_hourly` continuous aggregate (used for price lookups) is bucketed on `updated_at` (DB write time), which only has data from 2026-02-26 onward. For any period starting before that date, start-of-period price lookups returned NULL, causing `equity_at_start = 0` for all positions. The PnL formula `equity_at_end - equity_at_start + redemptions - deposits` then treated the entire ending equity as profit. In one verified case, an account's realized PnL was reported as +117,005 ETH instead of the correct -2,518 ETH.

**Root cause:** The `share_price_change` hypertable was reindexed around 2026-02-26. The `updated_at` column (DB write timestamp) only starts from that date, while `block_timestamp` (on-chain time) has full history back to 2025-11-01. The cagg inherits the `updated_at` gap.

**Fix:** Added a raw-table fallback after the cagg-based `_tmp_prices_at_start` lookup. For any vault that had shares at period start but got no price from the cagg, the fallback queries the raw `share_price_change` table using `block_timestamp`. The `NOT EXISTS` filter makes this self-optimizing — it's a no-op when the cagg has full coverage for the requested date range.

### Deployment notes

- **No schema changes** — only a `CREATE OR REPLACE FUNCTION`, runs instantly.
- **No statement timeout concerns** — this is a function replacement, not a data migration.
- **Backwards compatible** — function signature is unchanged; existing API calls work identically.
- **Cache invalidation required** — flush Redis keys matching `leaderboard:pnl:*` after deployment.
