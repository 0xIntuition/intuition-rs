#!/bin/bash
# Re-queues all accounts into the resolver stream so TNS names get refreshed.

DB_CONTAINER="database"
REDIS_CONTAINER="redis"
REDIS_STREAM="resolver_stream"
SCHEMA="public"

echo "Fetching all accounts from database..."

# Fetch accounts as: id|atom_id|label|image|account_type
accounts=$(docker exec "$DB_CONTAINER" psql -U postgres -d storage -t -A -F'|' \
  -c "SELECT id, COALESCE(atom_id, ''), label, COALESCE(image, ''), type FROM $SCHEMA.account;")

count=0
while IFS='|' read -r id atom_id label image account_type; do
  [[ -z "$id" ]] && continue

  # Build atom_id JSON value
  if [[ -n "$atom_id" ]]; then
    atom_id_json="\"$atom_id\""
  else
    atom_id_json="null"
  fi

  # Build image JSON value
  if [[ -n "$image" ]]; then
    escaped_image=$(echo "$image" | sed 's/"/\\"/g')
    image_json="\"$escaped_image\""
  else
    image_json="null"
  fi

  message="{\"message\":{\"Account\":{\"id\":\"$id\",\"atom_id\":$atom_id_json,\"label\":\"$label\",\"image\":$image_json,\"account_type\":\"$account_type\"}}}"

  docker exec "$REDIS_CONTAINER" redis-cli XADD "$REDIS_STREAM" '*' body "$message" > /dev/null
  echo "  Queued: $id"
  count=$((count + 1))
done <<< "$accounts"

echo "Done. Queued $count account(s) for re-resolution."
