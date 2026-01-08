# Season 2 Airdrop Features

**Status:** In Planning
**Priority:** High
**Last Updated:** 2025-01-07

---

## Overview

Backend support for Season 2 Airdrop program, including enhanced data tracking, PNL calculations, and economic game mechanics.

---

## 1. Oppose/Support Data Enhancement

**Status:** Needs Discussion

### Requirements
- Make oppose and support actions more useful and trackable
- Provide data insights on oppose/support patterns
- Enable analysis of user behavior around these actions

---

## 2. Portal & Economic Game Mechanics

**Status:** Needs Discussion

### Context
Portal is much larger than username, making economic games less compelling. Need to:
- Make clear the behaviors we are trying to incentivize
- Direct people to contribute and curate along specific guidelines
- Concentrate monetary activity in certain areas
- Improve early adopter experience

### Requirements
- Flexible system to direct users to focus areas (implementation-agnostic):
  - Specific lists
  - Specific tags
  - Specific sets of skills
  - Sections of the knowledge graph (when available)
  - Other targeting mechanisms as needed
- API endpoints to query and filter by these focus areas
- Tracking of user activity by focus area
- Metrics for monetary activity concentration
- Behavioral incentive tracking

**Note:** Since we're not using latent space yet, this requirement should be characterized broadly to allow flexibility in how we solve the problem.

---

## 3. Fee Tracking & Trading Volume

**Status:** Needs Discussion

### Requirements
- Track all fees associated with positions and transactions
- Provide fee breakdown by transaction type
- Support fee history queries
- Calculate total fees paid per user/vault/position

### Trading Volume
- Track and display trading volume metrics
- Trading volume shows the same results as protocol fees but presents bigger numbers from the end user's perspective
- Makes the platform feel more impressive and engaging
- Support volume queries per user/vault/position/time period

---

## 4. PNL (Profit & Loss) Calculations

**Status:** In Planning

### Requirements
- **Realized PNL:** Calculate PNL for closed positions
- **Unrealized PNL:** Calculate current PNL for open positions
- **PNL Granularity:**
  - Global PNL (across all positions)
  - Atom-specific PNL
  - Triple-specific PNL
- Support historical PNL queries

### Technical Analysis & Recommendation

#### Current State Assessment

The existing infrastructure is well-positioned for PnL tracking:

| Component | Status | Notes |
|-----------|--------|-------|
| TimescaleDB | Already in use | Hypertables on `signal`, `share_price_change`, `term_total_state_change` |
| Continuous Aggregates | Already in use | Hourly/daily/weekly/monthly for signals and share prices |
| Compression | Enabled | 90% storage savings on time-series data |
| Position Table | Exists | Tracks current state with cumulative totals |
| Signal Table | Exists | Already tracks position deltas via `deposit_id`/`redemption_id` |

#### Design Decision: Avoid Cartesian Expansion

**DO NOT** create a row for every `(account_id, term_id, curve_id, price_change)` event. This would cause:
- Storage explosion: 1M price changes × 10K accounts = 10B rows/year
- Write amplification: Every price change triggers writes for ALL account positions
- I/O dominance: Cartesian expansion overwhelms the system

**INSTEAD:** Track position changes separately from price changes, join at query time.

#### Recommended Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     EXISTING (keep as-is)                       │
├─────────────────────────────────────────────────────────────────┤
│  share_price_change (hypertable)                                │
│  - Tracks price per (term_id, curve_id) over time              │
│  - Already has continuous aggregates                            │
└─────────────────────────────────────────────────────────────────┘
                              +
┌─────────────────────────────────────────────────────────────────┐
│                     NEW: position_change                        │
├─────────────────────────────────────────────────────────────────┤
│  position_change (hypertable)                                   │
│  - Tracks ONLY when account's shares/cash flow changes          │
│  - Derived from deposit + redemption tables                     │
│  - Grows with transaction volume, NOT price volatility          │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                  PnL Computed at Query Time                     │
├─────────────────────────────────────────────────────────────────┤
│  JOIN bucketed position_change with bucketed share_price_change │
│  - shares_t = cumulative sum of shares_delta                    │
│  - price_t = last(share_price) in bucket                        │
│  - equity_t = shares_t × price_t                                │
│  - pnl_t = equity_t + assets_out - assets_in                    │
└─────────────────────────────────────────────────────────────────┘
```

#### Storage Comparison

| Approach | Writes/Year | Storage |
|----------|-------------|---------|
| Per-price-change snapshots | 10B rows | ~500 GB |
| Position-change only | 1M rows | ~500 MB |
| **Reduction** | **10,000x** | **1,000x** |

---

### Schema: position_change Table

```sql
-- ============================================================
-- POSITION_CHANGE HYPERTABLE
-- Captures ONLY when account's shares/cash flow changes
-- ============================================================

