# Projection-Based Time Series Design

## Overview

This document defines the preferred replacement for the current trigger-heavy indexing model.

The goal is not only to add time-series tables. The real goal is to make the indexer safe and predictable under scale:

1. No trigger cascades on hot paths
2. Clear table ownership per projection
3. Idempotent replay from an event log
4. Cheap chart queries from append-only history
5. Cheap leaderboard reads from precomputed state
6. No architecture that requires broad rescans of million-row tables on routine writes

The previous draft had the right direction, but it mixed together two incompatible ideas:

1. Account-scoped projections that want exact per-account PnL in near real time
2. Vault-scoped price updates that can affect very large numbers of holders

At our current scale, exact account-level mark-to-market updates on every `SharePriceChanged` event are too expensive if they imply fanout to all affected positions. Some of our tables, especially `position`, already have millions of rows. The design below explicitly avoids that trap.

---

## Core Decision

We will use **projection-owned read models**, but we will **not** attempt to update every account's unrealized PnL on every price change.

Instead:

1. Vault-level state updates happen immediately on `SharePriceChanged`
2. Position lifecycle state updates happen immediately on `Deposited` and `Redeemed`
3. Leaderboard/account PnL state is refreshed **incrementally in scheduled rebuild batches**
4. Rebuilds operate only on **affected accounts**, not the entire `position` table

This is the main design choice that makes the system viable at million-row scale.

---

## Design Principles

1. **No trigger cascades**. Derived state is computed in projections, not by Postgres triggers.
2. **Single-writer ownership**. A table has one owning projection.
3. **Append-only facts**. Event facts and time series are insert-only.
4. **Mutable state is narrow**. Current-state tables exist, but only where they make queries materially cheaper.
5. **No global rescans on hot writes**. A single deposit or price change must not force scans over all positions.
6. **Idempotent replay**. Every projection can restart from a checkpoint and safely reprocess events.
7. **Finality-first processing**. Projections should consume finalized blocks unless we explicitly choose to support rollback.
8. **Eventual consistency with bounded lag**. Derived tables may lag briefly, but their consistency model must be explicit.

---

## Why The Current System Fails

### Current system

```
Deposit INSERT
  -> trigger: update position
  -> trigger: update vault.position_count
  -> trigger: update triple_vault / triple_term
  -> trigger: update predicate_object / subject_predicate
  -> trigger: insert position_change
  -> trigger: insert signal
  -> trigger: update term totals
  -> trigger: insert term_total_state_change
```

### Problems

1. Lock contention on hot rows (`vault`, triple summary rows, term summary rows)
2. Retry loops around race-prone counts
3. Hidden write paths and hard-to-debug correctness bugs
4. Complex query-time SQL for leaderboard/PnL
5. Poor operational story for replay and failure visibility

---

## Event Store Model

We need one event log with independent projection checkpoints.

### Event envelope

Every decoded blockchain event should be stored once in a canonical envelope:

```sql
event_id           TEXT PRIMARY KEY
event_type         TEXT NOT NULL
block_number       BIGINT NOT NULL
block_timestamp    TIMESTAMPTZ NOT NULL
transaction_hash   TEXT NOT NULL
log_index          INTEGER NOT NULL
payload            JSONB NOT NULL
finalized          BOOLEAN NOT NULL
```

### Routing model

A single event can be consumed by multiple projections. The event store must support:

1. One physical append-only log
2. One checkpoint stream per projection
3. Per-projection shard assignment

This means `Deposited` can be consumed by:

1. `event_log`
2. `account_registry`
3. `position_tracking`
4. `vault_holders_index`
5. `vault_state`
6. `signals_analytics`
7. `protocol_stats`
8. `leaderboard_refresh`

without duplicating the event itself.

### Proposed solution to a key open question

Question: how do account-scoped and vault-scoped projections both see the same event?

Proposed solution: use **one canonical event log plus independent consumer checkpoints**, not separate event tables per shard key.

---

## Finality And Reorg Strategy

### Preferred approach

Process **finalized blocks only** in projection pipelines.

Why:

1. It keeps projection logic much simpler
2. It avoids compensating writes across many derived tables
3. It fits our current priorities better than full rollback support

### If we later need sub-finality reads

Add a separate short-lived unfinalized cache layer. Do not make every projection rollback-aware unless product requirements force it.

### Proposed solution to a key open question

Question: how do we handle reorgs?

Proposed solution: **do not** push reorg complexity into every projection. Consume finalized events only.

---

## Projection Topology

The earlier draft made `SharePriceChanged` drive account-scoped updates directly. That is the main thing we are changing.

### Projection 1: `event_log`

**Purpose**: canonical append-only fact tables

**Consumes**: all events

**Owns**:

1. `event`
2. `deposit`
3. `redemption`
4. `fee_transfer`
5. other raw fact tables as needed

**Write pattern**: append-only

**Idempotency**:

1. `event.event_id` unique
2. fact tables use `event_id` unique
3. natural uniqueness also enforced with `(transaction_hash, log_index, event_type)` where useful

### Projection 2: `account_registry`

**Purpose**: create account rows for any address seen anywhere

**Consumes**:

1. `AtomCreated`
2. `TripleCreated`
3. `Deposited`
4. `Redeemed`
5. `FeesTransferred`

**Owns**:

1. `account`

**Reason for splitting this out**

The previous draft made `core_entities` own `account` but did not let it see deposit/redemption events. That would leave missing account rows for normal users.

### Projection 3: `core_entities`

**Purpose**: immutable term metadata

**Consumes**:

1. `AtomCreated`
2. `TripleCreated`
3. `Initialized`

**Owns**:

1. `atom`
2. `triple`
3. `term`
4. `initialize`

**Important correction**

`term` is a dimension table. Aggregate fields do **not** live here. They live in `term_summary`, owned by `term_aggregates`.

### Projection 4: `vault_state`

**Shard key**: `(term_id, curve_id)`

**Purpose**: current vault state plus vault-level price history

**Consumes**:

1. `Deposited`
2. `Redeemed`
3. `SharePriceChanged`

**Owns**:

1. `vault`
2. `share_price_history`

**Important scope rule**

`vault_state` owns vault totals, but it does **not** attempt to derive unique holder counts from scratch unless it receives explicit holder open/close signals from another projection.

### Projection 5: `position_tracking`

**Shard key**: `(account_id, term_id, curve_id)`

**Purpose**: current position state and append-only position lifecycle facts

**Consumes**:

1. `Deposited`
2. `Redeemed`

**Owns**:

1. `position`
2. `position_change`
3. optional `position_lot` / cost-basis support tables

**Important correction**

`position_tracking` does **not** consume `SharePriceChanged`.

Why:

1. A price update is keyed by vault
2. An account-scoped projection cannot cheaply discover all affected positions
3. Fanout from one vault price change to potentially huge holder sets is exactly what we must avoid on the hot path

### Projection 6: `vault_holders_index`

**Shard key**: `(term_id, curve_id)`

**Purpose**: maintain the active holder list for a vault

