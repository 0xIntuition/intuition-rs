#!/usr/bin/env bash
#
# Backfill ENS resolution for accounts that were never sent to the resolver stream.
#
# Context: Between commit 0841ec2 (Dec 2025) and the fix that introduced this
# script, `create_default_account` stopped enqueueing resolver messages. Accounts
# created via Deposited/Redeemed/AtomCreated/ProtocolFeeAccrued event handlers
# were persisted with short-id labels (e.g. `0x1234...5678`) but never sent to
# the resolver, so their ENS reverse records were never looked up.
#
# This script enqueues resolver messages for all affected accounts. It must be
# run once per environment after deploying the code fix.
#
# Usage:
#   # 1. Port-forward the environment's Redis and Postgres
#   kubectl -n <env> port-forward svc/<env>-redis 6379:6379 &
#   kubectl -n <env> port-forward svc/<env>-timescale-db 5432:5432 &
#
#   # 2. Export credentials
#   export PGPASSWORD='...'
#   export DB_HOST=localhost DB_PORT=5432 DB_USER=intuition_admin DB_NAME=storage
#   export REDIS_HOST=localhost REDIS_PORT=6379
#
#   # 3. Dry-run first to see how many accounts will be enqueued
#   DRY_RUN=1 ./backfill_account_ens.sh
#
#   # 4. Run for real
#   ./backfill_account_ens.sh
#
# Throughput tuning:
#   CHUNK_SIZE:  how many accounts to enqueue per redis-cli --pipe batch (default 5000).
#                Larger = faster, but each chunk is atomic w.r.t. progress reporting.
#   CHUNK_SLEEP: seconds to sleep between chunks (default 0). Increase if you
#                need to pace the resolver consumer downstream of Redis.
#   BATCH_LIMIT: cap on total accounts to enqueue in this run (default: no limit).
#                Useful for staged rollouts.
#
# Note on throughput: we use `redis-cli --pipe` (RESP protocol bulk loader) which
# is the documented tool for mass-insertion. A single-command XADD-per-exec loop
# would bottleneck on redis-cli startup overhead (~50ms each = ~20 msgs/s).

set -euo pipefail

: "${DB_HOST:=localhost}"
: "${DB_PORT:=5432}"
: "${DB_USER:=intuition_admin}"
: "${DB_NAME:=storage}"
: "${REDIS_HOST:=localhost}"
: "${REDIS_PORT:=6379}"
: "${STREAM:=resolver_stream}"
: "${CHUNK_SIZE:=5000}"
: "${CHUNK_SLEEP:=0}"
: "${DRY_RUN:=0}"
: "${BATCH_LIMIT:=0}"

command -v psql >/dev/null || { echo "psql not found" >&2; exit 1; }
command -v redis-cli >/dev/null || { echo "redis-cli not found" >&2; exit 1; }

# Query: accounts that were created without being enqueued for ENS resolution.
# Conditions:
#   - type = 'Default' (real users, not ProtocolVault/AtomWallet)
#   - label matches the short-id fallback pattern `0x……...……`
#   - atom_id IS NULL (the atom_id path always enqueued — not affected by the bug)
#   - id != zero address (pointless to resolve ENS for the burn address)
ZERO_ADDR="0x0000000000000000000000000000000000000000"
read -r -d '' QUERY <<SQL || true
SELECT count(*)
FROM account
WHERE type = 'Default'
  AND atom_id IS NULL
  AND label LIKE '0x%...%'
  AND id != '$ZERO_ADDR';
SQL

TOTAL=$(psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -At -c "$QUERY")
echo "Accounts needing backfill: $TOTAL"

if [[ "$DRY_RUN" == "1" ]]; then
  echo "DRY_RUN=1, exiting without enqueueing."
  exit 0
fi

if [[ "$TOTAL" -eq 0 ]]; then
  echo "Nothing to do."
  exit 0
fi

echo "Enqueueing to stream '$STREAM' at $REDIS_HOST:$REDIS_PORT"
echo "Chunk size: $CHUNK_SIZE, sleep between chunks: ${CHUNK_SLEEP}s"

# (per-chunk query is assembled inside the loop below)

# Encodes `XADD <stream> * body <msg>` as RESP (redis wire protocol).
# Reads messages on stdin (one JSON per line) and writes RESP frames to stdout.
# `\r\n` line endings are required by the RESP spec.
resp_encode_xadd() {
  local stream="$1"
  awk -v stream="$stream" '
    BEGIN {
      # Pre-compute the fixed prefix common to every XADD command:
      #   *5\r\n $4\r\nXADD\r\n $N\r\n<stream>\r\n $1\r\n*\r\n $4\r\nbody\r\n
      prefix = "*5\r\n" \
               "$4\r\nXADD\r\n" \
               "$" length(stream) "\r\n" stream "\r\n" \
               "$1\r\n*\r\n" \
               "$4\r\nbody\r\n"
    }
    length($0) > 0 {
      # Append the per-message body argument: $<len>\r\n<json>\r\n
      printf "%s$%d\r\n%s\r\n", prefix, length($0), $0
    }
  '
}

total_enqueued=0
start_epoch=$(date +%s)
offset=0

# Enqueue in chunks so progress is observable and failures are localized.
# Each chunk is a single `redis-cli --pipe` invocation streaming CHUNK_SIZE
# XADDs over one TCP connection via the RESP bulk-load protocol.
while :; do
  chunk_query=$(cat <<SQL
SELECT json_build_object(
  'message', json_build_object(
    'Account', json_build_object(
      'id',           id,
      'atom_id',      atom_id,
      'label',        label,
      'image',        image,
      'account_type', type::text
    )
  )
)::text
FROM account
WHERE type = 'Default'
  AND atom_id IS NULL
  AND label LIKE '0x%...%'
  AND id != '$ZERO_ADDR'
ORDER BY id
OFFSET $offset
LIMIT $CHUNK_SIZE;
SQL
)
  # Count rows in this chunk before piping, so we know when to stop.
  chunk=$(psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -At -c "$chunk_query")
  chunk_count=$(printf '%s\n' "$chunk" | grep -c '^{' || true)
  [[ "$chunk_count" -eq 0 ]] && break

  printf '%s\n' "$chunk" \
    | resp_encode_xadd "$STREAM" \
    | redis-cli -h "$REDIS_HOST" -p "$REDIS_PORT" --pipe >/dev/null

  total_enqueued=$((total_enqueued + chunk_count))
  offset=$((offset + CHUNK_SIZE))

  elapsed=$(($(date +%s) - start_epoch))
  rate=$((total_enqueued / (elapsed == 0 ? 1 : elapsed)))
  echo "  enqueued=$total_enqueued / $TOTAL elapsed=${elapsed}s rate=${rate}/s"

  if [[ "$BATCH_LIMIT" -gt 0 && "$total_enqueued" -ge "$BATCH_LIMIT" ]]; then
    echo "  hit BATCH_LIMIT=$BATCH_LIMIT, stopping."
    break
  fi

  if (( $(awk "BEGIN { print ($CHUNK_SLEEP > 0) }") )); then
    sleep "$CHUNK_SLEEP"
  fi
done

elapsed=$(($(date +%s) - start_epoch))
echo "Done. Enqueued $total_enqueued messages in ${elapsed}s."
