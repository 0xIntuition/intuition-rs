# Chart API

A high-performance Axum-based REST API for serving share price chart data with Redis caching, automatic gap-filling, and SVG chart generation.

## Features

- **Multiple Output Formats**: JSON data or SVG line charts
- **Automatic Gap-Filling**: Returns continuous time series even when data points are missing
- **Redis Caching**: Intelligent caching with interval-based TTL
- **TimescaleDB Integration**: Leverages continuous aggregates for efficient queries
- **OpenAPI Documentation**: Swagger UI for interactive API exploration
- **Configurable SVG Charts**: Customize dimensions, colors, and styling

## API Endpoint

```
GET /api/v1/curve/{curve_id}/term/{term_id}/data
```

### Path Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `curve_id` | string | The curve identifier (numeric, stored as string for precision) |
| `term_id` | string | The term identifier (hex string, e.g., `0x...`) |

### Query Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `interval` | string | Yes | - | Time interval: `1h`, `1d`, `1w`, `1m` |
| `format` | string | Yes | - | Output format: `json` or `svg` |
| `start` | string | Yes | - | Range start timestamp (unix seconds, unix milliseconds, or RFC3339) |
| `end` | string | Yes | - | Range end timestamp (unix seconds, unix milliseconds, or RFC3339) |
| `width` | integer | No | `800` | SVG width in pixels |
| `height` | integer | No | `400` | SVG height in pixels |
| `line_color` | string | No | `#3B82F6` | SVG line color (hex) |
| `background_color` | string | No | transparent | SVG background color (hex) |

### Range Behavior

Data points are derived from the aligned interval buckets within `[start, end)`. For example:
- `start=2026-01-01T00:00:00Z`, `end=2026-02-01T00:00:00Z`, `interval=1w` → 4 points
- `start=2026-01-01T00:00:00Z`, `end=2026-02-01T00:00:00Z`, `interval=1d` → 31 points

## PnL Endpoints

```
GET /api/v1/accounts/{account_id}/pnl
GET /api/v1/accounts/{account_id}/pnl/current
GET /api/v1/accounts/{account_id}/pnl/realized
GET /api/v1/accounts/{account_id}/positions/{term_id}/{curve_id}/pnl
```

### PnL Parameters

- `account_id` must be a 0x-prefixed 40-character hex address.
- `term_id` is a 0x-prefixed hex string.
- `curve_id` is a numeric string.

### PnL Methodology

- `equity_value = shares_total * share_price / 1e18`
- `net_invested = total_assets_in - total_assets_out`
- `total_pnl = equity_value + total_assets_out - total_assets_in`
- `pnl_pct = (total_pnl / net_invested) * 100` when `net_invested > 0`, otherwise `0`
- `unrealized_pnl = equity_value - net_invested`

### PnL Performance Notes

- Account-level PnL aggregates per-position series and limits concurrency to avoid saturating the DB.
- Large accounts may still need tighter ranges or longer intervals for responsive queries.

## Response Formats

### JSON Response

```json
{
  "term_id": "0x1234...abcd",
  "curve_id": "1",
  "interval": "1d",
  "count": 30,
  "data": [
    {
      "timestamp": "2024-01-01T00:00:00Z",
      "share_price": "1000000000000000000"
    },
    {
      "timestamp": "2024-01-02T00:00:00Z",
      "share_price": "1050000000000000000"
    }
  ]
}
```

> **Note**: `share_price` is serialized as a string to preserve precision for large numbers (U256).

### SVG Response

Returns an SVG line chart with:
- Responsive viewBox
- Subtle grid lines
- Configurable line color and dimensions
- "No data available" message when empty

Content-Type: `image/svg+xml`

## Gap-Filling Behavior

The API ensures continuous time series data:

1. **Data exists for the requested range**: Returns actual data points with gaps filled using the last known value.

2. **No data in requested range but historical data exists**: Uses the most recent historical data point to create a constant line.

3. **No data exists at all**: Returns a `404 Not Found` error.

### Example

If data exists for Day 1 and Day 5:
- Day 1: `1000` (actual)
- Day 2: `1000` (filled from Day 1)
- Day 3: `1000` (filled from Day 1)
- Day 4: `1000` (filled from Day 1)
- Day 5: `1200` (actual)

## Caching

Redis caching with interval-based TTL:

| Interval | Cache TTL |
|----------|-----------|
| `1h` | 30 seconds |
| `1d` | 60 seconds |
| `1w` | 120 seconds |
| `1m` | 300 seconds |

Cache key format: `chart:{graph_type}:{term_id}:{curve_id}:{interval}:{start}:{end}:{format}`

PnL cache keys:

- Position: `pnl:position:{account_id}:{term_id}:{curve_id}:{interval}:{start}:{end}`
- Account: `pnl:account:{account_id}:{interval}:{start}:{end}`

## Migration Notes

PnL endpoints require the Season 2 PnL migrations:

- `infrastructure/hasura/migrations/intuition/1767883119000_season2_pnl`

## Error Responses

| Status Code | Description |
|-------------|-------------|
| `400 Bad Request` | Invalid term_id/curve_id combination, invalid interval, or invalid format |
| `404 Not Found` | No data available for the requested term/curve |
| `500 Internal Server Error` | Database or internal error |

### Error Response Body

```json
{
  "error": "Invalid term_id/curve_id combination: no vault exists"
}
```