**Consumes**:

1. `Deposited`
2. `Redeemed`

**Owns**:

1. `active_vault_position`

Suggested schema:

```sql
CREATE TABLE active_vault_position (
    term_id         TEXT NOT NULL,
    curve_id        NUMERIC NOT NULL,
    account_id      TEXT NOT NULL,
    shares          NUMERIC NOT NULL,
    total_deposits  NUMERIC NOT NULL DEFAULT 0,
    total_redemptions NUMERIC NOT NULL DEFAULT 0,
    opened_at       TIMESTAMPTZ NOT NULL,
    updated_at      TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (term_id, curve_id, account_id)
);

CREATE INDEX idx_avp_account ON active_vault_position (account_id);
```

Rows with `shares = 0` should be deleted from this table.

**Why this table exists**

This table gives us a vault-keyed holder index without scanning the global `position` table. It is the bridge between vault price changes and later incremental account refreshes.

### Projection 7: `term_aggregates`

**Shard key**: `term_id`

**Purpose**: term-level market cap, asset, and relationship summaries

**Consumes**:

1. `TripleCreated`
2. `SharePriceChanged`
3. holder open/close delta messages if needed

**Owns**:

1. `term_summary`
2. `term_market_cap_history`
3. `predicate_object_summary`
4. `subject_predicate_summary`

**Important correction**

If combined triple/counter-triple stats require updating two terms, we should not do ad hoc cross-shard writes.

Preferred solution:

1. Keep per-term canonical stats in the owning term shard
2. Compute combined/counter views in a separate read model or scheduled reducer
3. Do not make one `term_id` shard write another `term_id` row on the hot path

### Projection 8: `signals_analytics`

**Shard key**: `(term_id, curve_id)`

**Purpose**: append-only signal stream and time bucket rollups

**Consumes**:

1. `Deposited`
2. `Redeemed`

**Owns**:

1. `signal`
2. Timescale continuous aggregates on top of `signal`

### Projection 9: `protocol_stats`

**Shard key**: `GLOBAL`

**Purpose**: small global counters and snapshots

**Consumes**:

1. all key protocol events

**Owns**:

1. `stats`
2. `stats_history`

### Projection 10: `leaderboard_refresh`

**Purpose**: precompute account-level PnL and leaderboard rows without per-price-change account fanout

**Consumes directly**:

1. `Deposited`
2. `Redeemed`
3. `SharePriceChanged`

**Owns**:

1. `account_stats`
2. `account_pnl_state`
3. `account_pnl_snapshot`
4. `leaderboard_cache`
5. `dirty_vault`
6. `dirty_account`

**Most important rule**

`leaderboard_refresh` does **not** update all impacted accounts inline for every `SharePriceChanged`.

Instead:

1. On `Deposited` / `Redeemed`, mark the affected account dirty
2. On `SharePriceChanged`, mark the affected vault dirty
3. On each scheduled rebuild, resolve dirty vaults to accounts through `active_vault_position`
4. Recompute only those accounts

This is the critical solution for million-row scale.

---

## Shard Keys

| Projection | Shard Key | Why |
|-----------|-----------|-----|
| `event_log` | none / append-only | write-once facts |
| `account_registry` | `account_id` | account upserts |
| `core_entities` | `term_id` | immutable term metadata |
| `vault_state` | `(term_id, curve_id)` | vault current state |
| `position_tracking` | `(account_id, term_id, curve_id)` | position current state |
| `vault_holders_index` | `(term_id, curve_id)` | vault-to-holder lookup |
| `term_aggregates` | `term_id` | term summaries |
| `signals_analytics` | `(term_id, curve_id)` | signal history |
| `protocol_stats` | `GLOBAL` | singleton state |
| `leaderboard_refresh` | batch job + `account_id` writes | targeted recompute |

---

## Current-State Tables

### `vault`

```sql
CREATE TABLE vault (
    term_id             TEXT NOT NULL,
    curve_id            NUMERIC NOT NULL,
    total_shares        NUMERIC NOT NULL DEFAULT 0,
    current_share_price NUMERIC NOT NULL DEFAULT 0,
    total_assets        NUMERIC NOT NULL DEFAULT 0,
    market_cap          NUMERIC NOT NULL DEFAULT 0,
    holder_count        INTEGER NOT NULL DEFAULT 0,
    created_at          TIMESTAMPTZ NOT NULL,
    updated_at          TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (term_id, curve_id)
);

CREATE INDEX idx_vault_term ON vault (term_id);
CREATE INDEX idx_vault_market_cap ON vault (market_cap DESC);
```

### `position`

```sql
CREATE TABLE position (
    account_id          TEXT NOT NULL,
    term_id             TEXT NOT NULL,
    curve_id            NUMERIC NOT NULL,
    shares              NUMERIC NOT NULL DEFAULT 0,
    total_deposits      NUMERIC NOT NULL DEFAULT 0,
    total_redemptions   NUMERIC NOT NULL DEFAULT 0,
    realized_pnl        NUMERIC NOT NULL DEFAULT 0,
    cost_basis          NUMERIC NOT NULL DEFAULT 0,
    opened_at           TIMESTAMPTZ NOT NULL,
    closed_at           TIMESTAMPTZ,
    updated_at          TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (account_id, term_id, curve_id)
);

CREATE INDEX idx_position_account ON position (account_id);
CREATE INDEX idx_position_active_account ON position (account_id) WHERE shares > 0;
CREATE INDEX idx_position_term_curve ON position (term_id, curve_id);
```

### `term_summary`

```sql
CREATE TABLE term_summary (
    term_id                 TEXT NOT NULL PRIMARY KEY,
    term_type               TEXT NOT NULL,
    total_assets            NUMERIC NOT NULL DEFAULT 0,
    total_market_cap        NUMERIC NOT NULL DEFAULT 0,
    total_holder_count      INTEGER NOT NULL DEFAULT 0,
    updated_at              TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_term_summary_market_cap ON term_summary (total_market_cap DESC);
```

Note: if we need combined triple/counter-triple stats, store them in a separate table produced by an offline reducer or scheduled projection. Do not mix that into hot-path per-term ownership.

---

## Append-Only Fact And History Tables

### `share_price_history`

```sql
CREATE TABLE share_price_history (
    event_id           TEXT NOT NULL PRIMARY KEY,
    term_id            TEXT NOT NULL,
    curve_id           NUMERIC NOT NULL,
    share_price        NUMERIC NOT NULL,
    total_assets       NUMERIC NOT NULL,
    total_shares       NUMERIC NOT NULL,
    market_cap         NUMERIC NOT NULL,
    block_number       BIGINT NOT NULL,
    block_timestamp    TIMESTAMPTZ NOT NULL,
    transaction_hash   TEXT NOT NULL,
    log_index          INTEGER NOT NULL,
    ts                 TIMESTAMPTZ NOT NULL
);
```

### `position_change`

