#!/bin/bash

# Query statistics script for TimescaleDB
# Usage: ./scripts/query-stats.sh [command]
# Commands: top, slow, reset, all

# Use docker exec to connect to the database container
DB_CMD="docker exec database psql -U postgres -d storage -c"

case "${1:-all}" in
  "top")
    echo "=== Top 10 Queries by Total Execution Time ==="
    $DB_CMD "SELECT query, calls, total_exec_time, mean_exec_time, min_exec_time, max_exec_time, rows FROM pg_stat_statements ORDER BY total_exec_time DESC LIMIT 10;"
    ;;
  "slow")
    echo "=== Slow Queries (avg > 1s) ==="
    $DB_CMD "SELECT query, calls, mean_exec_time, min_exec_time, max_exec_time, rows FROM pg_stat_statements WHERE mean_exec_time > 1000 ORDER BY mean_exec_time DESC;"
    ;;
  "reset")
    echo "=== Resetting Query Statistics ==="
    $DB_CMD "SELECT pg_stat_statements_reset();"
    echo "Statistics reset!"
    ;;
  "all")
    echo "=== Query Statistics Overview ==="
    $DB_CMD "SELECT query, calls, total_exec_time, mean_exec_time, min_exec_time, max_exec_time, rows FROM pg_stat_statements ORDER BY total_exec_time DESC LIMIT 20;"
    echo ""
    echo "=== Database Activity ==="
    $DB_CMD "SELECT datname, numbackends, xact_commit, xact_rollback, blks_read, blks_hit FROM pg_stat_database WHERE datname = 'storage';"
    ;;
  *)
    echo "Usage: $0 [top|slow|reset|all]"
    echo "  top   - Show top 10 queries by execution time"
    echo "  slow  - Show slow queries (avg > 1s)"
    echo "  reset - Reset query statistics"
    echo "  all   - Show overview (default)"
    ;;
esac
