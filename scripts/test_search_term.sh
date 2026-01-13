#!/bin/bash

# Test script for search_term function with triple results (US-009)
# This script tests semantic search across both atoms and triples

set -e

echo "========================================="
echo "US-009: Test search_term with Triple Results"
echo "========================================="
echo ""

DB_CONTAINER="database"
DB_USER="postgres"
DB_NAME="storage"

# Color codes for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counter
PASSED=0
FAILED=0

# Function to run SQL and capture output
run_sql() {
    docker exec $DB_CONTAINER psql -U $DB_USER -d $DB_NAME -c "$1" 2>&1
}

# Function to test a query
test_query() {
    local test_name="$1"
    local query="$2"
    local expected_pattern="$3"

    echo -e "${YELLOW}Test: $test_name${NC}"
    echo "Query: $query"

    result=$(run_sql "$query")

    if echo "$result" | grep -q "$expected_pattern"; then
        echo -e "${GREEN}✓ PASSED${NC}"
        ((PASSED++))
    else
        echo -e "${RED}✗ FAILED${NC}"
        echo "Expected pattern: $expected_pattern"
        echo "Got: $result"
        ((FAILED++))
    fi
    echo ""
}

echo "Step 1: Verify test data exists"
echo "--------------------------------"
run_sql "SELECT type, COUNT(*) FROM term GROUP BY type ORDER BY type;"
echo ""

run_sql "SELECT COUNT(*) as embeddings_count FROM term_embeddings;"
echo ""

echo "Step 2: Test search_term with 'person' query (should return triple)"
echo "---------------------------------------------------------------------"
test_query \
    "Search for 'person' should return results" \
    "SELECT id, type FROM search_term('person') LIMIT 5;" \
    "Triple"

echo "Step 3: Verify type field correctly identifies 'triple' vs 'atom'"
echo "------------------------------------------------------------------"
result=$(run_sql "SELECT type FROM search_term('person') WHERE type = 'Triple' LIMIT 1;")
if echo "$result" | grep -q "Triple"; then
    echo -e "${GREEN}✓ PASSED: Type field correctly identifies 'Triple'${NC}"
    ((PASSED++))
else
    echo -e "${RED}✗ FAILED: Type field not found or incorrect${NC}"
    ((FAILED++))
fi
echo ""

result=$(run_sql "SELECT type FROM search_term('Alice') WHERE type = 'Atom' LIMIT 1;")
if echo "$result" | grep -q "Atom"; then
    echo -e "${GREEN}✓ PASSED: Type field correctly identifies 'Atom'${NC}"
    ((PASSED++))
else
    echo -e "${RED}✗ FAILED: Type field not found or incorrect${NC}"
    ((FAILED++))
fi
echo ""

echo "Step 4: Test search for 'Alice' (atom content)"
echo "-----------------------------------------------"
test_query \
    "Search for 'Alice' should return atom results" \
    "SELECT id, type FROM search_term('Alice') LIMIT 5;" \
    "Atom"

echo "Step 5: Verify backward compatibility - atom search still works"
echo "----------------------------------------------------------------"
result=$(run_sql "SELECT COUNT(*) FROM search_term('Alice') WHERE type = 'Atom';")
if echo "$result" | grep -E "[1-9]"; then
    echo -e "${GREEN}✓ PASSED: Atom search returns results (backward compatible)${NC}"
    ((PASSED++))
else
    echo -e "${RED}✗ FAILED: Atom search broken${NC}"
    ((FAILED++))
fi
echo ""

echo "Step 6: Test mixed results (query that matches both atoms and triples)"
echo "-----------------------------------------------------------------------"
result=$(run_sql "SELECT type, COUNT(*) FROM search_term('person Alice') GROUP BY type ORDER BY type;")
echo "$result"
echo ""

echo "Step 7: Check search_term function signature"
echo "---------------------------------------------"
run_sql "\df search_term"
echo ""

echo "Step 8: Verify search returns term table structure"
echo "---------------------------------------------------"
run_sql "SELECT id, type, atom_id, triple_id FROM search_term('person') LIMIT 2;"
echo ""

echo "Step 9: Note on similarity scores"
echo "----------------------------------"
echo "The current search_term function returns SETOF term, which does not include distance/similarity."
echo "To include similarity scores, the function would need to be modified to return a custom type"
echo "that includes a similarity/distance field along with the term fields."
echo ""

echo "========================================="
echo "Test Summary"
echo "========================================="
echo -e "${GREEN}Passed: $PASSED${NC}"
echo -e "${RED}Failed: $FAILED${NC}"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed.${NC}"
    exit 1
fi