```sql
CREATE TABLE position_change (
    event_id           TEXT NOT NULL PRIMARY KEY,
    account_id         TEXT NOT NULL,
    term_id            TEXT NOT NULL,
    curve_id           NUMERIC NOT NULL,
    event_type         TEXT NOT NULL,
    shares_delta       NUMERIC NOT NULL,
    assets_in          NUMERIC NOT NULL DEFAULT 0,
    assets_out         NUMERIC NOT NULL DEFAULT 0,
    execution_price    NUMERIC,
    block_number       BIGINT NOT NULL,
    block_timestamp    TIMESTAMPTZ NOT NULL,
    transaction_hash   TEXT NOT NULL,
    log_index          INTEGER NOT NULL,
    ts                 TIMESTAMPTZ NOT NULL
);
```

### Important correction

The earlier draft proposed writing `share_price = 0` when a deposit/redemption arrives before a price event. We should not do that if the column is meant for cost basis.

Preferred solution:

1. If the execution price is present in the source event, store it directly
2. If not, store `NULL`, not `0`
3. Cost basis should come from deterministic lot accounting or another explicit formula, not from a fake zero price

---

## Leaderboard And Account PnL Design

This section is the most important part of the design.

### Problem

We want:

1. fast reads for leaderboard pages
2. exact enough account PnL
3. no global rescans of `position`
4. no per-price-change writes to every holder

You cannot get all four if you insist on exact per-event account updates for unrealized PnL. The solution is **incremental scheduled recomputation**.

### Owned tables

#### `account_stats`

Updated on deposit/redemption only.

```sql
CREATE TABLE account_stats (
    account_id              TEXT NOT NULL PRIMARY KEY,
    total_position_count    INTEGER NOT NULL DEFAULT 0,
    active_position_count   INTEGER NOT NULL DEFAULT 0,
    total_deposits          NUMERIC NOT NULL DEFAULT 0,
    total_redemptions       NUMERIC NOT NULL DEFAULT 0,
    total_volume            NUMERIC NOT NULL DEFAULT 0,
    first_position_at       TIMESTAMPTZ,
    last_activity_at        TIMESTAMPTZ,
    updated_at              TIMESTAMPTZ NOT NULL
);
```

#### `account_pnl_state`

Latest materialized account-level values.

```sql
CREATE TABLE account_pnl_state (
    account_id              TEXT NOT NULL PRIMARY KEY,
    total_deposits          NUMERIC NOT NULL DEFAULT 0,
    total_redemptions       NUMERIC NOT NULL DEFAULT 0,
    realized_pnl            NUMERIC NOT NULL DEFAULT 0,
    unrealized_pnl          NUMERIC NOT NULL DEFAULT 0,
    total_pnl               NUMERIC NOT NULL DEFAULT 0,
    current_equity_value    NUMERIC NOT NULL DEFAULT 0,
    winning_positions       INTEGER NOT NULL DEFAULT 0,
    losing_positions        INTEGER NOT NULL DEFAULT 0,
    last_recomputed_at      TIMESTAMPTZ NOT NULL,
    source_watermark        BIGINT NOT NULL
);
```

#### `account_pnl_snapshot`

Hourly snapshots for period leaderboards.

```sql
CREATE TABLE account_pnl_snapshot (
    account_id              TEXT NOT NULL,
    total_pnl               NUMERIC NOT NULL DEFAULT 0,
    realized_pnl            NUMERIC NOT NULL DEFAULT 0,
    unrealized_pnl          NUMERIC NOT NULL DEFAULT 0,
    current_equity_value    NUMERIC NOT NULL DEFAULT 0,
    active_position_count   INTEGER NOT NULL DEFAULT 0,
    winning_positions       INTEGER NOT NULL DEFAULT 0,
    losing_positions        INTEGER NOT NULL DEFAULT 0,
    ts                      TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (account_id, ts)
);
```

#### `leaderboard_cache`

Versioned, not `TRUNCATE`-based.

```sql
CREATE TABLE leaderboard_cache (
    cache_version           BIGINT NOT NULL,
    period                  TEXT NOT NULL,
    sort_key                TEXT NOT NULL,
    rank                    INTEGER NOT NULL,
    account_id              TEXT NOT NULL,
    total_pnl               NUMERIC NOT NULL DEFAULT 0,
    realized_pnl            NUMERIC NOT NULL DEFAULT 0,
    unrealized_pnl          NUMERIC NOT NULL DEFAULT 0,
    pnl_pct                 NUMERIC NOT NULL DEFAULT 0,
    total_volume            NUMERIC NOT NULL DEFAULT 0,
    current_equity_value    NUMERIC NOT NULL DEFAULT 0,
    active_position_count   INTEGER NOT NULL DEFAULT 0,
    winning_positions       INTEGER NOT NULL DEFAULT 0,
    losing_positions        INTEGER NOT NULL DEFAULT 0,
    computed_at             TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (cache_version, period, sort_key, rank)
);

CREATE INDEX idx_lb_lookup
ON leaderboard_cache (period, sort_key, rank, cache_version DESC);
```

#### Dirty-set tables

```sql
CREATE TABLE dirty_account (
    account_id          TEXT PRIMARY KEY,
    reason              TEXT NOT NULL,
    first_marked_at     TIMESTAMPTZ NOT NULL,
    last_marked_at      TIMESTAMPTZ NOT NULL
);

CREATE TABLE dirty_vault (
    term_id             TEXT NOT NULL,
    curve_id            NUMERIC NOT NULL,
    first_marked_at     TIMESTAMPTZ NOT NULL,
    last_marked_at      TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (term_id, curve_id)
);
```

### Refresh algorithm

Every rebuild cycle:

1. Read `dirty_account`
2. Read `dirty_vault`
3. Expand dirty vaults to accounts through `active_vault_position`
4. Union the two account sets
5. Recompute only those accounts by joining:
   - `position` for that account
   - `vault` for latest prices
   - optional lot/cost-basis tables
6. Upsert `account_pnl_state`
7. Write snapshots if the hourly bucket changed
8. Rebuild the leaderboard cache for the affected periods using versioned writes
9. Clear only the processed dirty rows

### Why this works at scale

1. It avoids scanning the full `position` table on each deposit or price change
2. It avoids writing account PnL rows for every holder on each vault price update
3. It turns the expensive work into a controlled batch
4. The batch touches only accounts impacted by recent activity or recent price changes

### Proposed solution to a key open question

Question: how do we keep leaderboards accurate without fanout to all accounts on every price tick?

Proposed solution: maintain `dirty_vault` and `dirty_account` sets, then perform scheduled incremental recomputation through `active_vault_position`.

---

## Position Counts And Holder Counts

This was ambiguous in the previous draft.

### Rule

Counts based on open/closed positions must come from explicit lifecycle transitions, not inferred independently in multiple projections.

### Preferred solution

When `position_tracking` processes a deposit/redemption that changes a position state:

1. `0 -> >0`: emit `PositionOpened`
2. `>0 -> 0`: emit `PositionClosed`

Consumers:

1. `vault_state` increments/decrements `holder_count`
2. `term_aggregates` increments/decrements `total_holder_count`
3. `leaderboard_refresh` updates `account_stats.active_position_count`
4. `protocol_stats` updates global counts if needed

