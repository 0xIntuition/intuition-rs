# PnL Leaderboard Functions

Season 2 Leaderboard feature for ranking accounts by PnL metrics, enabling users to discover top performers, track their own ranking, and analyze aggregate market statistics.

## Overview

This migration adds four PostgreSQL functions exposed via Hasura GraphQL:

| Function | Description |
|----------|-------------|
| `get_pnl_leaderboard` | Main ranked leaderboard with filtering, sorting, and pagination |
| `get_account_pnl_rank` | Individual account rank and percentile lookup |
| `get_pnl_leaderboard_stats` | Aggregate statistics (total traders, median PnL, profitability) |
| `get_vault_leaderboard` | Vault-specific leaderboard with `redeemable_assets` calculation |

## Key Features

### Dual Value Format
All monetary fields are returned in two formats:
- `*_raw`: Base-10 integer (wei, 18 decimals) for precise calculations
- `*_formatted`: Human-readable decimal (ETH, max 4 decimal places) for display

### Time Filters
- `24h` - Last 24 hours
- `7d` - Last 7 days
- `30d` - Last 30 days
- `90d` - Last 90 days
- `all_time` - All historical data
- `custom` - Custom date range via `p_start_time` and `p_end_time`

### Sort Options
- `total_pnl` - Total profit/loss (default)
- `pnl_pct` - ROI percentage
- `win_rate` - Percentage of winning positions
- `total_volume` - Total trading volume
- `position_count` - Number of positions
- `newest` - Most recent traders (by first position date)
- `most_improved` - Biggest PnL change in the time period

### Redeemable Assets Calculation
The `get_vault_leaderboard` function calculates the actual redemption value using bonding curve math:

**Linear Curve (curve_id = 1):**
```
rawAssets = shares * totalAssets / totalShares
```

**Offset Progressive Curve (curve_id = 2):**
```
s = totalShares + OFFSET (3e19)
sNext = s - shares
area = (s² - sNext²) / 1e18
rawAssets = area * HALF_SLOPE (5e16) / 1e18
```

**Fees Applied:**
- Protocol fee: 1.25% (always)
- Exit fee: 0.75% (conservatively always applied)

```
redeemable_assets = rawAssets - protocolFee - exitFee
```

## Returned Fields

### Leaderboard Entry Fields

| Field | Description |
|-------|-------------|
| `rank` | Position in the leaderboard |
| `account_id` | Unique account identifier |
| `account_label` | Human-readable account name/label |
| `account_image` | Account avatar/image URL |
| `total_pnl_raw/formatted` | Combined realized + unrealized PnL |
| `realized_pnl_raw/formatted` | PnL from closed positions (shares = 0) |
| `unrealized_pnl_raw/formatted` | PnL from open positions (shares > 0) |
| `pnl_pct` | ROI percentage |
| `pnl_change_raw/formatted` | Change in PnL for the time period |
| `total_position_count` | Total number of positions ever held |
| `active_position_count` | Number of positions currently open |
| `winning_positions` | Count of positions with positive PnL |
| `losing_positions` | Count of positions with negative PnL |
| `win_rate` | Percentage of winning positions |
| `total_deposits_raw/formatted` | Sum of all deposit amounts |
| `total_redemptions_raw/formatted` | Sum of all redemption amounts |
| `total_volume_raw/formatted` | total_deposits + total_redemptions |
| `current_equity_value_raw/formatted` | Current market value of all open positions |
| `best_trade_pnl_raw/formatted` | Highest PnL from a single position |
| `worst_trade_pnl_raw/formatted` | Lowest PnL from a single position |
| `redeemable_assets_raw/formatted` | Actual redemption value after fees (vault leaderboard only) |
| `first_position_at` | Timestamp of first position creation |
| `last_activity_at` | Timestamp of most recent position update |

### Account Rank Fields

| Field | Description |
|-------|-------------|
| `rank` | Account's position in leaderboard |
| `total_accounts` | Total number of accounts in leaderboard |
| `percentile` | Account's percentile (higher = better) |
| `account_id` | Account identifier |
| `account_label` | Account name/label |
| `account_image` | Account avatar URL |
| `total_pnl_raw/formatted` | Account's total PnL |
| `pnl_pct` | Account's ROI percentage |
| `win_rate` | Account's win rate |
| `total_position_count` | Account's total positions |
| `total_volume_raw/formatted` | Account's total trading volume |

### Leaderboard Stats Fields

| Field | Description |
|-------|-------------|
| `total_traders` | Count of accounts in leaderboard |
| `total_pnl_sum_raw/formatted` | Sum of all traders' PnL |
| `avg_pnl_raw/formatted` | Average PnL per trader |
| `median_pnl_raw/formatted` | Median PnL (50th percentile) |
| `total_volume_raw/formatted` | Sum of all trading volume |
| `avg_volume_raw/formatted` | Average volume per trader |
| `profitable_traders` | Count of traders with positive PnL |
| `unprofitable_traders` | Count of traders with non-positive PnL |
| `profitable_pct` | Percentage of profitable traders |

## Query Examples

### 1. Top 10 Traders by Total PnL (All Time)

```graphql
{
  get_pnl_leaderboard(args: {
    p_limit: 10
    p_sort_by: "total_pnl"
    p_sort_order: "DESC"
    p_time_filter: "all_time"
  }) {
    rank
    account_id
    account_label
    total_pnl_formatted
    pnl_pct
    win_rate
  }
}
```

### 2. Top 10 by ROI (Last 7 Days)

```graphql
{
  get_pnl_leaderboard(args: {
    p_limit: 10
    p_time_filter: "7d"
    p_sort_by: "pnl_pct"
  }) {
    rank
    account_id
    account_label
    total_pnl_formatted
    pnl_pct
  }
}
```

