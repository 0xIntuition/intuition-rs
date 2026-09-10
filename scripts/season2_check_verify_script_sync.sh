#!/bin/sh
# GWTH-4354: drift guard for the embedded migration copies inside the
# season2 verification scripts.
#
# scripts/season2_verify_leaderboard_cutoff.sql embeds both migration files
# verbatim (see that script's own header for why: it runs via `psql -f -`
# piped over `kubectl exec -i` stdin, where a `\i` include would try to open
# a path that does not exist on the pod's filesystem). Nothing at parse time
# enforces that those embedded copies stay in sync with the real migration
# files when either changes -- this script is that enforcement.
#
# This is a standalone script, run by hand. It is NOT wired into CI -- this
# repo has no DB-backed CI yet (see docs/leaderboard-wind-down-epoch-20.md).
#
# Usage:
#   scripts/season2_check_verify_script_sync.sh
#
# Exits 0 and prints an OK line per file if both embedded copies match the
# real migration files byte-for-byte. Exits non-zero with a diff and a clear
# message otherwise.

set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
verify_script="$repo_root/scripts/season2_verify_leaderboard_cutoff.sql"
migration_dir="$repo_root/infrastructure/hasura/migrations/intuition/1771526444000_leaderboard_wind_down_epoch_20"
up_sql="$migration_dir/up.sql"
down_sql="$migration_dir/down.sql"
temp_tables_script="$repo_root/scripts/season2_verify_leaderboard_period_temp_tables.sql"
temp_tables_migration_dir="$repo_root/infrastructure/hasura/migrations/intuition/1771526445000_fix_pnl_leaderboard_period_temp_tables"
temp_tables_up_sql="$temp_tables_migration_dir/up.sql"

if [ ! -f "$verify_script" ]; then
  echo "FAIL: verify script not found: $verify_script" >&2
  exit 2
fi
if [ ! -f "$up_sql" ]; then
  echo "FAIL: migration up.sql not found: $up_sql" >&2
  exit 2
fi
if [ ! -f "$down_sql" ]; then
  echo "FAIL: migration down.sql not found: $down_sql" >&2
  exit 2
fi

if [ ! -f "$temp_tables_script" ]; then
  echo "FAIL: temp-tables verify script not found: $temp_tables_script" >&2
  exit 2
fi
if [ ! -f "$temp_tables_up_sql" ]; then
  echo "FAIL: temp-tables migration up.sql not found: $temp_tables_up_sql" >&2
  exit 2
fi

tmp_dir=$(mktemp -d)
trap 'rm -rf "$tmp_dir"' EXIT INT TERM

status=0

# extract_block start_marker end_marker out_file
# Prints the lines strictly between (not including) the two exact-match
# marker lines found in $verify_script to out_file. Fails loudly (exit 2)
# if either marker is missing, rather than silently diffing an empty block.
extract_block() {
  script_file="$1"
  start_marker="$2"
  end_marker="$3"
  out_file="$4"
  awk -v start="$start_marker" -v end="$end_marker" '
    $0 == start { found_start = 1; in_block = 1; next }
    $0 == end   { found_end = 1; in_block = 0 }
    in_block    { print }
    END {
      if (!found_start) { print "MISSING_START_MARKER" > "/dev/stderr"; exit 2 }
      if (!found_end)   { print "MISSING_END_MARKER" > "/dev/stderr"; exit 2 }
    }
  ' "$script_file" > "$out_file"
}

# check_one script_file label start_marker end_marker real_file
check_one() {
  script_file="$1"
  label="$2"
  start_marker="$3"
  end_marker="$4"
  real_file="$5"
  embedded_file="$tmp_dir/embedded-$label"

  if ! extract_block "$script_file" "$start_marker" "$end_marker" "$embedded_file"; then
    echo "FAIL: could not find the $label sentinel markers in $(basename "$script_file")" >&2
    echo "      expected to find these two lines, unindented, exactly as shown:" >&2
    echo "        $start_marker" >&2
    echo "        $end_marker" >&2
    status=1
    return
  fi

  if diff -u "$real_file" "$embedded_file" > "$tmp_dir/diff-$label"; then
    echo "OK: embedded $label copy in $(basename "$script_file") matches $real_file"
  else
    echo "FAIL: embedded $label copy in $(basename "$script_file") has drifted from $real_file" >&2
    echo "      re-sync the embedded block from $real_file (verbatim, between the" >&2
    echo "      BEGIN/END EMBEDDED sentinel comments) and re-run this script." >&2
    cat "$tmp_dir/diff-$label" >&2
    status=1
  fi
}

check_one "$verify_script" "up.sql" \
  "-- BEGIN EMBEDDED up.sql (verbatim - keep byte-identical, see scripts/season2_check_verify_script_sync.sh)" \
  "-- END EMBEDDED up.sql" \
  "$up_sql"

check_one "$verify_script" "down.sql" \
  "-- BEGIN EMBEDDED down.sql (verbatim - keep byte-identical, see scripts/season2_check_verify_script_sync.sh)" \
  "-- END EMBEDDED down.sql" \
  "$down_sql"

check_one "$temp_tables_script" "1771526445000-up.sql" \
  "-- BEGIN EMBEDDED 1771526445000/up.sql (verbatim - keep byte-identical, see scripts/season2_check_verify_script_sync.sh)" \
  "-- END EMBEDDED 1771526445000/up.sql" \
  "$temp_tables_up_sql"

exit $status