This avoids duplicated inference logic and keeps counts consistent.

---

## Term Aggregates And Triple Relationships

The previous draft tried to combine hot-path term aggregation with cross-term combined triple/counter-triple aggregation.

That should be split.

### Hot-path term summary

Owned by `term_aggregates`, keyed by `term_id`:

1. `total_assets`
2. `total_market_cap`
3. `total_holder_count`

### Relationship summaries

Also owned by `term_aggregates`, but only for rows naturally keyed by the current term's metadata.

### Combined triple/counter views

Preferred solution:

1. Keep a mapping from triple to counter-triple
2. Build combined stats in a separate reducer or scheduled materializer
3. Do not require one term shard to write another term's canonical row

### Proposed solution to a key open question

Question: how do we maintain triple + counter-triple combined stats without violating shard ownership?

Proposed solution: keep canonical per-term rows hot-path only; compute combined rows in a separate reducer.

---

## Charts And Continuous Aggregates

Chart queries should read from append-only history plus Timescale continuous aggregates.

### Preferred read sources

1. `share_price_history` -> `share_price_stats_hourly/daily/weekly/monthly`
2. `position_change` -> `position_change_hourly/daily`
3. `term_market_cap_history` -> term market cap rollups

### Important migration note

The current API still reads `share_price_change` and current leaderboard SQL functions. Migration must either:

1. add compatibility views with old names, or
2. rewrite API queries during cutover

Do not assume table creation alone completes the migration.

---

## API Read Rules

### Fast paths

1. Charts read continuous aggregates
2. Leaderboards read `leaderboard_cache`
3. Account summary reads `account_stats` + `account_pnl_state`
4. Vault page reads `vault`
5. Position history reads `position_change`

### Lag visibility

Expose projection lag or watermark in API/admin endpoints so we know how stale each read model is.

Proposed solution:

Add `projection_checkpoint`:

```sql
projection_name      TEXT PRIMARY KEY
last_event_id        TEXT NOT NULL
last_block_number    BIGINT NOT NULL
last_processed_at    TIMESTAMPTZ NOT NULL
lag_seconds          INTEGER NOT NULL
status               TEXT NOT NULL
last_error           TEXT
```

---

## Open Questions And Proposed Answers

### 1. Should leaderboards be exact in real time?

Proposed answer: no. They should be exact at the last successful rebuild watermark. The API should expose `computed_at`.

### 2. Should custom-period leaderboards remain supported?

Proposed answer: yes, but only from `account_pnl_snapshot` and only as a secondary path. Fixed periods should use `leaderboard_cache`.

### 3. What accounting model should we use for realized PnL?

Proposed answer: start with **average cost basis per position**, unless product explicitly requires FIFO/LIFO. It is simpler, cheaper, and aligns with the incremental state model.

### 4. Where should execution price come from?

Proposed answer: from the event if available; otherwise leave it `NULL` and derive cost basis from deterministic position accounting, not synthetic zero prices.

### 5. How do we create account rows for normal users?

Proposed answer: the dedicated `account_registry` projection should consume all address-bearing events and upsert rows.

### 6. Should we keep foreign keys between projection-owned tables?

Proposed answer: avoid hot-path cross-projection foreign keys. Keep constraints only within a projection where they do not interfere with replay or ordering.

### 7. How do we avoid duplicate fact rows on replay?

Proposed answer: every fact row gets `event_id UNIQUE`; checkpoints are only advanced after the owning transaction commits.

### 8. How do we atomically refresh leaderboard cache?

Proposed answer: use `cache_version` and switch the active version pointer after a full successful write. Do not use plain `TRUNCATE + INSERT`.

### 9. How do we prevent full rescans of `position`?

Proposed answer: use `dirty_account`, `dirty_vault`, and `active_vault_position`; rebuild only affected accounts.

### 10. Should `SharePriceChanged` be consumed by account-scoped projections?

Proposed answer: no. It should mark vaults dirty and update vault state, not directly drive per-account writes.

---

## Required New Tables

This section lists the tables that do not exist today, or that should be treated as new canonical replacements even if similarly named tables already exist.

### 1. Event ingestion and replay

#### `projection_event`

Canonical append-only decoded event log.

```sql
CREATE TABLE projection_event (
    event_id             TEXT PRIMARY KEY,
    event_type           TEXT NOT NULL,
    chain_id             BIGINT NOT NULL,
    contract_address     TEXT NOT NULL,
    block_number         BIGINT NOT NULL,
    block_hash           TEXT NOT NULL,
    block_timestamp      TIMESTAMPTZ NOT NULL,
    transaction_hash     TEXT NOT NULL,
    transaction_index    INTEGER NOT NULL,
    log_index            INTEGER NOT NULL,
    finalized            BOOLEAN NOT NULL DEFAULT TRUE,
    payload              JSONB NOT NULL,
    inserted_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_projection_event_tx_log
ON projection_event (transaction_hash, log_index, event_type);

CREATE INDEX idx_projection_event_block
ON projection_event (block_number, log_index);

CREATE INDEX idx_projection_event_type_block
ON projection_event (event_type, block_number, log_index);
```

#### `projection_checkpoint`

Checkpoint and health table for each projection worker group.

```sql
CREATE TABLE projection_checkpoint (
    projection_name      TEXT NOT NULL,
    shard_id             INTEGER NOT NULL,
    last_event_id        TEXT NOT NULL,
    last_block_number    BIGINT NOT NULL,
    last_processed_at    TIMESTAMPTZ NOT NULL,
    lag_seconds          INTEGER NOT NULL DEFAULT 0,
    status               TEXT NOT NULL,
    last_error           TEXT,
    updated_at           TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (projection_name, shard_id)
);
```

#### `projection_dead_letter`

Failed events that require manual intervention or code fixes.

```sql
CREATE TABLE projection_dead_letter (
    projection_name      TEXT NOT NULL,
    shard_id             INTEGER NOT NULL,
    event_id             TEXT NOT NULL,
    error_class          TEXT NOT NULL,
    error_message        TEXT NOT NULL,
    payload              JSONB NOT NULL,
    first_failed_at      TIMESTAMPTZ NOT NULL,
    last_failed_at       TIMESTAMPTZ NOT NULL,
    retry_count          INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (projection_name, shard_id, event_id)
);
```

### 2. Canonical fact tables

#### `event`

API-facing audit log derived from `projection_event`.

```sql
CREATE TABLE event (
    event_id             TEXT PRIMARY KEY,
    event_type           TEXT NOT NULL,
    block_number         BIGINT NOT NULL,
    transaction_hash     TEXT NOT NULL,
    log_index            INTEGER NOT NULL,
    ts                   TIMESTAMPTZ NOT NULL
);
```

#### `deposit`

