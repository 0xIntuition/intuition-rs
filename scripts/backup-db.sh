#!/bin/bash

# Database Backup Script
# This script creates a compressed backup of the PostgreSQL database running in Docker

set -e  # Exit on error

# Configuration
CONTAINER_NAME="database"
DB_NAME="storage"
DB_USER="postgres"
BACKUP_DIR="./backups"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="storage_backup_${TIMESTAMP}.dump"
BACKUP_PATH="${BACKUP_DIR}/${BACKUP_FILE}"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
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

# Check if container is running
print_info "Checking if database container is running..."
if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    print_error "Container '${CONTAINER_NAME}' is not running!"
    print_info "Start it with: docker-compose up -d database"
    exit 1
fi

# Create backup directory if it doesn't exist
print_info "Creating backup directory..."
mkdir -p "$BACKUP_DIR"

# Create backup
print_info "Starting backup of database '${DB_NAME}'..."
print_info "This may take a few minutes depending on database size..."

if docker exec "$CONTAINER_NAME" pg_dump \
    -U "$DB_USER" \
    -d "$DB_NAME" \
    --format=custom \
    --compress=9 \
    --verbose \
    2>&1 | tee /tmp/backup.log | grep -E "(processing|dumping)" | tail -5 > "$BACKUP_PATH"; then

    # Actually run the backup (the above was just for logging)
    docker exec "$CONTAINER_NAME" pg_dump \
        -U "$DB_USER" \
        -d "$DB_NAME" \
        --format=custom \
        --compress=9 > "$BACKUP_PATH"

    # Get backup file size
    BACKUP_SIZE=$(du -h "$BACKUP_PATH" | cut -f1)

    print_info "✓ Backup completed successfully!"
    print_info "Location: ${BACKUP_PATH}"
    print_info "Size: ${BACKUP_SIZE}"

    # Clean up old backups (keep last 7)
    print_info "Cleaning up old backups (keeping last 7)..."
    BACKUP_COUNT=$(ls -1 "${BACKUP_DIR}"/storage_backup_*.dump 2>/dev/null | wc -l | tr -d ' ')
    if [ "$BACKUP_COUNT" -gt 7 ]; then
        ls -t "${BACKUP_DIR}"/storage_backup_*.dump | tail -n +8 | xargs rm -f
        DELETED=$((BACKUP_COUNT - 7))
        print_info "Deleted ${DELETED} old backup(s)"
    fi

    # Show remaining backups
    print_info "Available backups:"
    ls -lh "${BACKUP_DIR}"/storage_backup_*.dump | tail -7 | awk '{print "  - " $9 " (" $5 ")"}'

    # Generate checksum for integrity verification
    print_info "Generating checksum..."
    if command -v shasum &> /dev/null; then
        shasum -a 256 "$BACKUP_PATH" > "${BACKUP_PATH}.sha256"
        print_info "Checksum saved to: ${BACKUP_PATH}.sha256"
    fi

    # Capture latest block information
    print_info "Capturing latest block information..."
    LATEST_BLOCK=$(docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -t -c "
        SELECT COALESCE(MAX(max_block), 0)
        FROM (
            SELECT MAX(block_number) as max_block FROM atom
            UNION ALL SELECT MAX(block_number) FROM deposit
            UNION ALL SELECT MAX(block_number) FROM redemption
            UNION ALL SELECT MAX(block_number) FROM share_price_change
            UNION ALL SELECT MAX(block_number) FROM triple
            UNION ALL SELECT MAX(block_number) FROM signal
            UNION ALL SELECT MAX(block_number) FROM event
            UNION ALL SELECT MAX(block_number::numeric) FROM vault
            UNION ALL SELECT MAX(block_number::numeric) FROM position
        ) sub;" | tr -d ' ')

    STATS_BLOCK=$(docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -t -c "
        SELECT COALESCE(last_processed_block_number, 0) FROM stats WHERE id = 0;" | tr -d ' ')

    # Save block metadata to file
    BLOCK_METADATA="${BACKUP_PATH}.block_info"
    cat > "$BLOCK_METADATA" <<EOF
Backup File: ${BACKUP_FILE}
Backup Timestamp: $(date -u +"%Y-%m-%d %H:%M:%S UTC")
Latest Block Ingested: ${LATEST_BLOCK}
Stats Table Block: ${STATS_BLOCK}
Database Name: ${DB_NAME}
EOF

    print_info "Latest block ingested: ${LATEST_BLOCK}"
    print_info "Block metadata saved to: ${BLOCK_METADATA}"

    print_info "Backup completed at: $(date)"

else
    print_error "Backup failed! Check the logs above for details."
    exit 1
fi
