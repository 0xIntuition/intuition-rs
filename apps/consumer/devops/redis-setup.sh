#!/bin/sh
echo "Initializing Redis streams..."

# Set Redis connection parameters
REDIS_HOST=${REDIS_HOST:-redis}
REDIS_PORT=${REDIS_PORT:-6379}
REDIS_CLI_ARGS="-h ${REDIS_HOST} -p ${REDIS_PORT}"

# Wait for Redis to be ready
echo "Waiting for Redis to be ready..."
until redis-cli ${REDIS_CLI_ARGS} ping >/dev/null 2>&1; do
  echo "Redis is unavailable - sleeping"
  sleep 1
done
echo "Redis is ready!"

# Connect to Redis and create the streams
# Note: Redis streams are created automatically when first message is added,
# but we can pre-create them with consumer groups for better organization

# Raw logs stream (equivalent to raw_logs.fifo)
redis-cli ${REDIS_CLI_ARGS} XGROUP CREATE raw_logs_stream intuition-consumer-group 0 MKSTREAM || echo "raw_logs_stream consumer group already exists or stream exists"

# Decoded logs stream (equivalent to decoded_logs.fifo)  
redis-cli ${REDIS_CLI_ARGS} XGROUP CREATE decoded_logs_stream intuition-consumer-group 0 MKSTREAM || echo "decoded_logs_stream consumer group already exists or stream exists"

# Resolver stream (equivalent to resolver queue)
redis-cli ${REDIS_CLI_ARGS} XGROUP CREATE resolver_stream intuition-consumer-group 0 MKSTREAM || echo "resolver_stream consumer group already exists or stream exists"

# IPFS upload stream (equivalent to ipfs_upload queue)
redis-cli ${REDIS_CLI_ARGS} XGROUP CREATE ipfs_upload_stream intuition-consumer-group 0 MKSTREAM || echo "ipfs_upload_stream consumer group already exists or stream exists"

# Create additional consumer groups for different environments if needed
# You can add more consumer groups here for load balancing

echo "Redis streams initialized successfully!"

# Optional: List all streams to verify creation
echo "Available streams:"
redis-cli ${REDIS_CLI_ARGS} --scan --pattern "*_stream"

# Optional: Show stream info
echo "Stream details:"
redis-cli ${REDIS_CLI_ARGS} XINFO GROUPS raw_logs_stream 2>/dev/null || echo "No groups for raw_logs_stream yet"
redis-cli ${REDIS_CLI_ARGS} XINFO GROUPS decoded_logs_stream 2>/dev/null || echo "No groups for decoded_logs_stream yet"  
redis-cli ${REDIS_CLI_ARGS} XINFO GROUPS resolver_stream 2>/dev/null || echo "No groups for resolver_stream yet"
redis-cli ${REDIS_CLI_ARGS} XINFO GROUPS ipfs_upload_stream 2>/dev/null || echo "No groups for ipfs_upload_stream yet"