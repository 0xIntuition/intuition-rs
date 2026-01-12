# Chart-API Analysis

## 1. Overall Architecture and Structure

The **chart-api** is a high-performance REST API built with **Axum** that serves share price chart data for the Intuition protocol. It follows a clean, modular architecture:

### Directory Structure
```
chart-api/
├── src/
│   ├── main.rs              # Entry point
│   ├── app.rs               # Application setup & router
│   ├── state.rs             # Shared application state
│   ├── types.rs             # Core type definitions
│   ├── error.rs             # Error handling
│   ├── openapi.rs           # OpenAPI/Swagger configuration
│   ├── endpoints/
│   │   ├── mod.rs
│   │   └── chart_data.rs    # Main chart endpoint handler
│   ├── models/
│   │   ├── mod.rs
│   │   └── chart_data.rs    # Data models for chart responses
│   ├── services/
│   │   ├── mod.rs
│   │   ├── data_fetcher.rs  # PostgreSQL/TimescaleDB queries
│   │   ├── gap_filler.rs    # Time series gap-filling logic
│   │   └── svg_generator.rs # SVG chart generation
│   └── cache/
│       ├── mod.rs
│       └── redis.rs         # Redis caching layer
```

### Architectural Flow
```
Client Request → Axum Router → Redis Cache Check →
PostgreSQL/TimescaleDB Query → Gap Filling →
Format (JSON/SVG) → Cache & Return
```

---

## 2. Main Dependencies and Their Purposes

### Core Framework Dependencies
| Dependency | Version | Purpose |
|------------|---------|---------|
| axum | 0.8.4 | Web framework for building the REST API |
| tokio | 1.20.1 | Async runtime |
| tower-http | 0.6.2 | CORS middleware |

### Database & Caching
| Dependency | Version | Purpose |
|------------|---------|---------|
| sqlx | 0.8.6 | PostgreSQL database access with compile-time query checking |
| redis | 0.32.0 | Redis caching with async support |

### Data Handling
| Dependency | Purpose |
|------------|---------|
| alloy (workspace) | Ethereum types (U256, FixedBytes) for blockchain data |
| chrono (workspace) | DateTime manipulation |
| serde / serde_json (workspace) | Serialization/deserialization |

### Visualization
| Dependency | Version | Purpose |
|------------|---------|---------|
| svg | 0.18.0 | Lightweight SVG generation (no runtime dependencies) |

### Documentation
| Dependency | Version | Purpose |
|------------|---------|---------|
| utoipa | 5.4.0 | OpenAPI spec generation |
| utoipa-swagger-ui | 9.0.2 | Interactive API documentation |

### Workspace Dependencies
- **models**: Shared data models (U256Wrapper, database entities)
- **shared-utils**: Common utilities (PostgreSQL connection pooling)

---

## 3. API Endpoints and Functionality

### Primary Endpoint
```
GET /api/v1/curves/{curve_id}/terms/{term_id}/data
```

**Path Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| curve_id | String | Numeric identifier for the bonding curve |
| term_id | String | Hex identifier (e.g., `0x1234...abcd`) |

**Query Parameters:**
| Parameter | Required | Description |
|-----------|----------|-------------|
| interval | Yes | Time bucket interval (`1h`, `1d`, `1w`, `1m`) |
| format | Yes | Output format (`json`, `svg`) |
| count | No | Number of data points (defaults based on interval) |
| width | No | SVG width in pixels (default: 800) |
| height | No | SVG height in pixels (default: 400) |
| line_color | No | SVG line color (default: `#3B82F6`) |
| background_color | No | SVG background color (default: transparent) |

**Default Count Values:**
| Interval | Default Count | Coverage |
|----------|---------------|----------|
| Hourly (`1h`) | 24 | Last 24 hours |
| Daily (`1d`) | 30 | Last 30 days |
| Weekly (`1w`) | 12 | Last 12 weeks |
| Monthly (`1m`) | 12 | Last 12 months |

### Auxiliary Endpoints
| Endpoint | Purpose |
|----------|---------|
| `GET /health` | Health check (returns "OK") |
| `GET /swagger-ui/` | Interactive API documentation |
| `GET /api-docs/openapi.json` | OpenAPI specification |

---

## 4. Data Models and Types

### Core Types (`src/types.rs`)

**`Interval` Enum:**
```rust
enum Interval {
    Hourly,   // 1h
    Daily,    // 1d
    Weekly,   // 1w
    Monthly,  // 1m
}
```

**`OutputFormat` Enum:**
```rust
enum OutputFormat {
    Json,
    Svg,
}
```

**`SvgConfig` Struct:**
- Configurable dimensions, colors, line width, padding

### Data Models (`src/models/chart_data.rs`)

**`ChartDataPoint`:**
```rust
struct ChartDataPoint {
    timestamp: DateTime<Utc>,
    share_price: U256Wrapper,  // Serialized as string for precision
}
```

**`ChartResponse` (JSON output):**
```rust
struct ChartResponse {
    term_id: String,
    curve_id: String,
    interval: String,
    count: usize,
    data: Vec<ChartDataPoint>,
}
```

**`AggregateDataPoint` (from TimescaleDB):**
```rust
struct AggregateDataPoint {
    bucket: DateTime<Utc>,
    term_id: String,
    curve_id: U256Wrapper,
    first_share_price: U256Wrapper,
    last_share_price: U256Wrapper,
    difference: U256Wrapper,
    change_count: i64,
}
```

