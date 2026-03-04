# hasura-migrations:3.1.7

**Date:** 2026-03-04

## Feature: Add realized_pnl_pct and unrealized_pnl_pct to PnL Leaderboard

### Summary

Adds `realized_pnl_pct` and `unrealized_pnl_pct` fields to the PnL leaderboard, providing ROI percentages broken down by closed vs open positions. Previously only total `pnl_pct` was available alongside the raw realized/unrealized amounts.

Also fixes the `get_vault_leaderboard_period` function's `pnl_pct` denominator to use `(equity_at_start + period_deposits)` instead of the buggy CASE expression that ignored in-period deposits (same fix already applied to `get_pnl_leaderboard_period` in 3.1.5).

### New Fields

| Field | Type | Description |
|-------|------|-------------|
| `realized_pnl_pct` | `NUMERIC(20,4)` | ROI % on closed positions |
| `unrealized_pnl_pct` | `NUMERIC(20,4)` | ROI % on open positions |

### Formulas

**All-time functions** (`get_pnl_leaderboard`, `get_vault_leaderboard`):

```
realized_pnl_pct   = realized_pnl   × 100 / deposits_on_closed_positions
unrealized_pnl_pct = unrealized_pnl × 100 / deposits_on_open_positions
```

- `deposits_on_closed_positions` = `SUM(total_deposits) WHERE shares <= 0`
- `deposits_on_open_positions` = `SUM(total_deposits) WHERE shares > 0`

**Period functions** (`get_pnl_leaderboard_period`, `get_vault_leaderboard_period`):

```
realized_pnl_pct   = realized_pnl   × 100 / SUM(equity_at_start + period_deposits) for closed positions
unrealized_pnl_pct = unrealized_pnl × 100 / SUM(equity_at_start + period_deposits) for open positions
```

- "closed" = positions where `shares_at_end <= 0`
- "open" = positions where `shares_at_end > 0`
- Returns `0` when denominator is 0

### Migration

`1771526409000_add_realized_unrealized_pnl_pct`

### Changes

- `infrastructure/hasura/migrations/intuition/1771526409000_add_realized_unrealized_pnl_pct/up.sql`
  - `DROP TABLE IF EXISTS pnl_leaderboard_entry CASCADE` — cascades to all 4 functions
  - Recreate `pnl_leaderboard_entry` with 36 columns (was 34) — adds `realized_pnl_pct` and `unrealized_pnl_pct` after `pnl_pct`
  - Recreate `get_pnl_leaderboard` — adds `deposits_on_closed_positions` / `deposits_on_open_positions` aggregates and new pct calculations
  - Recreate `get_vault_leaderboard` — same pattern
  - Recreate `get_pnl_leaderboard_period` — adds `denominator_closed` / `denominator_open` aggregates using `(equity_at_start + period_deposits)`
  - Recreate `get_vault_leaderboard_period` — same pattern + fixes `pnl_pct` denominator from CASE expression to `(equity_at_start + period_deposits)`
- `infrastructure/hasura/migrations/intuition/1771526409000_add_realized_unrealized_pnl_pct/down.sql`
  - Restores 34-column table and all 4 functions at their previous versions
- `infrastructure/hasura/metadata/actions.graphql`
  - Adds `realized_pnl_pct: String!` and `unrealized_pnl_pct: String!` to `PnlLeaderboardEntryOutput`

### Impact

- **Scope:** `pnl_leaderboard_entry` table (composite type), all 4 leaderboard functions, Hasura action GraphQL type.
- **Breaking:** No. Adding fields to GraphQL types is backward-compatible — existing queries are unaffected. New fields are only returned when explicitly requested.
- **Deployment order:** Migration first, then chart-api, then Hasura metadata apply. Old chart-api works against new DB (extra columns ignored by `sqlx::FromRow` name-based mapping).
- **Behavioral change:** `get_vault_leaderboard_period` `pnl_pct` values will change due to the denominator fix. Accounts with both pre-existing equity and in-period deposits will see corrected (generally lower) ROI values.
