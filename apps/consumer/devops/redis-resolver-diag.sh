#!/bin/sh
# Run on intuition-mainnet-next (or wherever resolver consumer + Redis live).
# Usage: REDIS_URL=redis://host:6379 ./redis-resolver-diag.sh
#    or: REDIS_HOST=redis REDIS_PORT=6379 ./redis-resolver-diag.sh

set -e

if [ -n "$REDIS_URL" ]; then
  REDIS_CLI_ARGS="-u ${REDIS_URL}"
else
  REDIS_HOST=${REDIS_HOST:-redis}
  REDIS_PORT=${REDIS_PORT:-6379}
  REDIS_CLI_ARGS="-h ${REDIS_HOST} -p ${REDIS_PORT}"
fi

STREAM="${RESOLVER_STREAM:-resolver_stream}"
GROUP="${CONSUMER_GROUP:-intuition-consumer-group}"

echo "=== Redis resolver stream diagnostic ==="
echo "Stream: $STREAM  Group: $GROUP"
echo ""

echo "--- Ping ---"
redis-cli ${REDIS_CLI_ARGS} ping || { echo "Redis unreachable"; exit 1; }
echo ""

echo "--- Stream length (XLEN) ---"
redis-cli ${REDIS_CLI_ARGS} XLEN "$STREAM"
echo ""

echo "--- Consumer groups (XINFO GROUPS) ---"
redis-cli ${REDIS_CLI_ARGS} XINFO GROUPS "$STREAM" 2>/dev/null || echo "(no groups or stream missing)"
echo ""

echo "--- Pending summary (XPENDING, no range) ---"
redis-cli ${REDIS_CLI_ARGS} XPENDING "$STREAM" "$GROUP" 2>/dev/null || echo "(failed)"
echo ""

echo "--- First 20 pending entries [id, consumer, idle_ms, delivery_count] ---"
redis-cli ${REDIS_CLI_ARGS} XPENDING "$STREAM" "$GROUP" - + 20
echo ""

echo "--- Stream info (XINFO STREAM) ---"
redis-cli ${REDIS_CLI_ARGS} XINFO STREAM "$STREAM" 2>/dev/null || echo "(failed)"
echo ""

echo "--- Consumers in group (XINFO CONSUMERS) ---"
redis-cli ${REDIS_CLI_ARGS} XINFO CONSUMERS "$STREAM" "$GROUP" 2>/dev/null || echo "(failed)"
