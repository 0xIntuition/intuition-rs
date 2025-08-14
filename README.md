# Intuition Rust

A comprehensive Rust workspace for blockchain data indexing and processing, featuring a modular architecture with multiple specialized services.

## 🏗️ Architecture

This workspace contains the following core services:

### Core Services
- **`cli`** - Terminal UI client for interacting with the Intuition system
- **`consumer`** - Event processing pipeline (RAW, DECODED, and RESOLVER consumers)
- **`consumer-api`** - REST API for re-fetching and managing Atoms
- **`models`** - Domain models and data structures for the Intuition system

### Infrastructure Services
- **`hasura`** - GraphQL API with database migrations and configuration
- **`image-guard`** - Image processing and validation service
- **`rpc-proxy`** - RPC call proxy with caching for `eth_call` methods

### Supporting Services
- **`histocrawler`** - Historical data crawler
- **`shared-utils`** - Common utilities and shared code
- **`migration-scripts`** - Database migration utilities

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

### Option 1: Using Published Docker Images (Recommended)

```bash
# Start with Base Sepolia network
./start.sh histo_base_sepolia_1_5

# Start with local Ethereum node
./start.sh histo_local_1_5
```

### Option 2: Building from Source

```bash
# Build all Docker images from source
cargo make build-docker-images

# Start the system
./start.sh histo_base_sepolia_1_5
```

### Option 3: Running with Integration Tests

```bash
# Start with tests enabled
./start.sh histo_local_1_5 test
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
./cli.sh
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
BASE_MAINNET_RPC_URL=http://geth:8545
BASE_SEPOLIA_RPC_URL=http://geth:8545
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
docker-compose -f docker-compose-apps.yml up -d

# Stop all services
./stop.sh

# View logs
docker-compose -f docker-compose-apps.yml logs -f
```

## 📁 Project Structure

```
intuition-rs/
├── cli/                    # Terminal UI client
├── consumer/              # Event processing pipeline
├── consumer-api/          # REST API service
├── hasura/               # GraphQL API & migrations
├── image-guard/          # Image processing service
├── models/               # Domain models & data structures
├── rpc-proxy/            # RPC proxy with caching
├── integration-tests/    # End-to-end tests
├── shared-utils/         # Common utilities
└── docker-compose-*.yml  # Service orchestration
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

## 📚 Additional Resources

- [Hasura Documentation](https://hasura.io/docs/)
- [Alchemy Dashboard](https://dashboard.alchemy.com/)
- [Pinata Documentation](https://docs.pinata.cloud/)

## 📄 License

See [LICENSE](LICENSE) file for details.

---

**Note**: This project is under active development. Code and APIs are subject to change.