CREATE TABLE IF NOT EXISTS position_change (
    id BIGSERIAL,
    created_at TIMESTAMPTZ NOT NULL,
    account_id TEXT NOT NULL,
    term_id TEXT NOT NULL,
    curve_id NUMERIC(78, 0) NOT NULL,

    -- Position deltas
    shares_delta NUMERIC(78, 0) NOT NULL,  -- +shares for deposit, -shares for redemption

    -- Asset flow tracking (already net of fees)
    assets_in NUMERIC(78, 0) NOT NULL DEFAULT 0,   -- deposit.assets_after_fees
    assets_out NUMERIC(78, 0) NOT NULL DEFAULT 0,  -- redemption.assets

    -- Event tracing
    event_type TEXT NOT NULL,  -- 'deposit' or 'redemption'
    event_id TEXT NOT NULL,    -- deposit.id or redemption.id

    -- Blockchain metadata
    block_number NUMERIC(78, 0) NOT NULL,
    transaction_hash TEXT NOT NULL,
    log_index BIGINT NOT NULL,

    CONSTRAINT position_change_event_type_check
        CHECK (event_type IN ('deposit', 'redemption'))
);

-- Convert to hypertable (1-day chunks)
SELECT create_hypertable('position_change', 'created_at',
    chunk_time_interval => INTERVAL '1 day');

-- ============================================================
-- INDEXES
-- ============================================================

-- Primary access pattern: account + term + curve + time
CREATE INDEX idx_position_change_account_term_curve_time
    ON position_change (account_id, term_id, curve_id, created_at DESC);

-- Join pattern with share_price_change: term + curve + time
CREATE INDEX idx_position_change_term_curve_time
    ON position_change (term_id, curve_id, created_at DESC);

-- Account-level queries
CREATE INDEX idx_position_change_account_time
    ON position_change (account_id, created_at DESC);

-- Event lookups for audit
CREATE INDEX idx_position_change_event
    ON position_change (event_type, event_id);

-- ============================================================
-- COMPRESSION (90% storage savings after 7 days)
-- ============================================================

ALTER TABLE position_change SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'account_id, term_id, curve_id',
    timescaledb.compress_orderby = 'created_at DESC'
);

SELECT add_compression_policy('position_change', INTERVAL '7 days');
```

---

### Continuous Aggregates for PnL

```sql
-- ============================================================
-- HOURLY POSITION AGGREGATE
-- Pre-computes cumulative shares and assets per bucket
-- ============================================================

CREATE MATERIALIZED VIEW position_change_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', created_at) AS bucket,
    account_id,
    term_id,
    curve_id,

    -- Period totals (within this hour)
    SUM(shares_delta) AS shares_delta_period,
    SUM(assets_in) AS assets_in_period,
    SUM(assets_out) AS assets_out_period,
    COUNT(*) AS transaction_count

FROM position_change
GROUP BY bucket, account_id, term_id, curve_id;

