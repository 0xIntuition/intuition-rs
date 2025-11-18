#!/bin/bash

# Database Restore Script
# This script restores a PostgreSQL database from a backup file

set -e  # Exit on error

# Configuration
CONTAINER_NAME="database"
DB_NAME="storage"
DB_USER="postgres"
BACKUP_DIR="./backups"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
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

print_prompt() {
    echo -e "${BLUE}[PROMPT]${NC} $1"
}

# Function to show usage
usage() {
    echo "Usage: $0 <backup_file>"
    echo ""
    echo "Examples:"
    echo "  $0 ./backups/storage_backup_20241118_120000.dump"
    echo "  $0 latest  # Restores the most recent backup"
    echo ""
    echo "Available backups:"
    if ls "${BACKUP_DIR}"/storage_backup_*.dump 1> /dev/null 2>&1; then
        ls -lh "${BACKUP_DIR}"/storage_backup_*.dump | tail -10 | awk '{print "  - " $9 " (" $5 ", " $6 " " $7 " " $8 ")"}'
    else
        echo "  No backups found in ${BACKUP_DIR}"
    fi
    exit 1
}

# Check if backup file is provided
if [ $# -eq 0 ]; then
    print_error "No backup file specified!"
    usage
fi

# Handle 'latest' keyword
if [ "$1" == "latest" ]; then
    BACKUP_FILE=$(ls -t "${BACKUP_DIR}"/storage_backup_*.dump 2>/dev/null | head -1)
    if [ -z "$BACKUP_FILE" ]; then
        print_error "No backup files found in ${BACKUP_DIR}"
        exit 1
    fi
    print_info "Using latest backup: ${BACKUP_FILE}"
else
    BACKUP_FILE="$1"
fi

# Check if backup file exists
if [ ! -f "$BACKUP_FILE" ]; then
    print_error "Backup file not found: ${BACKUP_FILE}"
    usage
fi

# Verify checksum if available
if [ -f "${BACKUP_FILE}.sha256" ]; then
    print_info "Verifying backup integrity..."
    if command -v shasum &> /dev/null; then
        if shasum -a 256 -c "${BACKUP_FILE}.sha256" --status 2>/dev/null; then
            print_info "✓ Checksum verified successfully"
        else
            print_warning "Checksum verification failed! Backup may be corrupted."
            print_prompt "Continue anyway? (y/N): "
            read -r response
            if [[ ! "$response" =~ ^[Yy]$ ]]; then
                print_info "Restore cancelled."
                exit 1
            fi
        fi
    fi
fi

# Display block information if available
if [ -f "${BACKUP_FILE}.block_info" ]; then
    print_info "Backup block information:"
    cat "${BACKUP_FILE}.block_info" | grep -E "(Latest Block|Stats Table Block)" | sed 's/^/  /'
fi

# Check if container is running
print_info "Checking if database container is running..."
if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    print_error "Container '${CONTAINER_NAME}' is not running!"
    print_info "Start it with: docker-compose up -d database"
    exit 1
fi

# Warning prompt
print_warning "⚠️  WARNING: This will REPLACE all data in the '${DB_NAME}' database!"
print_warning "All existing data, including tables, triggers, and functions will be dropped."
print_prompt "Are you sure you want to continue? Type 'yes' to proceed: "
read -r confirmation

if [ "$confirmation" != "yes" ]; then
    print_info "Restore cancelled."
    exit 0
fi

# Create a pre-restore backup as safety measure
print_info "Creating safety backup before restore..."
SAFETY_BACKUP="${BACKUP_DIR}/pre_restore_safety_$(date +%Y%m%d_%H%M%S).dump"
docker exec "$CONTAINER_NAME" pg_dump \
    -U "$DB_USER" \
    -d "$DB_NAME" \
    --format=custom \
    --compress=9 > "$SAFETY_BACKUP" 2>/dev/null || true
print_info "Safety backup created: ${SAFETY_BACKUP}"

# Perform restore
print_info "Starting restore from: ${BACKUP_FILE}"
print_info "This may take several minutes..."

# Copy backup file into container
print_info "Copying backup file to container..."
docker cp "$BACKUP_FILE" "${CONTAINER_NAME}:/tmp/restore.dump"

# Restore the database
print_info "Restoring database..."
if docker exec "$CONTAINER_NAME" pg_restore \
    -U "$DB_USER" \
    -d "$DB_NAME" \
    --clean \
    --if-exists \
    --no-owner \
    --no-acl \
    --verbose \
    /tmp/restore.dump 2>&1 | tee /tmp/restore.log | grep -E "(processing|creating|restoring)"; then

    print_info "✓ Restore completed successfully!"

    # Clean up temp file in container
    docker exec "$CONTAINER_NAME" rm -f /tmp/restore.dump

    # Verify restore
    print_info "Verifying database state..."
    TABLE_COUNT=$(docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public';" | tr -d ' ')
    TRIGGER_COUNT=$(docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT COUNT(*) FROM information_schema.triggers WHERE trigger_schema = 'public';" | tr -d ' ')
    FUNCTION_COUNT=$(docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT COUNT(*) FROM information_schema.routines WHERE routine_schema = 'public';" | tr -d ' ')

    print_info "Database statistics:"
    print_info "  - Tables: ${TABLE_COUNT}"
    print_info "  - Triggers: ${TRIGGER_COUNT}"
    print_info "  - Functions: ${FUNCTION_COUNT}"

    # Check latest block after restore
    print_info "Checking latest block after restore..."
    RESTORED_BLOCK=$(docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -t -c "
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

    print_info "  - Latest block restored: ${RESTORED_BLOCK}"
    print_warning "⚠️  Resume indexing from block: ${RESTORED_BLOCK}"

    print_info "Restore completed at: $(date)"
    print_info ""
    print_info "Next steps:"
    print_info "  1. Run './scripts/verify-db-triggers.sh' to verify all triggers"
    print_info "  2. Run './scripts/check-latest-block.sh' for detailed block analysis"
    print_info "  3. Resume indexer from block ${RESTORED_BLOCK}"
    print_info "  4. Test your application to ensure everything works correctly"
    print_info "  5. If issues occur, restore from safety backup: ${SAFETY_BACKUP}"

else
    print_error "Restore failed! Check the logs above for details."
    print_error "You can restore from the safety backup: ${SAFETY_BACKUP}"
    docker exec "$CONTAINER_NAME" rm -f /tmp/restore.dump
    exit 1
fi