```sql
CREATE TABLE deposit (
    event_id             TEXT PRIMARY KEY,
    sender_id            TEXT NOT NULL,
    receiver_id          TEXT NOT NULL,
    term_id              TEXT NOT NULL,
    curve_id             NUMERIC NOT NULL,
    vault_type           TEXT NOT NULL,
    assets               NUMERIC NOT NULL,
    assets_after_fees    NUMERIC NOT NULL,
    shares               NUMERIC NOT NULL,
    total_shares         NUMERIC NOT NULL,
    block_number         BIGINT NOT NULL,
    transaction_hash     TEXT NOT NULL,
    log_index            INTEGER NOT NULL,
    ts                   TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_deposit_receiver_ts ON deposit (receiver_id, ts DESC);
CREATE INDEX idx_deposit_term_curve_ts ON deposit (term_id, curve_id, ts DESC);
```

#### `redemption`

```sql
CREATE TABLE redemption (
    event_id             TEXT PRIMARY KEY,
    sender_id            TEXT NOT NULL,
    receiver_id          TEXT NOT NULL,
    term_id              TEXT NOT NULL,
    curve_id             NUMERIC NOT NULL,
    vault_type           TEXT NOT NULL,
    assets               NUMERIC NOT NULL,
    fees                 NUMERIC NOT NULL,
    shares               NUMERIC NOT NULL,
    total_shares         NUMERIC NOT NULL,
    block_number         BIGINT NOT NULL,
    transaction_hash     TEXT NOT NULL,
    log_index            INTEGER NOT NULL,
    ts                   TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_redemption_receiver_ts ON redemption (receiver_id, ts DESC);
CREATE INDEX idx_redemption_term_curve_ts ON redemption (term_id, curve_id, ts DESC);
```

#### `fee_transfer`

```sql
CREATE TABLE fee_transfer (
    event_id             TEXT PRIMARY KEY,
    sender_id            TEXT NOT NULL,
    receiver_id          TEXT NOT NULL,
    amount               NUMERIC NOT NULL,
    block_number         BIGINT NOT NULL,
    transaction_hash     TEXT NOT NULL,
    log_index            INTEGER NOT NULL,
    ts                   TIMESTAMPTZ NOT NULL
);
```

### 3. Read models and indexes

#### `active_vault_position`

This is a required table, not an optional optimization.

```sql
CREATE TABLE active_vault_position (
    term_id              TEXT NOT NULL,
    curve_id             NUMERIC NOT NULL,
    account_id           TEXT NOT NULL,
    shares               NUMERIC NOT NULL,
    total_deposits       NUMERIC NOT NULL DEFAULT 0,
    total_redemptions    NUMERIC NOT NULL DEFAULT 0,
    opened_at            TIMESTAMPTZ NOT NULL,
    updated_at           TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (term_id, curve_id, account_id)
);

CREATE INDEX idx_avp_account ON active_vault_position (account_id);
CREATE INDEX idx_avp_term_curve ON active_vault_position (term_id, curve_id);
```

#### `account_pnl_state`

Required for fast account summary reads and leaderboard assembly.

```sql
CREATE TABLE account_pnl_state (
    account_id              TEXT NOT NULL PRIMARY KEY,
    total_deposits          NUMERIC NOT NULL DEFAULT 0,
    total_redemptions       NUMERIC NOT NULL DEFAULT 0,
    realized_pnl            NUMERIC NOT NULL DEFAULT 0,
    unrealized_pnl          NUMERIC NOT NULL DEFAULT 0,
    total_pnl               NUMERIC NOT NULL DEFAULT 0,
    current_equity_value    NUMERIC NOT NULL DEFAULT 0,
    winning_positions       INTEGER NOT NULL DEFAULT 0,
    losing_positions        INTEGER NOT NULL DEFAULT 0,
    last_recomputed_at      TIMESTAMPTZ NOT NULL,
    source_watermark        BIGINT NOT NULL
);
```

#### `account_pnl_snapshot`

```sql
CREATE TABLE account_pnl_snapshot (
    account_id              TEXT NOT NULL,
    total_pnl               NUMERIC NOT NULL DEFAULT 0,
    realized_pnl            NUMERIC NOT NULL DEFAULT 0,
    unrealized_pnl          NUMERIC NOT NULL DEFAULT 0,
    current_equity_value    NUMERIC NOT NULL DEFAULT 0,
    active_position_count   INTEGER NOT NULL DEFAULT 0,
    winning_positions       INTEGER NOT NULL DEFAULT 0,
    losing_positions        INTEGER NOT NULL DEFAULT 0,
    ts                      TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (account_id, ts)
);

CREATE INDEX idx_aps_account_ts ON account_pnl_snapshot (account_id, ts DESC);
```

#### `leaderboard_cache_version`

Holds the current active cache version per period/sort key.

```sql
CREATE TABLE leaderboard_cache_version (
    period               TEXT NOT NULL,
    sort_key             TEXT NOT NULL,
    active_version       BIGINT NOT NULL,
    updated_at           TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (period, sort_key)
);
```

#### `dirty_account`

```sql
CREATE TABLE dirty_account (
    account_id          TEXT PRIMARY KEY,
    reason              TEXT NOT NULL,
    first_marked_at     TIMESTAMPTZ NOT NULL,
    last_marked_at      TIMESTAMPTZ NOT NULL
);
```

#### `dirty_vault`

```sql
CREATE TABLE dirty_vault (
    term_id             TEXT NOT NULL,
    curve_id            NUMERIC NOT NULL,
    first_marked_at     TIMESTAMPTZ NOT NULL,
    last_marked_at      TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (term_id, curve_id)
);
```

#### `position_lot`

If we decide average-cost basis is not enough, this is the extension point for exact lot tracking.

```sql
CREATE TABLE position_lot (
    account_id          TEXT NOT NULL,
    term_id             TEXT NOT NULL,
    curve_id            NUMERIC NOT NULL,
    lot_seq             BIGINT NOT NULL,
    acquired_at         TIMESTAMPTZ NOT NULL,
    acquired_event_id   TEXT NOT NULL,
    shares_open         NUMERIC NOT NULL,
    cost_basis          NUMERIC NOT NULL,
    PRIMARY KEY (account_id, term_id, curve_id, lot_seq)
);
```

### 4. Lifecycle event tables

These are optional as physical tables. They can also be internal logical messages. If we want easy auditing and replay, a table is better.

#### `position_lifecycle_event`

```sql
CREATE TABLE position_lifecycle_event (
    lifecycle_event_id   TEXT PRIMARY KEY,
    source_event_id      TEXT NOT NULL,
    lifecycle_type       TEXT NOT NULL,
    account_id           TEXT NOT NULL,
    term_id              TEXT NOT NULL,
    curve_id             NUMERIC NOT NULL,
    ts                   TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_ple_term_curve ON position_lifecycle_event (term_id, curve_id, ts DESC);
CREATE INDEX idx_ple_account ON position_lifecycle_event (account_id, ts DESC);
```

### 5. Hypertables and CAGGs

The following should be created as Timescale hypertables:

1. `projection_event`
2. `share_price_history`
3. `position_change`
4. `account_pnl_snapshot`
5. `term_market_cap_history`
6. `signal`
7. `stats_history`