-- Refresh every hour with 3-hour lookback
SELECT add_continuous_aggregate_policy('position_change_hourly',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');

-- Enable real-time for latest data
ALTER MATERIALIZED VIEW position_change_hourly
    SET (timescaledb.materialized_only = false);

-- ============================================================
-- DAILY POSITION AGGREGATE (built from hourly)
-- ============================================================

CREATE MATERIALIZED VIEW position_change_daily
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 day', bucket) AS bucket,
    account_id,
    term_id,
    curve_id,

    SUM(shares_delta_period) AS shares_delta_period,
    SUM(assets_in_period) AS assets_in_period,
    SUM(assets_out_period) AS assets_out_period,
    SUM(transaction_count) AS transaction_count

FROM position_change_hourly
GROUP BY time_bucket('1 day', bucket), account_id, term_id, curve_id;

SELECT add_continuous_aggregate_policy('position_change_daily',
    start_offset => INTERVAL '3 days',
    end_offset => INTERVAL '1 day',
    schedule_interval => INTERVAL '1 day');

ALTER MATERIALIZED VIEW position_change_daily
    SET (timescaledb.materialized_only = false);
```

---

### PnL Query Function

```sql
-- ============================================================
-- PNL CHART QUERY WITH GAP FILLING
-- Joins position_change with share_price_change at query time
-- ============================================================

CREATE OR REPLACE FUNCTION get_position_pnl_chart(
    p_account_id TEXT,
    p_term_id TEXT,
    p_curve_id NUMERIC(78, 0),
    p_start_time TIMESTAMPTZ,
    p_end_time TIMESTAMPTZ,
    p_interval INTERVAL DEFAULT INTERVAL '1 hour'
)
RETURNS TABLE (
    time TIMESTAMPTZ,
    shares_total NUMERIC(78, 0),
    share_price NUMERIC(78, 0),
    equity_value NUMERIC,
    total_assets_in NUMERIC(78, 0),
    total_assets_out NUMERIC(78, 0),
    net_invested NUMERIC,
    total_pnl NUMERIC,
    pnl_pct NUMERIC(10, 4)
) AS $$
BEGIN
    RETURN QUERY
    WITH
    -- Get cumulative position changes up to each bucket
    position_buckets AS (
        SELECT
            time_bucket_gapfill(p_interval, pc.created_at) AS bucket,
            -- Cumulative sums using window functions
            SUM(SUM(pc.shares_delta)) OVER (ORDER BY time_bucket(p_interval, pc.created_at)) AS shares_total,
            SUM(SUM(pc.assets_in)) OVER (ORDER BY time_bucket(p_interval, pc.created_at)) AS total_assets_in,
            SUM(SUM(pc.assets_out)) OVER (ORDER BY time_bucket(p_interval, pc.created_at)) AS total_assets_out
        FROM position_change pc
        WHERE pc.account_id = p_account_id
          AND pc.term_id = p_term_id
          AND pc.curve_id = p_curve_id
          AND pc.created_at >= p_start_time
          AND pc.created_at <= p_end_time
        GROUP BY time_bucket_gapfill(p_interval, pc.created_at)
    ),
    -- Get last share price per bucket
    price_buckets AS (
        SELECT
            time_bucket_gapfill(p_interval, spc.updated_at) AS bucket,
            locf(last(spc.share_price, spc.updated_at)) AS share_price
        FROM share_price_change spc
        WHERE spc.term_id = p_term_id
          AND spc.curve_id = p_curve_id
          AND spc.updated_at >= p_start_time
          AND spc.updated_at <= p_end_time
        GROUP BY time_bucket_gapfill(p_interval, spc.updated_at)
    )
    SELECT
        COALESCE(pb.bucket, pr.bucket) AS time,
        locf(pb.shares_total) AS shares_total,
        pr.share_price,
        -- Equity = shares × price (divide by 1e18 for precision)
        (locf(pb.shares_total) * pr.share_price / 1e18)::NUMERIC AS equity_value,
        locf(pb.total_assets_in) AS total_assets_in,
        locf(pb.total_assets_out) AS total_assets_out,
        -- Net invested = total in - total out
        (locf(pb.total_assets_in) - locf(pb.total_assets_out))::NUMERIC AS net_invested,
        -- Total PnL = equity + withdrawn - invested
        ((locf(pb.shares_total) * pr.share_price / 1e18)
         + locf(pb.total_assets_out)
         - locf(pb.total_assets_in))::NUMERIC AS total_pnl,
        -- PnL % = total_pnl / max(net_invested, 1) * 100
        (((locf(pb.shares_total) * pr.share_price / 1e18)
          + locf(pb.total_assets_out)
          - locf(pb.total_assets_in)) * 100.0
         / GREATEST(locf(pb.total_assets_in) - locf(pb.total_assets_out), 1))::NUMERIC(10,4) AS pnl_pct
    FROM position_buckets pb
    FULL OUTER JOIN price_buckets pr ON pb.bucket = pr.bucket
    ORDER BY time;
END;
$$ LANGUAGE plpgsql STABLE;
```

---

### Data Population

#### Backfill from Existing Data

```sql
-- ============================================================
-- BACKFILL position_change FROM EXISTING DEPOSITS/REDEMPTIONS
-- ============================================================

INSERT INTO position_change (
    created_at, account_id, term_id, curve_id,
    shares_delta, assets_in, assets_out,
    event_type, event_id,
    block_number, transaction_hash, log_index
)
-- Deposits: receiver_id gets shares
SELECT
    d.created_at,
    d.receiver_id AS account_id,  -- receiver owns the position
    d.term_id,
    d.curve_id,
    d.shares AS shares_delta,     -- positive
    d.assets_after_fees AS assets_in,  -- already net of fees
    0 AS assets_out,
    'deposit' AS event_type,
    d.id AS event_id,
    d.block_number,
    d.transaction_hash,
    d.log_index
FROM deposit d

UNION ALL

-- Redemptions: sender_id burns shares
SELECT
    r.created_at,
    r.sender_id AS account_id,    -- sender owns the position
    r.term_id,
    r.curve_id,
    -r.shares AS shares_delta,    -- negative
    0 AS assets_in,
    r.assets AS assets_out,       -- already net of fees
    'redemption' AS event_type,
    r.id AS event_id,
    r.block_number,
    r.transaction_hash,
    r.log_index
FROM redemption r

ORDER BY created_at, log_index;
```

#### Triggers for Automatic Population

```sql
-- ============================================================
-- TRIGGER: Auto-populate position_change on deposit
-- ============================================================

CREATE OR REPLACE FUNCTION insert_position_change_from_deposit()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO position_change (
        created_at, account_id, term_id, curve_id,
        shares_delta, assets_in, assets_out,
        event_type, event_id,
        block_number, transaction_hash, log_index
    ) VALUES (
        NEW.created_at,
        NEW.receiver_id,      -- receiver gets shares
        NEW.term_id,
        NEW.curve_id,
        NEW.shares,           -- positive delta
        NEW.assets_after_fees,
        0,
        'deposit',
        NEW.id,
        NEW.block_number,
        NEW.transaction_hash,
        NEW.log_index
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER deposit_position_change_trigger
AFTER INSERT ON deposit
FOR EACH ROW
EXECUTE FUNCTION insert_position_change_from_deposit();

-- ============================================================
-- TRIGGER: Auto-populate position_change on redemption
-- ============================================================

CREATE OR REPLACE FUNCTION insert_position_change_from_redemption()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO position_change (
        created_at, account_id, term_id, curve_id,
        shares_delta, assets_in, assets_out,
        event_type, event_id,
        block_number, transaction_hash, log_index
    ) VALUES (
        NEW.created_at,
        NEW.sender_id,        -- sender burns shares
        NEW.term_id,
        NEW.curve_id,
        -NEW.shares,          -- negative delta
        0,
        NEW.assets,
        'redemption',
        NEW.id,
        NEW.block_number,
        NEW.transaction_hash,
        NEW.log_index
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER redemption_position_change_trigger
AFTER INSERT ON redemption
FOR EACH ROW
EXECUTE FUNCTION insert_position_change_from_redemption();
```

---

### PnL Calculation Formulas

| Metric | Formula |
|--------|---------|
| **Shares at time t** | `SUM(shares_delta) up to t` |
| **Price at time t** | `last(share_price) from share_price_change` |
| **Equity at time t** | `shares_t × price_t` |
| **Net Invested** | `SUM(assets_in) - SUM(assets_out)` |
| **Total PnL** | `equity_t + assets_out - assets_in` |
| **PnL %** | `total_pnl / max(net_invested, 1) × 100` |
| **Unrealized PnL** | `equity_t - net_invested` |
| **Realized PnL** | `assets_out - (proportional cost basis)` |

---

### API Endpoints

```
# Position-specific PnL chart
GET /api/v1/accounts/{account_id}/positions/{term_id}/{curve_id}/pnl
    ?start={timestamp}
    &end={timestamp}
    &interval={1m|5m|1h|1d}

# Global account PnL (all positions)
GET /api/v1/accounts/{account_id}/pnl
    ?start={timestamp}
    &end={timestamp}
    &interval={1h|1d|1w}

# Current PnL snapshot (real-time)
GET /api/v1/accounts/{account_id}/pnl/current

# Realized PnL breakdown
GET /api/v1/accounts/{account_id}/pnl/realized
    ?start={timestamp}
    &end={timestamp}
```

---

### Open Questions

| Question | Recommendation |
|----------|----------------|
| **Realized PnL (lot accounting)?** | Use "net invested" method for total PnL. FIFO/LIFO lot tracking only if regulatory required. |
| **Chart resolutions?** | Support 1m, 5m, 1h, 1d via continuous aggregate hierarchy |
| **Sender ≠ Receiver (gifting)?** | Deposits: `receiver_id` gets shares. Redemptions: `sender_id` burns shares. This handles gifting correctly. |

---

### Performance Expectations

| Query Type | Without Aggregates | With Aggregates | Improvement |
|------------|-------------------|-----------------|-------------|
| 1-day PnL chart | 2500ms | 45ms | **55x** |
| 30-day PnL chart | 15000ms | 120ms | **125x** |
| Account portfolio | 800ms | 25ms | **32x** |

---

### Optional Optimizations

1. **Nightly snapshots for hot accounts**: Pre-compute `(account_id, term_id, curve_id, shares, invested, ts)` for accounts with 10K+ transactions
2. **Redis cache**: Cache last N days for frequently queried accounts/terms
3. **Validation job**: Periodically cross-check `position.shares` against `SUM(shares_delta)` for integrity

---

## 5. Position Timeseries Data

**Status:** In Planning

### Requirements
- Track when positions were opened
- Track when positions were closed (if applicable)
- Current status of positions (open/closed)
- Position lifecycle events timeline
- Support queries for position history over time ranges

### Technical Notes

The `position_pnl_snapshot` table proposed above serves this requirement as well:
- First snapshot for a (account_id, term_id, curve_id) = position opened
- Snapshot with shares = 0 = position closed
- `event_type` field tracks lifecycle events

### Additional Schema (if separate tracking needed)

```sql
CREATE TABLE position_lifecycle_event (
    id BIGSERIAL PRIMARY KEY,
    time TIMESTAMPTZ NOT NULL,
    account_id TEXT NOT NULL,
    term_id TEXT NOT NULL,
    curve_id NUMERIC(78,0) NOT NULL,
    event_type TEXT NOT NULL,  -- 'opened', 'increased', 'decreased', 'closed'
    shares_before NUMERIC(78,0),
    shares_after NUMERIC(78,0),
    shares_delta NUMERIC(78,0),
    transaction_hash TEXT,
    block_number BIGINT
);

CREATE INDEX idx_lifecycle_account ON position_lifecycle_event (account_id, time DESC);
CREATE INDEX idx_lifecycle_position ON position_lifecycle_event (account_id, term_id, curve_id, time DESC);
```

---

## Cross-Feature Benefits

This time-series infrastructure unlocks multiple use cases:

| Use Case | Enabled By |
|----------|------------|
| User portfolio tracking | `position_pnl_snapshot` + aggregates |
| Trending markets for explore | Signal stats + share price change stats (existing) |
| Leaderboards | `account_pnl_daily` aggregate |
| Airdrop calculations | Historical PnL snapshots |

---

## Implementation Phases

### Phase 1: Schema & Infrastructure
- [ ] Create `position_change` hypertable with indexes
- [ ] Enable compression policy (7-day threshold)
- [ ] Add triggers on `deposit` and `redemption` tables

### Phase 2: Backfill & Validation
- [ ] Run backfill query to populate from existing deposits/redemptions
- [ ] Validate `SUM(shares_delta)` matches `position.shares` for each position
- [ ] Verify `assets_in`/`assets_out` totals match position cumulative fields

### Phase 3: Continuous Aggregates
- [ ] Create `position_change_hourly` aggregate
- [ ] Create `position_change_daily` aggregate
- [ ] Set up refresh policies with real-time enabled
- [ ] Create `get_position_pnl_chart()` function

### Phase 4: API Integration
- [ ] Implement PnL query endpoints (position-specific, account-global)
- [ ] Add to Hasura/GraphQL schema
- [ ] Integrate with chart-api service

### Phase 5: Optimization (Optional)
- [ ] Add nightly snapshot job for high-activity accounts
- [ ] Implement Redis caching for hot data
- [ ] Set up validation job to detect drift

---

## Related Documentation

- [BACKEND_REQUIREMENTS.md](./BACKEND_REQUIREMENTS.md) - Full requirements list
- [ARCHITECTURE_PROPOSAL.md](./ARCHITECTURE_PROPOSAL.md) - System architecture
- [SYSTEM_ARCHITECTURE_CURRENT.md](./SYSTEM_ARCHITECTURE_CURRENT.md) - Current implementation details