### 3. Newest Traders (Last 30 Days)

```graphql
{
  get_pnl_leaderboard(args: {
    p_limit: 10
    p_time_filter: "30d"
    p_sort_by: "newest"
  }) {
    rank
    account_id
    account_label
    first_position_at
    total_pnl_formatted
  }
}
```

### 4. Most Improved Traders (Last 7 Days)

```graphql
{
  get_pnl_leaderboard(args: {
    p_limit: 10
    p_time_filter: "7d"
    p_sort_by: "most_improved"
  }) {
    rank
    account_id
    account_label
    pnl_change_formatted
    total_pnl_formatted
  }
}
```

### 5. Get My Rank

```graphql
{
  get_account_pnl_rank(args: {
    p_account_id: "0x91814fB52546fbFe050556c41Bdb56D9704f37D4"
    p_sort_by: "total_pnl"
    p_time_filter: "all_time"
  }) {
    rank
    percentile
    total_pnl_formatted
    pnl_pct
    win_rate
  }
}
```

### 6. Leaderboard Statistics

```graphql
{
  get_pnl_leaderboard_stats(args: {
    p_time_filter: "30d"
  }) {
    total_traders
    avg_pnl_formatted
    median_pnl_formatted
    profitable_traders
    unprofitable_traders
    profitable_pct
  }
}
```

### 7. Vault Leaderboard with Redeemable Assets (Linear Curve)

```graphql
{
  get_vault_leaderboard(args: {
    p_term_id: "0x7ec36d201c842dc787b45cb5bb753bea4cf849be3908fb1b0a7d067c3c3cc1f5"
    p_curve_id: "1"
    p_limit: 10
    p_sort_by: "total_pnl"
  }) {
    rank
    account_id
    account_label
    total_pnl_formatted
    current_equity_value_formatted
    redeemable_assets_formatted
  }
}
```

### 8. Vault Leaderboard (Offset Progressive Curve)

```graphql
{
  get_vault_leaderboard(args: {
    p_term_id: "0x5fb82a9ddd34419ac698e2b45c75cc1fda458b026c136dfe021342f77c4be4e3"
    p_curve_id: "2"
    p_limit: 10
    p_sort_by: "total_pnl"
  }) {
    rank
    account_id
    account_label
    total_pnl_formatted
    current_equity_value_formatted
    redeemable_assets_formatted
  }
}
```

### 9. Filter by Minimum Volume and Positions

```graphql
{
  get_pnl_leaderboard(args: {
    p_limit: 50
    p_min_positions: 5
    p_min_volume: 10
    p_sort_by: "total_pnl"
  }) {
    rank
    account_id
    account_label
    total_pnl_formatted
    total_position_count
    total_volume_formatted
  }
}
```

### 10. Custom Date Range

```graphql
{
  get_pnl_leaderboard(args: {
    p_limit: 20
    p_time_filter: "custom"
    p_start_time: "2025-01-01T00:00:00Z"
    p_end_time: "2025-01-31T23:59:59Z"
    p_sort_by: "total_pnl"
  }) {
    rank
    account_id
    account_label
    total_pnl_formatted
    pnl_change_formatted
  }
}
```

## Function Parameters

### get_pnl_leaderboard

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `p_limit` | INTEGER | 100 | Number of results (1-10000) |
| `p_offset` | INTEGER | 0 | Pagination offset |
| `p_time_filter` | TEXT | 'all_time' | Time filter preset |
| `p_start_time` | TIMESTAMPTZ | NULL | Custom start time |
| `p_end_time` | TIMESTAMPTZ | NULL | Custom end time |
| `p_sort_by` | TEXT | 'total_pnl' | Sort field |
| `p_sort_order` | TEXT | 'DESC' | Sort direction |
| `p_exclude_protocol_accounts` | BOOLEAN | TRUE | Exclude protocol accounts |
| `p_min_positions` | INTEGER | 1 | Minimum position count |
| `p_min_volume` | NUMERIC | 0 | Minimum volume (ETH) |
| `p_term_id` | TEXT | NULL | Filter to specific vault |

### get_account_pnl_rank

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `p_account_id` | TEXT | required | Account address |
| `p_sort_by` | TEXT | 'total_pnl' | Sort field for ranking |
| `p_time_filter` | TEXT | 'all_time' | Time filter preset |
| `p_term_id` | TEXT | NULL | Filter to specific vault |

### get_pnl_leaderboard_stats

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `p_time_filter` | TEXT | 'all_time' | Time filter preset |
| `p_term_id` | TEXT | NULL | Filter to specific vault |

### get_vault_leaderboard

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `p_term_id` | TEXT | required | Vault term ID |
| `p_curve_id` | NUMERIC | NULL | Optional curve ID filter |
| `p_limit` | INTEGER | 100 | Number of results (1-10000) |
| `p_offset` | INTEGER | 0 | Pagination offset |
| `p_sort_by` | TEXT | 'total_pnl' | Sort field |
| `p_sort_order` | TEXT | 'DESC' | Sort direction |

## Performance Optimizations

- Uses `position_change_daily` continuous aggregate for time-filtered queries
- Early filtering of accounts with activity in time window before full position scan
- New indexes:
  - `idx_position_account_shares` - for account aggregation on active positions
  - `idx_position_vault_account` - composite index for vault joins

## Notes

- `redeemable_assets` is only calculated for `get_vault_leaderboard` (returns NULL for main leaderboard)
- Protocol accounts (`ProtocolVault`, `AtomWallet`) are excluded by default
- Input validation: limits clamped 1-10000, offsets must be non-negative
- Division by zero protection using `NULLIF()` throughout
