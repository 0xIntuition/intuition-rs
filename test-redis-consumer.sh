#!/bin/bash

echo "Testing Redis consumer setup..."

# Source environment variables
source .env

echo "Environment variables:"
echo "CONSUMER_TYPE: $CONSUMER_TYPE"
echo "REDIS_URL: $REDIS_URL"
echo "RAW_CONSUMER_QUEUE_URL: $RAW_CONSUMER_QUEUE_URL"

echo ""
echo "Testing Redis connectivity..."
redis-cli -u $REDIS_URL ping

echo ""
echo "Available streams:"
redis-cli -u $REDIS_URL --scan --pattern "*_stream"

echo ""
echo "Stream info for raw_logs_stream:"
redis-cli -u $REDIS_URL XINFO STREAM raw_logs_stream

echo ""
echo "Consumer groups for raw_logs_stream:"
redis-cli -u $REDIS_URL XINFO GROUPS raw_logs_stream

echo ""
echo "Testing consumer read from raw_logs_stream:"
redis-cli -u $REDIS_URL XREADGROUP GROUP intuition-consumer-group test-debug-consumer COUNT 5 STREAMS raw_logs_stream ">"

echo ""
echo "Now testing the consumer with verbose logging..."
RUST_LOG=debug cargo run --bin consumer -- --mode raw