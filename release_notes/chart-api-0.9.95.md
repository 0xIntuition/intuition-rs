# chart-api:0.9.95

**Date:** 2026-03-04

## Feature: Add realized_pnl_pct and unrealized_pnl_pct to leaderboard response

### Summary

Adds `realized_pnl_pct` and `unrealized_pnl_pct` fields to the PnL leaderboard API response. These provide ROI percentages broken down by closed vs open positions, complementing the existing `pnl_pct` (total ROI) and the raw `realized_pnl_raw` / `unrealized_pnl_raw` amounts.

Requires hasura-migrations:3.1.7 (`1771526409000_add_realized_unrealized_pnl_pct`).

### New Response Fields

```json
{
  "pnl_pct": "67.7448",
  "realized_pnl_pct": "0",
  "unrealized_pnl_pct": "67.7448",
  ...
}
```

### Changes

- `apps/chart-api/src/services/leaderboard.rs`
  - Added `realized_pnl_pct: BigDecimal` and `unrealized_pnl_pct: BigDecimal` to `PnlLeaderboardEntryRow` (36 columns, was 34)
- `apps/chart-api/src/models/leaderboard.rs`
  - Added `realized_pnl_pct: String` and `unrealized_pnl_pct: String` to `PnlLeaderboardEntry` and `PnlLeaderboardEntrySchema`
- `apps/chart-api/src/endpoints/leaderboard.rs`
  - Added mapping for both new fields in the row-to-response conversion

### Impact

- **Breaking:** No. Adds new fields to JSON response; existing consumers that ignore unknown fields are unaffected.
- **Deployment order:** Deploy hasura-migrations:3.1.7 first, then chart-api:0.9.95.
