#!/usr/bin/env bash
set -euo pipefail

# Settle a Season 2 epoch and optionally finalize it.
#
# Requirements:
# - DATABASE_URL environment variable
# - psql available

EPOCH=""
FORCE="false"
FINALIZE="false"

usage() {
  cat <<EOF
Usage: $(basename "$0") --epoch EPOCH [--force] [--finalize]

Options:
  --epoch      Season 2 epoch number (8..30)
  --force      Recompute by deleting existing epoch ledger entries before settlement
  --finalize   Finalize epoch immediately after successful settlement
  -h, --help   Show this help

Environment:
  DATABASE_URL PostgreSQL connection string (required)
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --epoch)
      EPOCH="${2:-}"
      shift 2
      ;;
    --force)
      FORCE="true"
      shift
      ;;
    --finalize)
      FINALIZE="true"
      shift
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

if [[ -z "$EPOCH" ]]; then
  echo "--epoch is required" >&2
  usage
  exit 1
fi

if ! [[ "$EPOCH" =~ ^[0-9]+$ ]]; then
  echo "Epoch must be an integer" >&2
  exit 1
fi

if (( EPOCH < 8 || EPOCH > 30 )); then
  echo "Epoch must be in range 8..30 for Season 2" >&2
  exit 1
fi

SETTLE_SQL="SELECT * FROM settle_season2_epoch(${EPOCH}, ${FORCE});"
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -c "$SETTLE_SQL"

if [[ "$FINALIZE" == "true" ]]; then
  FINALIZE_SQL="SELECT * FROM finalize_season2_epoch(${EPOCH});"
  psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -c "$FINALIZE_SQL"
fi

echo "Done: epoch=${EPOCH} force=${FORCE} finalize=${FINALIZE}"
