#!/bin/bash

# Cleanup Old Backups Script
# This script removes old database backups based on retention policy

# Configuration
BACKUP_DIR="./backups"
DEFAULT_KEEP=7  # Keep last 7 backups by default

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to show usage
usage() {
    echo "Usage: $0 [number_to_keep]"
    echo ""
    echo "Examples:"
    echo "  $0           # Keep last 7 backups (default)"
    echo "  $0 14        # Keep last 14 backups"
    echo "  $0 all       # Remove all backups (with confirmation)"
    exit 1
}

# Parse arguments
KEEP=${1:-$DEFAULT_KEEP}

if [ "$KEEP" == "all" ]; then
    print_warning "⚠️  This will DELETE ALL backups!"
    echo -n "Type 'DELETE ALL' to confirm: "
    read -r confirmation
    if [ "$confirmation" != "DELETE ALL" ]; then
        print_info "Cleanup cancelled."
        exit 0
    fi
    KEEP=0
elif ! [[ "$KEEP" =~ ^[0-9]+$ ]]; then
    print_error "Invalid argument: $KEEP"
    usage
fi

# Check if backup directory exists
if [ ! -d "$BACKUP_DIR" ]; then
    print_error "No backup directory found at: ${BACKUP_DIR}"
    exit 1
fi

# Count backups
BACKUP_COUNT=$(ls -1 "${BACKUP_DIR}"/storage_backup_*.dump 2>/dev/null | wc -l | tr -d ' ')

if [ "$BACKUP_COUNT" -eq 0 ]; then
    print_info "No backups found in: ${BACKUP_DIR}"
    exit 0
fi

print_info "Found ${BACKUP_COUNT} backup(s)"

if [ "$BACKUP_COUNT" -le "$KEEP" ]; then
    print_info "Nothing to clean up (keeping last ${KEEP} backups)"
    exit 0
fi

TO_DELETE=$((BACKUP_COUNT - KEEP))
print_info "Will delete ${TO_DELETE} old backup(s), keeping the latest ${KEEP}"

echo ""
print_warning "Backups to be deleted:"
ls -t "${BACKUP_DIR}"/storage_backup_*.dump | tail -n "+$((KEEP + 1))" | while read -r file; do
    SIZE=$(du -h "$file" | cut -f1)
    echo "  - $(basename "$file") (${SIZE})"
done

echo ""
echo -n "Proceed with deletion? (y/N): "
read -r response

if [[ ! "$response" =~ ^[Yy]$ ]]; then
    print_info "Cleanup cancelled."
    exit 0
fi

# Delete old backups
print_info "Deleting old backups..."
DELETED=0
ls -t "${BACKUP_DIR}"/storage_backup_*.dump | tail -n "+$((KEEP + 1))" | while read -r file; do
    rm -f "$file"
    rm -f "${file}.sha256"  # Also remove checksum file
    print_info "Deleted: $(basename "$file")"
    DELETED=$((DELETED + 1))
done

print_info "✓ Cleanup completed!"
print_info "Remaining backups: $(ls -1 "${BACKUP_DIR}"/storage_backup_*.dump 2>/dev/null | wc -l | tr -d ' ')"

# Show disk space freed (approximate)
REMAINING_SIZE=$(du -sh "${BACKUP_DIR}" 2>/dev/null | cut -f1)
print_info "Backup directory size: ${REMAINING_SIZE}"