The following should remain current-state tables, not hypertables:

1. `vault`
2. `position`
3. `active_vault_position`
4. `term_summary`
5. `account_stats`
6. `account_pnl_state`
7. `leaderboard_cache`
8. `leaderboard_cache_version`
9. `dirty_account`
10. `dirty_vault`
11. `projection_checkpoint`
12. `projection_dead_letter`

---

## Rust Projection Interfaces

The projections are going to be written in Rust. We should standardize the interface now so every projection has the same reliability and observability model.

### Design goals

1. Deterministic event handling
2. Explicit idempotency boundaries
3. One transaction per event or per bounded batch
4. Clear checkpoint semantics
5. Easy replay and backfill
6. Per-projection metrics and health

### Canonical event types

We should decode raw events once, then use strongly typed Rust enums in all projections.

```rust
#[derive(Debug, Clone)]
pub struct EventEnvelope {
    pub event_id: String,
    pub chain_id: i64,
    pub block_number: i64,
    pub block_timestamp: chrono::DateTime<chrono::Utc>,
    pub transaction_hash: String,
    pub transaction_index: i32,
    pub log_index: i32,
    pub finalized: bool,
    pub payload: ProjectionEvent,
}

#[derive(Debug, Clone)]
pub enum ProjectionEvent {
    AtomCreated(AtomCreatedData),
    TripleCreated(TripleCreatedData),
    Deposited(DepositedData),
    Redeemed(RedeemedData),
    SharePriceChanged(SharePriceChangedData),
    FeesTransferred(FeesTransferredData),
    Initialized(InitializedData),
    PositionOpened(PositionLifecycleData),
    PositionClosed(PositionLifecycleData),
}
```

### Core traits

```rust
use async_trait::async_trait;

#[async_trait]
pub trait Projection: Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    fn name(&self) -> &'static str;

    fn subscriptions(&self) -> &'static [EventSubscription];

    async fn handle(
        &self,
        ctx: &ProjectionContext,
        event: &EventEnvelope,
    ) -> Result<ProjectionOutcome, Self::Error>;
}

#[async_trait]
pub trait BatchProjection: Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    fn name(&self) -> &'static str;

    async fn run_batch(
        &self,
        ctx: &ProjectionContext,
        trigger: BatchTrigger,
    ) -> Result<BatchOutcome, Self::Error>;
}
```

### Subscription model

```rust
#[derive(Debug, Clone, Copy)]
pub struct EventSubscription {
    pub event_type: ProjectionEventKind,
    pub shard_strategy: ShardStrategy,
}

#[derive(Debug, Clone, Copy)]
pub enum ShardStrategy {
    Global,
    ByAccount,
    ByTerm,
    ByVault,
    ByPosition,
}
```

### Context and stores

```rust
pub struct ProjectionContext {
    pub db: sqlx::PgPool,
    pub checkpoints: Arc<dyn CheckpointStore>,
    pub metrics: Arc<dyn ProjectionMetrics>,
    pub publisher: Arc<dyn DerivedEventPublisher>,
    pub clock: Arc<dyn Clock>,
}

#[async_trait]
pub trait CheckpointStore: Send + Sync {
    async fn load(&self, projection: &str, shard_id: i32) -> anyhow::Result<Option<Checkpoint>>;
    async fn save(
        &self,
        projection: &str,
        shard_id: i32,
        checkpoint: Checkpoint,
    ) -> anyhow::Result<()>;
}

#[async_trait]
pub trait DerivedEventPublisher: Send + Sync {
    async fn publish(&self, events: Vec<EventEnvelope>) -> anyhow::Result<()>;
}
```

### Projection outcome

```rust
#[derive(Debug, Default)]
pub struct ProjectionOutcome {
    pub rows_written: u64,
    pub derived_events: Vec<EventEnvelope>,
    pub mark_checkpoint: bool,
}

#[derive(Debug, Default)]
pub struct BatchOutcome {
    pub rows_written: u64,
    pub accounts_recomputed: u64,
    pub mark_checkpoint: bool,
}
```

### Required runtime behavior

For correctness, every event-driven projection must follow this sequence:

1. Begin SQL transaction
2. Verify idempotency guard for `event_id`
3. Apply all table changes owned by that projection
4. Persist derived lifecycle events if any
5. Persist checkpoint in the same logical unit of work where possible
6. Commit
7. Emit metrics

If a projection crashes before commit, the event must be safely replayable.

### Idempotency pattern

Every projection should maintain a per-projection processed-event guard.

```sql
CREATE TABLE projection_processed_event (
    projection_name      TEXT NOT NULL,
    shard_id             INTEGER NOT NULL,
    event_id             TEXT NOT NULL,
    processed_at         TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (projection_name, shard_id, event_id)
);
```

Recommended flow in Rust:

1. `INSERT ... ON CONFLICT DO NOTHING`
2. if zero rows inserted, treat event as already processed
3. otherwise continue

This is cheap, explicit, and replay-safe.

### Projection categories

#### Event-driven projections

Use the `Projection` trait:

1. `event_log`
2. `account_registry`
3. `core_entities`
4. `vault_state`
5. `position_tracking`
6. `vault_holders_index`
7. `term_aggregates`
8. `signals_analytics`
9. `protocol_stats`

#### Batch projections

Use the `BatchProjection` trait:

1. `leaderboard_refresh`
2. optional combined triple/counter reducers
3. optional backfill/reconciliation jobs

### Example interfaces per projection

```rust
pub struct VaultStateProjection;
pub struct PositionTrackingProjection;
pub struct VaultHoldersIndexProjection;
pub struct LeaderboardRefreshProjection;
```

Suggested subscription mapping:

1. `VaultStateProjection`: `Deposited`, `Redeemed`, `SharePriceChanged` with `ShardStrategy::ByVault`
2. `PositionTrackingProjection`: `Deposited`, `Redeemed` with `ShardStrategy::ByPosition`
3. `VaultHoldersIndexProjection`: `Deposited`, `Redeemed` with `ShardStrategy::ByVault`
4. `TermAggregatesProjection`: `TripleCreated`, `SharePriceChanged`, `PositionOpened`, `PositionClosed` with `ShardStrategy::ByTerm`
5. `LeaderboardRefreshProjection`: scheduled batch plus dirty-set resolution

### Worker model

Recommended runtime topology:

1. one worker pool per event-driven projection
2. shard-affine workers for stateful projections
3. one or more dedicated batch workers for leaderboard rebuilds
4. bounded queues between ingest and projection workers

Each worker should expose:

1. current shard assignment
2. current checkpoint
3. lag in blocks and seconds
4. events/sec
5. db latency
6. error rate

### Backfill interface

Backfill is a first-class requirement. We should not treat replay as an emergency-only workflow.

```rust
#[async_trait]
pub trait BackfillableProjection: Projection {
    async fn reset_shard(
        &self,
        ctx: &ProjectionContext,
        shard_id: i32,
        from_block: Option<i64>,
    ) -> anyhow::Result<()>;
}
```

