# Health Check API

A lightweight Axum-based API service for health checking Hasura GraphQL instances running in a Kubernetes cluster.

## Overview

This service provides a secured REST API endpoint that queries Hasura GraphQL engines for statistics and health information. It's designed to run in a GCP Kubernetes cluster and can query Hasura instances by environment name.

## Features

- **API Key Authentication**: All requests must include a valid API key in the `x-api-key` header
- **Environment-based Queries**: Query Hasura instances by providing the environment name
- **Kubernetes-native**: Designed to run in Kubernetes with service discovery
- **Contract Address Discovery**: Automatically fetches the contract address from the consumer pod's environment
- **GraphQL Stats**: Retrieves comprehensive statistics from Hasura including:
  - Last processed block number and timestamp
  - Total accounts, atoms, fees, positions, signals, and triples

## API Endpoints

### POST /health-check

Check the health of a Hasura instance and retrieve statistics.

**Request Headers:**
```
x-api-key: your-api-key
Content-Type: application/json
```

**Request Body:**
```json
{
  "environment": "intuition-testnet-b"
}
```

**Fields:**
- `contract_address` (optional, string): Contract address to check (not yet implemented)
- `environment` (optional, string): Environment name to check

**Note:** At least one of `contract_address` or `environment` must be provided.

**Success Response:**
```json
{
  "status": "ok",
  "contract_address": "0x1234567890abcdef...",
  "server_stats": {
    "last_processed_block_number": "12345",
    "last_processed_block_timestamp": "2024-01-20T12:00:00Z",
    "last_updated": "2024-01-20T12:05:00Z",
    "total_accounts": 1000,
    "total_atoms": 5000,
    "total_fees": "100",
    "total_positions": 2500,
    "total_signals": 3000,
    "total_triples": 1500
  }
}
```

**Offline Response (when Hasura is unreachable):**
```json
{
  "status": "offline",
  "contract_address": "0x1234567890abcdef...",
  "server_stats": {
    "error": "Failed to connect to Hasura: ..."
  }
}
```

**Error Responses:**

- `400 Bad Request`: Missing required fields or invalid input
- `401 Unauthorized`: Invalid or missing API key
- `500 Internal Server Error`: Failed to query Hasura or other server errors

## Configuration

### Environment Variables

- `API_KEY` (required): API key for authenticating requests
- `PORT` (optional): Port to listen on (default: 3000)
- `RUST_LOG` (optional): Log level (default: info)

### Example .env file

```bash
API_KEY=your-secret-api-key-here
PORT=3000
RUST_LOG=info
```

## Running Locally

1. Copy the sample environment file:
```bash
cp .env.sample .env
```

2. Edit `.env` and set your API key

3. Run the service:
```bash
cargo run -p health-check
```

The service will be available at `http://localhost:3000`

## Testing

Example curl request:
```bash
curl --request POST \
  --header 'x-api-key: your-api-key' \
  --header 'content-type: application/json' \
  --url 'http://localhost:3000/health-check' \
  --data '{"environment":"intuition-testnet-b"}'
```

## Kubernetes Deployment

### Prerequisites

- A GCP Kubernetes cluster
- `kubectl` configured to access your cluster
- Docker image built and pushed to GCR

### Build and Push Docker Image

```bash
# Build the Docker image
docker build -f apps/health-check/Dockerfile -t gcr.io/YOUR_PROJECT_ID/health-check:latest .

# Push to GCR
docker push gcr.io/YOUR_PROJECT_ID/health-check:latest
```

### Deploy to Kubernetes

1. Update the deployment configuration:
```bash
# Edit infrastructure/health-check/deployment.yaml
# Replace PROJECT_ID with your GCP project ID
# Update the API key in the Secret
```

2. Apply the configuration:
```bash
kubectl apply -f infrastructure/health-check/deployment.yaml
```

This will create:
- ServiceAccount for the health-check pod
- ClusterRole with permissions to read pods across all namespaces
- ClusterRoleBinding to grant the ServiceAccount the necessary permissions
- Deployment, Service, and Secret resources

3. Verify the deployment:
```bash
kubectl get pods -n default | grep health-check
kubectl logs -n default -l app=health-check
```

### Service Discovery

The service queries Hasura instances using Kubernetes DNS. For an environment named `intuition-testnet-b`, it will query:
```
http://intuition-testnet-b-graphql-engine:8080/v1/graphql
```

## How It Works

1. **Request Validation**: The API validates that at least one of `contract_address` or `environment` is provided
2. **Authentication**: The middleware checks for a valid API key in the `x-api-key` header
3. **Contract Address Lookup**: For environment-based requests, the service queries Kubernetes to:
   - Find the `{environment}-decoded-consumer` pod in the `{environment}` namespace
   - Extract the `INTUITION_CONTRACT_ADDRESS` environment variable
4. **Hasura Query**: The service:
   - Constructs the Hasura URL using Kubernetes service discovery
   - Sends a GraphQL query for statistics
   - Parses and validates the response
   - If Hasura is offline, returns status "offline" with error details instead of failing
5. **Response**: Returns the contract address and stats in a structured JSON response

## Future Enhancements

- Implement `contract_address` based health checks
- Add caching for frequently queried stats
- Implement Prometheus metrics export
- Add more comprehensive health checks (database connectivity, etc.)

## License

See the main project LICENSE file.
