#!/bin/bash

# Check Latest Block Script
# This script checks the latest block number ingested in the database

set -e  # Exit on error

# Configuration
CONTAINER_NAME="database"
DB_NAME="storage"
DB_USER="postgres"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Function to print colored messages
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_section() {
    echo ""
    echo -e "${BLUE}==================== $1 ====================${NC}"
}

print_highlight() {
    echo -e "${CYAN}$1${NC}"
}

# Check if container is running
if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    print_error "Container '${CONTAINER_NAME}' is not running!"
    print_info "Start it with: docker-compose up -d database"
    exit 1
fi

print_section "LATEST BLOCK ANALYSIS"

# Query to get the latest block from stats table
STATS_BLOCK=$(docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -t -c "
SELECT COALESCE(last_processed_block_number, 0)
FROM stats
WHERE id = 0;" | tr -d ' ')

STATS_TIMESTAMP=$(docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -t -c "
SELECT COALESCE(last_processed_block_timestamp, NOW())
FROM stats
WHERE id = 0;" | xargs)

print_info "Stats table reports:"
echo "  Block: ${STATS_BLOCK}"
echo "  Timestamp: ${STATS_TIMESTAMP}"

# Query to get the maximum block across all tables
print_section "BLOCK NUMBERS BY TABLE"

docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -c "
SELECT
  table_name,
  max_block_number,
  row_count,
  CASE
    WHEN max_block_number IS NULL THEN '(empty)'
    WHEN max_block_number = (SELECT MAX(max_block) FROM (
      SELECT MAX(block_number) as max_block FROM atom
      UNION ALL SELECT MAX(block_number) FROM deposit
      UNION ALL SELECT MAX(block_number) FROM redemption
      UNION ALL SELECT MAX(block_number) FROM share_price_change
      UNION ALL SELECT MAX(block_number) FROM triple
      UNION ALL SELECT MAX(block_number) FROM signal
      UNION ALL SELECT MAX(block_number) FROM fee_transfer
      UNION ALL SELECT MAX(block_number) FROM event
    ) sub) THEN '← LATEST'
    ELSE ''
  END as note
FROM (
  SELECT
    'atom' as table_name,
    MAX(block_number) as max_block_number,
    COUNT(*) as row_count
  FROM atom
  UNION ALL
  SELECT
    'deposit',
    MAX(block_number),
    COUNT(*)
  FROM deposit
  UNION ALL
  SELECT
    'redemption',
    MAX(block_number),
    COUNT(*)
  FROM redemption
  UNION ALL
  SELECT
    'share_price_change',
    MAX(block_number),
    COUNT(*)
  FROM share_price_change
  UNION ALL
  SELECT
    'triple',
    MAX(block_number),
    COUNT(*)
  FROM triple
  UNION ALL
  SELECT
    'signal',
    MAX(block_number),
    COUNT(*)
  FROM signal
  UNION ALL
  SELECT
    'fee_transfer',
    MAX(block_number),
    COUNT(*)
  FROM fee_transfer
  UNION ALL
  SELECT
    'event',
    MAX(block_number),
    COUNT(*)
  FROM event
  UNION ALL
  SELECT
    'vault',
    MAX(block_number::numeric),
    COUNT(*)
  FROM vault
  UNION ALL
  SELECT
    'position',
    MAX(block_number::numeric),
    COUNT(*)
  FROM position
) sub
ORDER BY max_block_number DESC NULLS LAST;" | sed 's/^/  /'

# Get the overall maximum block
MAX_BLOCK=$(docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -t -c "
SELECT COALESCE(MAX(max_block), 0)
FROM (
  SELECT MAX(block_number) as max_block FROM atom
  UNION ALL SELECT MAX(block_number) FROM deposit
  UNION ALL SELECT MAX(block_number) FROM redemption
  UNION ALL SELECT MAX(block_number) FROM share_price_change
  UNION ALL SELECT MAX(block_number) FROM triple
  UNION ALL SELECT MAX(block_number) FROM signal
  UNION ALL SELECT MAX(block_number) FROM fee_transfer
  UNION ALL SELECT MAX(block_number) FROM event
  UNION ALL SELECT MAX(block_number::numeric) FROM vault
  UNION ALL SELECT MAX(block_number::numeric) FROM position
) sub;" | tr -d ' ')

print_section "SUMMARY"

echo ""
print_highlight "📊 Latest Block Information:"
echo ""
echo "  ╔════════════════════════════════════════╗"
echo "  ║  Latest Block Ingested: ${MAX_BLOCK}        ║"
echo "  ║  Stats Table Block:     ${STATS_BLOCK}        ║"
echo "  ╚════════════════════════════════════════╝"
echo ""

if [ "$MAX_BLOCK" -eq "$STATS_BLOCK" ]; then
    print_info "✓ Stats table is in sync with actual data"
else
    print_warning "⚠ Stats table block (${STATS_BLOCK}) differs from max block (${MAX_BLOCK})"
fi

print_info ""
print_info "Use this block number when resuming indexing after a restore"
print_info "Indexer should resume from block: ${MAX_BLOCK}"

# If JSON output is requested
if [ "$1" == "--json" ]; then
    echo ""
    echo "{"
    echo "  \"latest_block\": ${MAX_BLOCK},"
    echo "  \"stats_block\": ${STATS_BLOCK},"
    echo "  \"stats_timestamp\": \"${STATS_TIMESTAMP}\","
    echo "  \"in_sync\": $([ "$MAX_BLOCK" -eq "$STATS_BLOCK" ] && echo "true" || echo "false")"
    echo "}"
fi

# Save to file if requested
if [ "$1" == "--save" ] || [ "$2" == "--save" ]; then
    BLOCK_INFO_FILE="./backups/latest_block_info.txt"
    echo "Latest Block: ${MAX_BLOCK}" > "$BLOCK_INFO_FILE"
    echo "Stats Block: ${STATS_BLOCK}" >> "$BLOCK_INFO_FILE"
    echo "Timestamp: ${STATS_TIMESTAMP}" >> "$BLOCK_INFO_FILE"
    echo "Checked At: $(date)" >> "$BLOCK_INFO_FILE"
    print_info "Block information saved to: ${BLOCK_INFO_FILE}"
fi