### Testing interface

Every projection should be testable against an ordered event sequence and expected table state.

```rust
pub struct ProjectionTestCase {
    pub name: &'static str,
    pub seed_sql: Vec<&'static str>,
    pub events: Vec<EventEnvelope>,
    pub assertions: Vec<TableAssertion>,
}
```

The minimum bar:

1. idempotency test
2. out-of-order replay test where relevant
3. batch recompute test
4. partial failure test
5. backfill parity test

---

## Performance And Reliability Standards

If we want to do this correctly, the projection runtime needs explicit engineering standards. These are not optional nice-to-haves.

### Database standards

1. Every hot-path write query must use the narrowest possible key lookup
2. No trigger-based derived writes
3. No unbounded scans in event handlers
4. All fact tables must have explicit idempotency keys
5. Current-state tables must have covering indexes for primary read patterns
6. Hypertables must use chunk intervals validated against real write volume
7. Compression and retention policies must be declared per hypertable

### Rust runtime standards

1. Use strongly typed decoded event structs, not ad hoc `serde_json::Value` handling in business logic
2. Use bounded concurrency per projection
3. Preserve shard affinity for stateful event-driven projections
4. Use structured logs and metrics from day one
5. Fail the projection loudly on invariant violations
6. Retry only transient failures; do not spin on logical bugs

### Reliability standards

1. Every projection must have:
   - checkpoint persistence
   - dead-letter support
   - idempotency guards
   - lag reporting
   - replay support
2. The API must be able to surface projection watermark and staleness
3. Dual-write migrations must have measurable parity gates
4. Backfill must be resumable
5. Batch rebuild jobs must be chunked and cancellable

### Performance standards

Target standards for the new system:

1. Event-driven projections should process events in O(1) or O(log N) DB work relative to global table size
2. `SharePriceChanged` handling must never require scanning all holders inline
3. Leaderboard read endpoints should be index-range scans over precomputed rows
4. Account summary reads should touch a bounded number of current-state rows
5. Backfill throughput should scale horizontally by shard

### Operational standards

We should be able to answer all of these in production:

1. Which projection is behind?
2. Which shard is failing?
3. Which event last failed?
4. How stale is leaderboard data?
5. Can we replay one shard without replaying all shards?
6. Can we cut over a read path while the old path still exists?

If the answer is no for any of those, the system is not production-ready.

---

## Recommended Rust Package Layout

The projection system should be implemented as a dedicated runtime library plus projection-specific modules. We should not scatter this logic across unrelated services.

### Recommended workspace structure

```text
apps/
  projection-runtime/
    Cargo.toml
    src/
      lib.rs
      app.rs
      config.rs
      error.rs
      metrics.rs
      clock.rs
      worker.rs
      batch_worker.rs
      checkpoint/
        mod.rs
        postgres.rs
      event_store/
        mod.rs
        postgres.rs
      publisher/
        mod.rs
        in_process.rs
      db/
        mod.rs
        transaction.rs
      projections/
        mod.rs
        event_log.rs
        account_registry.rs
        core_entities.rs
        vault_state.rs
        position_tracking.rs
        vault_holders_index.rs
        term_aggregates.rs
        signals_analytics.rs
        protocol_stats.rs
        leaderboard_refresh.rs
      types/
        mod.rs
        envelope.rs
        event.rs
        ids.rs
      repos/
        mod.rs
        account_repo.rs
        event_repo.rs
        vault_repo.rs
        position_repo.rs
        leaderboard_repo.rs
        dirty_repo.rs
        checkpoint_repo.rs
      sql/
        ...
  projection-runner/
    Cargo.toml
    src/
      main.rs
```

### Responsibility split

#### `apps/projection-runtime`

Library crate that owns:

1. core traits
2. event envelope types
3. worker runtime
4. checkpointing
5. shared repos
6. projection implementations
7. batch rebuild jobs

#### `apps/projection-runner`

Thin binary crate that owns:

1. config loading
2. dependency wiring
3. selecting enabled projections
4. process startup
5. graceful shutdown

This split keeps the runtime testable and makes it easy to run projections in different deployment shapes later.

### Internal module responsibilities

#### `config.rs`

Defines:

1. database URLs
2. shard counts per projection
3. worker concurrency
4. batch cadence
5. replay/backfill controls
6. feature flags for dual-write rollout

#### `worker.rs`

Owns the event-driven execution loop:

1. load checkpoint
2. pull next event slice from event store
3. route events to the correct shard worker
4. run handler inside a transaction
5. persist checkpoint
6. emit metrics

#### `batch_worker.rs`

Owns:

1. scheduled batch triggering
2. dirty-set pagination
3. bounded account recompute loops
4. cache version activation

#### `event_store/postgres.rs`

Owns:

1. loading `projection_event`
2. filtering by event type
3. ordered reads by `(block_number, transaction_index, log_index)`
4. shard partitioning

#### `repos/*`

These should contain the projection-specific SQL boundary. We should avoid writing large ad hoc SQL directly inside handler implementations.

Rule:

1. projections own orchestration and invariants
2. repos own SQL

### Optional future split

If the codebase grows, we can split projections into separate crates, but not before we have one stable runtime abstraction.

---

## Projection Implementation Map

This section describes what each Rust projection should implement and what its handler contract looks like.

### 1. `EventLogProjection`

**Trait**: `Projection`

**Subscriptions**:

1. all decoded contract events

**Responsibilities**:

1. write `event`
2. write `deposit`
3. write `redemption`
4. write `fee_transfer`
5. guarantee append-only idempotent facts

**Suggested repos**:

1. `event_repo`
2. `deposit_repo`
3. `redemption_repo`
4. `fee_transfer_repo`

**Key invariants**:

1. one fact row per `event_id`
2. block ordering preserved in stored metadata

### 2. `AccountRegistryProjection`

**Trait**: `Projection`

**Subscriptions**:

1. `AtomCreated`
2. `TripleCreated`
3. `Deposited`
4. `Redeemed`
5. `FeesTransferred`

**Responsibilities**:

1. upsert sender/receiver/creator/wallet addresses into `account`
2. never fail on duplicate addresses

**Suggested repo**:

1. `account_repo`

**Key invariants**:

1. account creation is idempotent
2. no dependency on metadata resolution being available

### 3. `CoreEntitiesProjection`

**Trait**: `Projection`

**Subscriptions**:

1. `AtomCreated`
2. `TripleCreated`
3. `Initialized`

**Responsibilities**:

1. insert `atom`
2. insert `triple`
3. insert `term`
4. insert `initialize`

**Suggested repos**:

1. `atom_repo`
2. `triple_repo`
3. `term_repo`

**Key invariants**:

1. immutable entities are inserted once
2. duplicate replays are ignored safely

### 4. `VaultStateProjection`

**Trait**: `Projection`

**Subscriptions**:

1. `Deposited`
2. `Redeemed`
3. `SharePriceChanged`
4. `PositionOpened`
5. `PositionClosed`

**Shard strategy**: `ByVault`

