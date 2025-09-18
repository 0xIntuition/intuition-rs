# Intuition Rust

[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/0xIntuition/intuition-rs)

A comprehensive Rust workspace for blockchain data indexing and processing, featuring a modular architecture with multiple specialized services.

## 🏗️ Architecture

This workspace contains the following core services:

### Core Services
- **`apps/cli`** - Terminal UI client for interacting with the Intuition system
- **`apps/consumer`** - Event processing pipeline using Redis Streams (RAW, DECODED, and RESOLVER consumers)
- **`apps/models`** - Domain models and data structures for the Intuition system

### Infrastructure Services
- **`infrastructure/hasura`** - GraphQL API with database migrations and configuration
- **`apps/image-guard`** - Image processing and validation service
- **`apps/rpc-proxy`** - RPC call proxy with caching for `eth_call` methods

### Supporting Services
- **`apps/histocrawler`** - Historical data crawler
- **`apps/shared-utils`** - Common utilities and shared code
- **`infrastructure/migration-scripts`** - Database migration utilities

## 🚀 Quick Start

### Prerequisites

1. **Install Rust toolchain**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Install required tools**
   ```bash
   # Install cargo-make for build automation
   cargo install --force cargo-make
   
   # Install Hasura CLI
   curl -L https://github.com/hasura/graphql-engine/raw/stable/cli/get.sh | bash
   ```

3. **Install Node.js dependencies** (for integration tests)
   ```bash
   cd integration-tests
   pnpm install
   ```

### Environment Setup

1. **Copy environment template**
   ```bash
   cp .env.sample .env
   ```

