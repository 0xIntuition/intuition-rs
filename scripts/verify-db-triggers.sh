#!/bin/bash

# Database Trigger Verification Script
# This script verifies that all expected triggers and functions are present after a restore

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

# Check if container is running
print_info "Checking if database container is running..."
if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    print_error "Container '${CONTAINER_NAME}' is not running!"
    print_info "Start it with: docker-compose up -d database"
    exit 1
fi

print_info "Verifying database '${DB_NAME}'..."
print_info ""

# Expected triggers (based on your schema)
EXPECTED_TRIGGERS=(
    "account_insert_trigger"
    "atom_insert_trigger"
    "triple_insert_trigger"
    "position_insert_trigger"
    "position_delete_trigger"
    "signal_insert_trigger"
    "fee_insert_trigger"
    "deposit_position_update_trigger"
    "redemption_position_update_trigger"
    "triple_vault_term_total_state_change_trigger"
    "term_total_state_change_trigger"
    "version_change_trigger"
    "thing_insert_update_term_text_trigger"
    "person_insert_update_term_text_trigger"
    "book_insert_update_term_text_trigger"
    "organization_insert_update_term_text_trigger"
    "position_update_trigger"
    "position_reopen_trigger"
    "position_close_trigger"
    "term_total_assets_market_cap_update_trigger"
    "triple_vault_term_totals_trigger"
    "vault_triple_vault_trigger"
    "triple_predicate_object_trigger"
    "triple_subject_predicate_trigger"
    "triple_term_predicate_object_trigger"
    "triple_term_subject_predicate_trigger"
)

# Section: Triggers
print_section "TRIGGERS VERIFICATION"

TRIGGER_QUERY="SELECT trigger_name, event_object_table, action_timing, event_manipulation
FROM information_schema.triggers
WHERE trigger_schema = 'public'
ORDER BY event_object_table, trigger_name;"

print_info "Total triggers found:"
TRIGGER_COUNT=$(docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT COUNT(*) FROM information_schema.triggers WHERE trigger_schema = 'public';" | tr -d ' ')
echo "  ${TRIGGER_COUNT} triggers"

print_info ""
print_info "Checking expected triggers..."
MISSING_TRIGGERS=0
FOUND_TRIGGERS=0

for trigger in "${EXPECTED_TRIGGERS[@]}"; do
    if docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT 1 FROM information_schema.triggers WHERE trigger_schema = 'public' AND trigger_name = '${trigger}';" | grep -q 1; then
        echo -e "  ${GREEN}✓${NC} ${trigger}"
        FOUND_TRIGGERS=$((FOUND_TRIGGERS + 1))
    else
        echo -e "  ${RED}✗${NC} ${trigger} - MISSING!"
        MISSING_TRIGGERS=$((MISSING_TRIGGERS + 1))
    fi
done

print_info ""
print_info "Trigger summary: ${FOUND_TRIGGERS}/${#EXPECTED_TRIGGERS[@]} expected triggers found"

if [ $MISSING_TRIGGERS -gt 0 ]; then
    print_error "${MISSING_TRIGGERS} triggers are missing!"
fi

# Section: Functions
print_section "FUNCTIONS VERIFICATION"

FUNCTION_QUERY="SELECT routine_name, routine_type
FROM information_schema.routines
WHERE routine_schema = 'public'
ORDER BY routine_name;"

print_info "Total functions found:"
FUNCTION_COUNT=$(docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT COUNT(*) FROM information_schema.routines WHERE routine_schema = 'public';" | tr -d ' ')
echo "  ${FUNCTION_COUNT} functions"

print_info ""
print_info "Functions list:"
docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -t -c "$FUNCTION_QUERY" | head -20 | sed 's/^/  /'

if [ "$FUNCTION_COUNT" -gt 20 ]; then
    print_info "  ... and $((FUNCTION_COUNT - 20)) more"
fi

# Section: Tables
print_section "TABLES VERIFICATION"

print_info "Total tables found:"
TABLE_COUNT=$(docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public' AND table_type = 'BASE TABLE';" | tr -d ' ')
echo "  ${TABLE_COUNT} tables"

print_info ""
print_info "Key tables:"
KEY_TABLES=("account" "atom" "triple" "position" "vault" "deposit" "redemption" "signal" "fee_transfer" "term" "triple_term" "triple_vault" "stats")

for table in "${KEY_TABLES[@]}"; do
    if docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT 1 FROM information_schema.tables WHERE table_schema = 'public' AND table_name = '${table}';" | grep -q 1; then
        # Get row count
        ROW_COUNT=$(docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT COUNT(*) FROM ${table};" 2>/dev/null | tr -d ' ' || echo "N/A")
        echo -e "  ${GREEN}✓${NC} ${table} (${ROW_COUNT} rows)"
    else
        echo -e "  ${RED}✗${NC} ${table} - MISSING!"
    fi
done

# Section: Indexes
print_section "INDEXES VERIFICATION"

INDEX_COUNT=$(docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT COUNT(*) FROM pg_indexes WHERE schemaname = 'public';" | tr -d ' ')
print_info "Total indexes found: ${INDEX_COUNT}"

# Section: Extensions
print_section "EXTENSIONS VERIFICATION"

print_info "Installed extensions:"
docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -c "SELECT extname, extversion FROM pg_extension;" | sed 's/^/  /'

# Section: Database Statistics
print_section "DATABASE STATISTICS"

STATS_QUERY="SELECT
  pg_size_pretty(pg_database_size('${DB_NAME}')) as db_size,
  (SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public') as tables,
  (SELECT COUNT(*) FROM information_schema.triggers WHERE trigger_schema = 'public') as triggers,
  (SELECT COUNT(*) FROM information_schema.routines WHERE routine_schema = 'public') as functions,
  (SELECT COUNT(*) FROM pg_indexes WHERE schemaname = 'public') as indexes;"

docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -c "$STATS_QUERY" | sed 's/^/  /'

# Section: Health Check
print_section "HEALTH CHECK"

# Check if stats table exists and has data
if docker exec "$CONTAINER_NAME" psql -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT 1 FROM stats WHERE id = 0;" | grep -q 1; then
    print_info "✓ Stats table is initialized"
else
    print_warning "Stats table might not be initialized properly"
fi

# Final summary
print_section "SUMMARY"

ISSUES=0

if [ $MISSING_TRIGGERS -gt 0 ]; then
    print_error "⚠ ${MISSING_TRIGGERS} missing triggers detected"
    ISSUES=$((ISSUES + 1))
else
    print_info "✓ All expected triggers are present"
fi

if [ "$FUNCTION_COUNT" -lt 20 ]; then
    print_warning "⚠ Function count seems low (expected ~25+)"
    ISSUES=$((ISSUES + 1))
else
    print_info "✓ Functions look good (${FUNCTION_COUNT} functions)"
fi

if [ "$TABLE_COUNT" -lt 20 ]; then
    print_warning "⚠ Table count seems low"
    ISSUES=$((ISSUES + 1))
else
    print_info "✓ Tables look good (${TABLE_COUNT} tables)"
fi

print_info ""
if [ $ISSUES -eq 0 ]; then
    print_info "✅ Database verification PASSED! Everything looks good."
    exit 0
else
    print_warning "⚠️  Database verification completed with ${ISSUES} issue(s)."
    print_warning "Review the issues above and consider re-running the restore if critical components are missing."
    exit 1
fi
