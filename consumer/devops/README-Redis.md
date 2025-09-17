# Redis Streams Setup for Intuition Consumer

This directory contains setup scripts and configuration for Redis Streams, which provides an alternative message queue backend to SQS for the Intuition consumer system.

## Files

- `redis-setup.sh`: Initialization script that creates Redis streams and consumer groups
- `localstack-setup.sh`: Original SQS setup script for comparison

## Redis Streams Created

The setup script creates the following Redis streams with corresponding SQS queue equivalents:

| Redis Stream | SQS Queue Equivalent | Purpose |
|--------------|---------------------|---------|
| `raw_logs_stream` | `raw_logs.fifo` | Raw blockchain event logs |
| `decoded_logs_stream` | `decoded_logs.fifo` | Decoded contract events |
| `resolver_stream` | `resolver` | ENS name resolution tasks |
| `ipfs_upload_stream` | `ipfs_upload` | IPFS upload tasks |

## Consumer Groups

Each stream has an `intuition-consumer-group` consumer group pre-created for load balancing across multiple consumer instances.

## Usage with Docker Compose

The Redis setup is automatically executed when running:

```bash
docker-compose -f docker-compose-shared.yml up
```

This will:
1. Start Redis server
2. Wait for Redis to be healthy
3. Run the setup script to create streams and consumer groups

## Environment Variables for Redis

When using Redis consumers, set these environment variables:

```bash
# Consumer type
CONSUMER_TYPE=redis_hybrid
# or
CONSUMER_TYPE=redis_streams

# Redis connection
REDIS_URL=redis://redis:6379

# Stream names (these match the streams created by setup script)
RAW_CONSUMER_STREAM=raw_logs_stream
DECODED_LOGS_STREAM=decoded_logs_stream
RESOLVER_STREAM=resolver_stream
IPFS_UPLOAD_STREAM=ipfs_upload_stream

# Optional: Custom consumer name prefix for horizontal scaling
# If not set, defaults to "intuition-consumer"
CONSUMER_NAME_PREFIX=my-app-consumer
```

## Horizontal Scaling

Redis Streams consumers support horizontal scaling by running multiple consumer instances. Each consumer gets a unique name based on:
- Custom prefix (via `CONSUMER_NAME_PREFIX` env var)
- Consumer mode
- Hostname 
- Process ID
- Timestamp

This ensures multiple consumers can work together in the same consumer group without conflicts.

Example consumer names:
- `intuition-consumer-decoded-worker-1-12345-1679123456789`
- `my-app-consumer-resolver-worker-2-12346-1679123456790`

To scale horizontally:
1. Deploy multiple instances with the same configuration
2. Optionally set different `CONSUMER_NAME_PREFIX` values per deployment
3. All instances will automatically join the same consumer group and share the workload

## Manual Setup

If you need to run the setup script manually:

```bash
# Make sure Redis is running
redis-server

# Run the setup script
./redis-setup.sh
```

## Monitoring Redis Streams

You can monitor the streams using Redis CLI:

```bash
# List all streams
redis-cli --scan --pattern "*_stream"

# Get stream info
redis-cli XINFO STREAM raw_logs_stream

# Get consumer group info
redis-cli XINFO GROUPS raw_logs_stream

# Get consumer info
redis-cli XINFO CONSUMERS raw_logs_stream intuition-consumer-group
```

## Stream Management Commands

```bash
# Add a test message to a stream
redis-cli XADD raw_logs_stream * body "test message"

# Read messages from stream
redis-cli XREAD STREAMS raw_logs_stream 0

# Read with consumer group
redis-cli XREADGROUP GROUP intuition-consumer-group consumer-1 STREAMS raw_logs_stream >
```

## Comparison with SQS

| Feature | SQS | Redis Streams |
|---------|-----|---------------|
| Persistence | Cloud-managed | Configurable (AOF/RDB) |
| Ordering | FIFO queues | Natural ordering by timestamp |
| Consumer Groups | Not native | Built-in support |
| Acknowledgment | Delete message | XACK command |
| Scalability | Auto-scaling | Manual scaling |
| Cost | Pay per use | Infrastructure cost |
| Latency | Network dependent | Very low (in-memory) |

## Troubleshooting

If streams are not created properly:

1. Check Redis server logs
2. Verify Redis is accessible: `redis-cli ping`
3. Manually run setup commands:
   ```bash
   redis-cli XGROUP CREATE raw_logs_stream intuition-consumer-group 0 MKSTREAM
   ```
4. Check if consumer group exists:
   ```bash
   redis-cli XINFO GROUPS raw_logs_stream
   ```