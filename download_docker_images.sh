#!/bin/bash

set -euo pipefail

IMAGES=(
    "ghcr.io/0xintuition/hasura-migrations:latest"
    "ghcr.io/0xintuition/consumer:latest"
    "ghcr.io/0xintuition/image-guard:latest"
    "ghcr.io/0xintuition/substreams-sink:latest"
    "ghcr.io/0xintuition/cli:latest"
    "ghcr.io/0xintuition/histoflux:latest"
    "ghcr.io/0xintuition/envio-indexer:latest"
    "ghcr.io/0xintuition/rpc-proxy:latest"
)

echo "Starting docker images download..."

for image in "${IMAGES[@]}"; do
    echo "Pulling $image..."
    if ! docker pull "$image"; then
        echo "Failed to pull $image"
        exit 1
    fi
done

echo "Building indexer and cache migrations..."
if ! cargo make build-indexer-and-cache-migrations; then
    echo "Failed to build indexer and cache migrations"
    exit 1
fi

echo "All operations completed successfully"
