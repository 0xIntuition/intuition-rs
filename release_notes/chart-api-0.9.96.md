# chart-api:0.9.96

**Date:** 2026-03-04

## Feature: Add min_deposit threshold endpoint for PnL leaderboard

### Summary

Adds a new endpoint `GET /api/v1/leaderboard/pnl/period/min-threshold` that supports filtering out positions where cumulative all-time deposits are below a configurable threshold. This prevents leaderboard gaming with tiny stakes.

Requires hasura-migrations:3.1.8 (`1771526410000_add_min_deposit_filter`).

### New Endpoint

`GET /api/v1/leaderboard/pnl/period/min-threshold`

Same parameters as `/api/v1/leaderboard/pnl/period` plus:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `min_deposit` | `f64` | `0` | Minimum cumulative deposit threshold in ETH/TRUST |

### Example

```
GET /api/v1/leaderboard/pnl/period/min-threshold?start=2026-01-01T00:00:00Z&end=2026-03-01T00:00:00Z&sort_by=pnl_pct&sort_order=DESC&min_deposit=5
```

### Changes

- `apps/chart-api/src/types.rs`
  - Added `PnlLeaderboardPeriodMinThresholdQueryParams` struct with `min_deposit` field
- `apps/chart-api/src/services/leaderboard.rs`
  - Added `fetch_pnl_leaderboard_period_min_threshold` function (11-param SQL call)
- `apps/chart-api/src/endpoints/leaderboard.rs`
  - Added `get_pnl_leaderboard_period_min_threshold` handler with separate cache key
- `apps/chart-api/src/app.rs`
  - Registered new route at `/api/v1/leaderboard/pnl/period/min-threshold`
- `apps/chart-api/src/openapi.rs`
  - Added new endpoint to Swagger/OpenAPI documentation

### Hasura Action

New action `get_pnl_leaderboard_period_min_threshold` added to `actions.graphql` and `actions.yaml` with `p_min_deposit` parameter.

### Impact

- **Breaking:** No. New endpoint only; existing `/api/v1/leaderboard/pnl/period` is unchanged.
- **Deployment order:** Deploy hasura-migrations:3.1.8 first, then chart-api:0.9.96, then Hasura metadata apply.