## Configuration

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `CHART_API_PORT` | Yes | - | Port to listen on (e.g., `3010`) |
| `DATABASE_URL` | Yes | - | PostgreSQL connection string |
| `REDIS_URL` | Yes | - | Redis connection string |
| `CORS_ALLOWED_ORIGINS` | No | `*` | Comma-separated list of allowed CORS origins |
| `RUST_LOG` | No | `info` | Log level (`debug`, `info`, `warn`, `error`) |

### Example `.env` File

```env
CHART_API_PORT=3010
DATABASE_URL=postgres://postgres:postgres@localhost:5435/storage
REDIS_URL=redis://localhost:6379
RUST_LOG=info
```

## Running Locally

### Prerequisites

- Rust 1.89+
- PostgreSQL with TimescaleDB
- Redis
- The database must have the required tables and continuous aggregates

### Using Cargo Make

```bash
# Start the required infrastructure (DB, Redis, etc.)
cargo make start-docker-shared

# Run the chart-api locally
cargo make chart-api-local
```

### Using Cargo Directly

```bash
# Set environment variables
export CHART_API_PORT=3010
export DATABASE_URL=postgres://postgres:postgres@localhost:5435/storage
export REDIS_URL=redis://localhost:6379

# Run the service
cargo run --bin chart-api
```

## Docker Deployment

### Build

```bash
# Build the Docker image
docker build -f apps/chart-api/Dockerfile -t chart-api .

# Or build all apps (includes chart-api)
cargo make build-apps
```

### Run with Docker Compose

The service is configured in `docker/docker-compose-apps.yml`:

```yaml
chart-api:
  container_name: chart-api
  image: ghcr.io/0xintuition/apps:latest
  command: ./chart-api
  environment:
    CHART_API_PORT: '3010'
    DATABASE_URL: 'postgres://postgres:postgres@database:5435/storage'
    REDIS_URL: 'redis://redis:6379'
    RUST_LOG: 'info'
  restart: always
  ports:
    - 3010:3010
```

```bash
# Start all services
cargo make start-docker-apps
```

## API Documentation

Swagger UI is available at:

```
http://localhost:3010/swagger-ui/
```

OpenAPI JSON spec:

```
http://localhost:3010/api-docs/openapi.json
```

## Health Check

```
GET /health
```

Returns `OK` with status `200` if the service is running.

## Examples

### Get Daily JSON Data (Monthly Range)

```bash
curl "http://localhost:3010/api/v1/curves/1/terms/0x1234abcd/data?interval=1d&format=json&start=2026-01-01T00:00:00Z&end=2026-02-01T00:00:00Z"
```

### Get Hourly JSON Data (48-Hour Range)

```bash
curl "http://localhost:3010/api/v1/curves/1/terms/0x1234abcd/data?interval=1h&format=json&start=2026-01-01T00:00:00Z&end=2026-01-03T00:00:00Z"
```

### Get SVG Chart (Default Styling)

```bash
curl "http://localhost:3010/api/v1/curves/1/terms/0x1234abcd/data?interval=1d&format=svg&start=2026-01-01T00:00:00Z&end=2026-02-01T00:00:00Z" > chart.svg
```

### Get Custom SVG Chart

```bash
curl "http://localhost:3010/api/v1/curves/1/terms/0x1234abcd/data?interval=1w&format=svg&start=2026-01-01T00:00:00Z&end=2026-02-01T00:00:00Z&width=1200&height=600&line_color=%23FF5733&background_color=%23FFFFFF" > chart.svg
```

### Embed SVG in HTML

```html
<img src="http://localhost:3010/api/v1/curves/1/terms/0x1234abcd/data?interval=1d&format=svg&start=2026-01-01T00:00:00Z&end=2026-02-01T00:00:00Z" alt="Share Price Chart" />
```

## Database Requirements

The API queries the following TimescaleDB continuous aggregates:

- `share_price_change_stats_hourly`
- `share_price_change_stats_daily`
- `share_price_change_stats_weekly`
- `share_price_change_stats_monthly`

These are automatically maintained by TimescaleDB based on the `share_price_change` hypertable.

### Required Tables

- `vault` - For validating term_id/curve_id combinations
- `share_price_change` - Source hypertable for price data

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Client Request                          │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                          Axum Router                            │
│  GET /api/v1/curves/{curve_id}/terms/{term_id}/data            │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Redis Cache                              │
│  Key: chart:{graph_type}:{term_id}:{curve_id}:{interval}:{start}:{end}:{format}   │
│  TTL: 30s - 300s based on interval                             │
└─────────────────────────────────────────────────────────────────┘
                                │
                         Cache Miss
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                       Data Fetcher                              │
│  Queries TimescaleDB continuous aggregates                      │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Gap Filler                               │
│  Fills missing time buckets with last known values              │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Response Generator                           │
│  JSON serialization or SVG chart generation                     │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Cache & Return                             │
│  Store in Redis, return to client                               │
└─────────────────────────────────────────────────────────────────┘
```

## Performance Considerations

1. **Continuous Aggregates**: Pre-computed by TimescaleDB, ensuring fast queries even for large datasets.

2. **Redis Caching**: Reduces database load for frequently requested charts.

3. **Gap-Filling in Application**: Performed in Rust for maximum flexibility and performance.

4. **SVG Generation**: Lightweight `svg` crate with no runtime dependencies.

## License

See the repository root for license information.