**Responsibilities**:

1. upsert current `vault`
2. append `share_price_history`
3. apply holder count deltas from lifecycle events

**Suggested repos**:

1. `vault_repo`
2. `share_price_history_repo`

**Key invariants**:

1. no broad joins
2. no holder inference from global tables
3. holder counts change only from lifecycle deltas

### 5. `PositionTrackingProjection`

**Trait**: `Projection`

**Subscriptions**:

1. `Deposited`
2. `Redeemed`

**Shard strategy**: `ByPosition`

**Responsibilities**:

1. update `position`
2. append `position_change`
3. optionally update `position_lot`
4. emit `PositionOpened` or `PositionClosed` when shares cross zero

**Suggested repos**:

1. `position_repo`
2. `position_change_repo`
3. `position_lot_repo`

**Key invariants**:

1. shares can never go negative
2. cost basis updates are deterministic
3. lifecycle events are emitted only on state transitions

### 6. `VaultHoldersIndexProjection`

**Trait**: `Projection`

**Subscriptions**:

1. `Deposited`
2. `Redeemed`

**Shard strategy**: `ByVault`

**Responsibilities**:

1. maintain `active_vault_position`
2. keep only open positions in the table
3. support fast vault-to-account expansion for dirty vaults

**Suggested repo**:

1. `active_vault_position_repo`

**Key invariants**:

1. rows with zero shares are removed
2. no scan of global `position`

### 7. `TermAggregatesProjection`

**Trait**: `Projection`

**Subscriptions**:

1. `TripleCreated`
2. `SharePriceChanged`
3. `PositionOpened`
4. `PositionClosed`

**Shard strategy**: `ByTerm`

**Responsibilities**:

1. update `term_summary`
2. append `term_market_cap_history`
3. update `predicate_object_summary`
4. update `subject_predicate_summary`

**Suggested repos**:

1. `term_summary_repo`
2. `predicate_object_repo`
3. `subject_predicate_repo`

**Key invariants**:

1. canonical per-term writes only
2. no cross-term hot-path mutations

### 8. `SignalsAnalyticsProjection`

**Trait**: `Projection`

**Subscriptions**:

1. `Deposited`
2. `Redeemed`

**Shard strategy**: `ByVault`

**Responsibilities**:

1. append `signal`
2. rely on Timescale CAGGs for rollups

**Suggested repo**:

1. `signal_repo`

**Key invariants**:

1. signal writes are append-only
2. signed volume semantics are consistent

### 9. `ProtocolStatsProjection`

**Trait**: `Projection`

**Subscriptions**:

1. protocol-wide events
2. `PositionOpened`
3. `PositionClosed`

**Shard strategy**: `Global`

**Responsibilities**:

1. update `stats`
2. append `stats_history`

**Suggested repo**:

1. `stats_repo`

**Key invariants**:

1. singleton updates stay cheap
2. all counters are derived from explicit events

### 10. `LeaderboardRefreshProjection`

**Trait**: `BatchProjection`

**Triggers**:

1. fixed schedule
2. manual admin trigger
3. optional high-watermark trigger

**Responsibilities**:

1. read `dirty_account`
2. read `dirty_vault`
3. expand dirty vaults through `active_vault_position`
4. recompute impacted accounts
5. upsert `account_pnl_state`
6. write `account_pnl_snapshot`
7. write new `leaderboard_cache` versions
8. switch `leaderboard_cache_version`

**Suggested repos**:

1. `dirty_repo`
2. `account_pnl_repo`
3. `leaderboard_repo`
4. `active_vault_position_repo`

**Key invariants**:

1. batch recompute is paginated
2. cache activation happens only after full successful write
3. dirty rows are cleared only after success

### Projection registry

The runtime should build projections through a registry, not by hardcoding startup logic in `main.rs`.

```rust
pub fn register_projections() -> Vec<Box<dyn RegisteredProjection>> {
    vec![
        Box::new(EventLogProjection::new()),
        Box::new(AccountRegistryProjection::new()),
        Box::new(CoreEntitiesProjection::new()),
        Box::new(VaultStateProjection::new()),
        Box::new(PositionTrackingProjection::new()),
        Box::new(VaultHoldersIndexProjection::new()),
        Box::new(TermAggregatesProjection::new()),
        Box::new(SignalsAnalyticsProjection::new()),
        Box::new(ProtocolStatsProjection::new()),
        Box::new(LeaderboardRefreshProjection::new()),
    ]
}
```

### Recommended repository boundaries

At minimum, define repository modules for:

1. `account_repo`
2. `atom_repo`
3. `triple_repo`
4. `term_repo`
5. `vault_repo`
6. `position_repo`
7. `position_change_repo`
8. `active_vault_position_repo`
9. `term_summary_repo`
10. `signal_repo`
11. `stats_repo`
12. `dirty_repo`
13. `leaderboard_repo`
14. `checkpoint_repo`

These boundaries matter because query tuning will be unavoidable. We want SQL isolation without losing type safety or projection-level invariants.

---

## Migration Plan

### Phase 1: append-only facts first

1. Add canonical event envelope
2. Add `event_log`
3. Add `share_price_history` and compatibility views if needed
4. Add projection checkpoints

### Phase 2: replace hot trigger paths

1. Add `position_tracking`
2. Add `vault_state`
3. Add `vault_holders_index`
4. Remove trigger-owned writes from those paths

### Phase 3: add aggregate read models

1. Add `term_aggregates`
2. Add `signals_analytics`
3. Add `protocol_stats`

### Phase 4: add incremental leaderboard system

1. Add `dirty_account`
2. Add `dirty_vault`
3. Add `account_pnl_state`
4. Add `account_pnl_snapshot`
5. Add versioned `leaderboard_cache`

### Phase 5: cut API reads over

1. Charts move to new history/CAGGs
2. Account views move to `account_stats` + `account_pnl_state`
3. Leaderboards move to `leaderboard_cache`

### Phase 6: remove old trigger and SQL-function dependencies

1. Retire old cascade triggers
2. Retire expensive leaderboard SQL functions
3. Keep compatibility views temporarily only if needed for rollout safety

---

## Validation Strategy

During dual-write:

1. Compare fact row counts by event type
2. Compare vault state per `(term_id, curve_id)`
3. Compare active position counts per account and per vault
4. Compare term market cap summaries
5. Compare leaderboard outputs for fixed periods within an agreed tolerance

### Proposed solution

Define cutover gates explicitly:

1. zero duplicate fact rows
2. zero checkpoint gaps
3. vault totals match
4. position shares match
5. leaderboard top-N parity acceptable

---

## Summary

The critical architectural decision is this:

**Do not try to push `SharePriceChanged` directly into account-scoped PnL updates.**

That model breaks down once `position` is large.

The scalable version is:

1. update vaults immediately
2. update positions immediately
3. maintain a vault->holder index
4. mark vaults and accounts dirty
5. recompute only affected accounts in scheduled leaderboard/PnL refreshes

This preserves the benefits of projections while respecting the actual scale of the data.