2. **Configure required environment variables**

   | Variable | Description | Source |
   |----------|-------------|---------|
   | `OPENAI_API_KEY` | OpenAI API key for AI features | [OpenAI Platform](https://platform.openai.com/api-keys) |
   | `PINATA_GATEWAY_TOKEN` | Pinata gateway token for IPFS | [Pinata Dashboard](https://app.pinata.cloud/developers/gateway-settings) |
   | `PINATA_API_JWT` | Pinata API JWT for IPFS uploads | [Pinata Dashboard](https://app.pinata.cloud/developers/api-keys) |
   | `RPC_URL_MAINNET` | Ethereum mainnet RPC endpoint | [Alchemy Dashboard](https://dashboard.alchemy.com/) |
   | `RPC_URL_BASE` | Base network RPC endpoint | [Alchemy Dashboard](https://dashboard.alchemy.com/apps) |
   | `RPC_URL_LINEA` | Linea network RPC endpoint | [Alchemy Dashboard](https://dashboard.alchemy.com/apps) |

## 🏃‍♂️ Running the System

**Note**: All scripts are located in the `scripts/` directory and should be run from the project root.

### Option 1: Using Published Docker Images (Recommended)

```bash

# Start with local Ethereum node
cargo make start-local
```

### Option 2: Building from Source

```bash
# Build all Docker images from source
cargo make build-docker-images

# Start the system
cargo make start-local
```

### Option 3: Running with Integration Tests

```bash
# Start with tests enabled
cargo make start-local test
```


## 🧪 Testing

### Run All Tests
```bash
cargo nextest run
```

### Run Integration Tests
```bash
cd integration-tests
export VITE_INTUITION_CONTRACT_ADDRESS=0x....
pnpm test src/follow.test.ts
```

### Run Specific Test Suites
```bash
# Test account operations
pnpm test src/create-person.test.ts

# Test vault operations
pnpm test src/vaults.test.ts

# Test AI agents
pnpm test src/ai-agents.test.ts
```

## 🛠️ Development

### CLI Tool
```bash
# Run the CLI to verify latest data
./scripts/cli.sh
```

### Code Quality
```bash
# Format code
cargo make fmt

# Run linter
cargo make clippy

# Run all checks
cargo make check
```

### Database Operations
```bash
# Start services and run migrations
cargo make start-docker-and-migrate

# Manual migration (if needed)
cp .env.sample .env
source .env
```

## 🔧 Local Development Setup

### Using Local Ethereum Node

Add to your `.env` file:
```bash
INTUITION_CONTRACT_ADDRESS=0x04056c43d0498b22f7a0c60d4c3584fb5fa881cc
START_BLOCK=0
```

Create local test data:
```bash
cd integration-tests
npm install
npm run create-predicates
```

### Manual Service Management

```bash
# Start all services
docker-compose -f docker/docker-compose-apps.yml up -d

# Stop all services
./scripts/stop.sh

# View logs
docker-compose -f docker/docker-compose-apps.yml logs -f
```

### Logging and Monitoring

**JSON Logging:**
All consumer services output structured JSON logs with the following fields:
- `timestamp`: ISO 8601 timestamp
- `level`: Log level (INFO, WARN, ERROR, DEBUG)
- `fields.message`: Log message content
- `target`: Module path
- `filename`: Source file name
- `line_number`: Line number in source file
- `threadId`: Thread identifier

**Viewing Logs:**
```bash
# View container logs directly
docker logs decoded_consumer | grep '"level":"INFO"'
docker logs resolver_consumer | grep '"level":"ERROR"'
docker logs ipfs_upload_consumer | grep '"level":"WARN"'
```

## 📁 Project Structure

```
intuition-rs/
├── apps/                 # Custom Rust applications
│   ├── cli/             # Terminal UI client
│   ├── consumer/        # Event processing pipeline (Redis Streams)
│   ├── histocrawler/    # Historical data crawler
│   ├── image-guard/     # Image processing service
│   ├── models/          # Domain models & data structures
│   ├── rpc-proxy/       # RPC proxy with caching
│   └── shared-utils/    # Common utilities
├── infrastructure/      # Infrastructure components
│   ├── hasura/         # GraphQL API & migrations
│   ├── blockscout/     # Blockchain explorer
│   ├── drizzle/        # Database schema management
│   ├── geth/           # Local Ethereum node config
│   ├── indexer-and-cache-migrations/  # Database migrations
│   ├── migration-scripts/  # Migration utilities
│   └── prometheus/     # Monitoring configuration
├── docker/             # Docker configuration
│   ├── docker-compose-apps.yml   # Application services
│   ├── docker-compose-shared.yml # Shared infrastructure
│   └── Dockerfile      # Multi-stage build
├── scripts/            # Shell scripts
│   ├── start.sh        # System startup
│   ├── stop.sh         # System shutdown
│   ├── cli.sh          # CLI runner
│   ├── init-dbs.sh     # Database initialization
├── integration-tests/  # End-to-end tests
└── README.md          # This file
```

## 🔄 Event Processing Pipeline

The system processes blockchain events through multiple stages:

1. **RAW** - Raw event ingestion from blockchain
2. **DECODED** - Event decoding and parsing
3. **RESOLVER** - Data resolution and enrichment

### Supported Contract Versions
- EthMultiVault v1.0
- EthMultiVault v1.5
- Multivault v2.0

## 🚨 Known Issues

- **Base Events Indexing**: To index Base events, uncomment `substreams-sink` and comment `envio-indexer` in `docker-compose.yml`. The optimal process is under investigation.

## 📊 Monitoring and Observability

### Logging

The system includes comprehensive logging capabilities:

**Features:**
- **Structured JSON Logging**: All services output machine-readable logs
- **Container Logs**: Direct access to service logs via Docker
- **Log Filtering**: Easy filtering by log level and service

**Benefits:**
- **Debugging**: Quickly find and analyze issues across services
- **Performance Monitoring**: Track service performance and bottlenecks
- **Audit Trail**: Complete visibility into system operations

**Getting Started:**
1. Start the system: `cargo make start-local`
2. View logs: `docker logs <service_name>`
3. Filter logs: `docker logs <service_name> | grep '"level":"INFO"'`

## 📚 Additional Resources

- [Hasura Documentation](https://hasura.io/docs/)
- [Alchemy Dashboard](https://dashboard.alchemy.com/)
- [Pinata Documentation](https://docs.pinata.cloud/)

## 📄 License

See [LICENSE](LICENSE) file for details.

---

**Note**: This project is under active development. Code and APIs are subject to change.