---

## 5. Notable Patterns, Design Decisions, and Areas of Concern

### Design Strengths

#### 1. Intelligent Caching Strategy
- Redis caching with **interval-based TTL** (30s-5min)
- Cache keys include all query parameters to avoid stale data
- Smart cache-aside pattern with automatic population

#### 2. Gap-Filling Algorithm
- Ensures continuous time series even with missing data
- Three-tier fallback strategy:
  1. Use actual data points and fill gaps with last known value
  2. If no data in range, use most recent historical data
  3. Return 404 if no data exists at all

#### 3. TimescaleDB Continuous Aggregates
- Queries pre-computed aggregate views:
  - `share_price_change_stats_hourly`
  - `share_price_change_stats_daily`
  - `share_price_change_stats_weekly`
  - `share_price_change_stats_monthly`

#### 4. Type-Safe Architecture
- Extensive use of `Result<T, ApiError>` for error handling
- Custom `ApiError` type with `thiserror` for clear error messages
- Proper HTTP status code mapping (400, 404, 500)

#### 5. Precision Handling
- U256 values serialized as strings in JSON to avoid JavaScript precision loss

#### 6. Separation of Concerns
- Clean module boundaries: endpoints, services, models, cache
- Services are testable and independent
- Dependency injection via `AppState`

### Design Concerns & Potential Issues

| Issue | Risk Level | Description | Recommendation |
|-------|------------|-------------|----------------|
| SQL Query Construction | LOW | Uses format! for view names (from trusted enum) | Consider `sqlx::query_builder` for full parameterization |
| Gap-Filling Complexity | MEDIUM | Monthly intervals use approximate 30-day duration | Add extensive unit tests for edge cases |
| SVG Precision Loss | LOW | Large U256 values lose precision when converted to f64 | Acceptable for charts, document inline |
| Redis Connection | MEDIUM | No connection health checks or retry logic | Add timeout handling and pool monitoring |
| No LIMIT Clause | MEDIUM | Could return excessive data for malformed requests | Add maximum count validation (e.g., max 1000) |
| Error Context Loss | LOW | Specific errors converted to generic Internal error | Preserve error types for debugging |
| CORS Policy | MEDIUM | Allows all origins | Restrict to known origins in production |
| Missing Input Validation | HIGH | No validation on count, term_id, curve_id | Add validation layer before database queries |
| Test Coverage | MEDIUM | Only 3 test functions across 2 modules | Add comprehensive test suite |

---

## 6. Integration with intuition-rs Project

### Workspace Integration
- Part of a monorepo workspace with 8 other applications
- Shares common dependencies via `[workspace.dependencies]`
- Uses **edition = "2024"** (bleeding-edge Rust edition)

### Internal Dependencies

| Crate | Location | Usage |
|-------|----------|-------|
| models | `apps/models` | U256Wrapper, database entities, error types |
| shared-utils | `apps/shared-utils` | PostgreSQL connection pooling |

### Database Schema Expectations

**Tables:**
- `vault`: Stores term_id/curve_id combinations for validation
- `share_price_change`: Hypertable tracking price changes over time

**Continuous Aggregates:**
```sql
-- Expected aggregate view schema
bucket TIMESTAMP,
term_id TEXT,
curve_id NUMERIC(78, 0),
first_share_price NUMERIC(78, 0),
last_share_price NUMERIC(78, 0),
difference NUMERIC(78, 0),
change_count BIGINT
```

### Deployment Architecture
- Built as part of `ghcr.io/0xintuition/apps:latest` multi-binary image
- Runs as separate container in `docker-compose-apps.yml`
- Exposed on port 3010
- Depends on: PostgreSQL (port 5435), Redis (port 6379)

### Related Applications
| App | Purpose |
|-----|---------|
| consumer | Ingests blockchain events and populates `share_price_change` |
| histocrawler | Backfills historical data |
| rpc-proxy | Provides blockchain RPC access |
| cli/tui | Administrative tools |

---

## 7. Recommendations

### Immediate Actions (High Priority)
1. Add input validation for `count`, `term_id`, `curve_id` parameters
2. Implement max count limit (e.g., 1000) to prevent excessive queries
3. Add connection retry logic for Redis and PostgreSQL
4. Restrict CORS origins in production

### Medium Priority
5. Expand test coverage (target: 80%+ for services and endpoints)
6. Add query result size limits to database queries
7. Implement connection pool monitoring and health checks
8. Add structured logging with request tracing

### Low Priority
9. Improve error context preservation in error mapping
10. Document timezone handling for timestamp truncation
11. Add request rate limiting to prevent abuse

---

## Summary

The **chart-api** is a well-architected, purpose-built microservice for serving time-series chart data from the Intuition protocol.

**Strengths:**
- Excellent separation of concerns with clear module boundaries
- Smart caching strategy with interval-based TTL
- Robust gap-filling logic for continuous time series
- Type-safe design leveraging Rust's strengths
- Good documentation (comprehensive README, OpenAPI spec)

**Areas for Improvement:**
- Enhanced input validation
- Expanded test coverage
- Better error context preservation
- Production-hardened security (CORS, rate limiting)

The application integrates cleanly into the larger `intuition-rs` ecosystem, relying on shared models and utilities while maintaining independence as a deployable service.
