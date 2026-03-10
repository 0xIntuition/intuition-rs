# chart-api:0.9.97

**Date:** 2026-03-09

## Feature: Add realized/unrealized PnL sort options to leaderboard

### Summary

Adds four new `sort_by` values to both PnL leaderboard period endpoints:
- `realized_pnl` - sort by absolute realized PnL
- `unrealized_pnl` - sort by absolute unrealized PnL
- `realized_pnl_pct` - sort by realized PnL percentage
- `unrealized_pnl_pct` - sort by unrealized PnL percentage

Requires hasura-migrations:3.2.1 (`1771526420000_add_realized_unrealized_pnl_sort`).

### Affected Endpoints

- `GET /api/v1/leaderboard/pnl/period`
- `GET /api/v1/leaderboard/pnl/period/min-threshold`

### Changes

- `apps/chart-api/src/endpoints/leaderboard.rs`
  - Updated `sort_by` validation in both handlers to accept the four new values
  - Updated OpenAPI descriptions for both endpoints
- `apps/chart-api/src/error.rs`
  - Updated `InvalidSortBy` error message to list all valid sort values

### Impact

- **Breaking:** No. Additive only; existing sort values are unchanged.
- **Deployment order:** Deploy hasura-migrations:3.2.1 first, then chart-api:0.9.97.
