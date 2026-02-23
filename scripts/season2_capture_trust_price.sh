#!/usr/bin/env bash
set -euo pipefail

# Capture TRUST/USD snapshot from CoinGecko and upsert into season2_trust_price_snapshot.
#
# Defaults:
# - CoinGecko coin id: intuition
# - Source: coingecko
# - Snapshot time: current UTC rounded down to 00:00 or 12:00
#
# Requirements:
# - DATABASE_URL environment variable
# - curl, jq, psql available

COIN_ID="intuition"
VS_CURRENCY="usd"
SOURCE="coingecko"
SNAPSHOT_AT=""

usage() {
  cat <<EOF
Usage: $(basename "$0") [--snapshot-at RFC3339] [--coin-id COIN_ID]

Options:
  --snapshot-at   Explicit snapshot timestamp (RFC3339, e.g. 2026-02-24T12:00:00Z)
  --coin-id       CoinGecko coin id (default: intuition)
  -h, --help      Show this help

Environment:
  DATABASE_URL    PostgreSQL connection string (required)
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --snapshot-at)
      SNAPSHOT_AT="${2:-}"
      shift 2
      ;;
    --coin-id)
      COIN_ID="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "DATABASE_URL is required" >&2
  exit 1
fi

if [[ -z "$SNAPSHOT_AT" ]]; then
  UTC_DAY="$(date -u +"%Y-%m-%d")"
  UTC_HOUR="$(date -u +"%H")"
  if (( 10#$UTC_HOUR < 12 )); then
    SNAPSHOT_AT="${UTC_DAY}T00:00:00Z"
  else
    SNAPSHOT_AT="${UTC_DAY}T12:00:00Z"
  fi
fi

PRICE_URL="https://api.coingecko.com/api/v3/simple/price?ids=${COIN_ID}&vs_currencies=${VS_CURRENCY}"
PRICE_USD="$(curl -fsSL "$PRICE_URL" | jq -er --arg coin "$COIN_ID" '.[$coin].usd')"

if [[ -z "$PRICE_USD" ]]; then
  echo "Failed to fetch TRUST/USD price from CoinGecko" >&2
  exit 1
fi

SQL="
SELECT snapshot_at, price_usd, source, source_ref
FROM upsert_season2_trust_price_snapshot(
  '${SNAPSHOT_AT}'::timestamptz,
  ${PRICE_USD}::numeric,
  '${SOURCE}',
  '${PRICE_URL}'
);
"

psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -c "$SQL"

echo "Captured ${COIN_ID^^}/${VS_CURRENCY^^} snapshot at ${SNAPSHOT_AT}: ${PRICE_USD}"
