#!/bin/bash

# List Database Backups Script
# This script lists all available database backups with details

# Configuration
BACKUP_DIR="./backups"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_section() {
    echo ""
    echo -e "${BLUE}==================== $1 ====================${NC}"
}

# Check if backup directory exists
if [ ! -d "$BACKUP_DIR" ]; then
    echo "No backup directory found at: ${BACKUP_DIR}"
    echo "Create your first backup with: ./scripts/backup-db.sh"
    exit 1
fi

# Count backups
BACKUP_COUNT=$(ls -1 "${BACKUP_DIR}"/storage_backup_*.dump 2>/dev/null | wc -l | tr -d ' ')

if [ "$BACKUP_COUNT" -eq 0 ]; then
    echo "No backups found in: ${BACKUP_DIR}"
    echo "Create your first backup with: ./scripts/backup-db.sh"
    exit 0
fi

print_section "AVAILABLE BACKUPS"
print_info "Found ${BACKUP_COUNT} backup(s) in ${BACKUP_DIR}"
echo ""

# Print header
printf "%-40s %-10s %-20s %-10s %s\n" "FILENAME" "SIZE" "DATE" "BLOCK" "CHECKSUM"
printf "%-40s %-10s %-20s %-10s %s\n" "----------------------------------------" "----------" "--------------------" "----------" "----------"

# List backups with details
ls -lt "${BACKUP_DIR}"/storage_backup_*.dump | while read -r line; do
    # Extract file info
    FILENAME=$(echo "$line" | awk '{print $NF}')
    BASENAME=$(basename "$FILENAME")
    SIZE=$(echo "$line" | awk '{print $5}')
    SIZE_HUMAN=$(du -h "$FILENAME" | cut -f1)
    DATE=$(echo "$line" | awk '{print $6, $7, $8}')

    # Check for checksum
    if [ -f "${FILENAME}.sha256" ]; then
        CHECKSUM="${GREEN}✓${NC}"
    else
        CHECKSUM="${YELLOW}-${NC}"
    fi

    # Check for block info
    if [ -f "${FILENAME}.block_info" ]; then
        BLOCK=$(grep "Latest Block Ingested:" "${FILENAME}.block_info" | awk '{print $NF}')
        if [ -z "$BLOCK" ]; then
            BLOCK="-"
        fi
    else
        BLOCK="-"
    fi

    printf "%-40s %-10s %-20s %-10s %b\n" "$BASENAME" "$SIZE_HUMAN" "$DATE" "$BLOCK" "$CHECKSUM"
done

echo ""
print_info "Usage examples:"
echo "  Restore latest: ./scripts/restore-db.sh latest"
echo "  Restore specific: ./scripts/restore-db.sh ${BACKUP_DIR}/storage_backup_YYYYMMDD_HHMMSS.dump"
echo "  Create new backup: ./scripts/backup-db.sh"
echo "  Verify database: ./scripts/verify-db-triggers.sh"
echo "  Check latest block: ./scripts/check-latest-block.sh"

# Calculate total backup size
TOTAL_SIZE=$(du -sh "${BACKUP_DIR}" 2>/dev/null | cut -f1)
echo ""
print_info "Total backup space used: ${TOTAL_SIZE}"
print_info "Note: BLOCK column shows the latest block ingested in each backup"
