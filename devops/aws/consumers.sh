#!/bin/bash

# Set BASE_DIR to the directory of this script
BASE_DIR="$(cd "$(dirname "$0")" && pwd)"

# Define arrays for environments and consumers
ENVIRONMENTS=(
    "dev-base-sepolia"
    "dev-base-mainnet"
    "prod-base-sepolia-v2"
    "prod-base-mainnet-v2"
    "prod-linea-mainnet"
    "prod-linea-mainnet-v2"
    "prod-linea-sepolia"
)

CONSUMERS=(
    "raw"
    "decoded"
    "resolver"
    "ipfs-upload"
)

# Ensure a valid environment is provided
if [ $# -lt 1 ]; then
    echo "Usage: $0 <environment>"
    echo "Valid environments: ${ENVIRONMENTS[*]}"
    exit 1
fi

ENV="$1"
valid_env=0
for e in "${ENVIRONMENTS[@]}"; do
    if [ "$e" == "$ENV" ]; then
        valid_env=1
        break
    fi
done

if [ $valid_env -eq 0 ]; then
    echo "Error: '$ENV' is not a valid environment."
    echo "Valid environments: ${ENVIRONMENTS[*]}"
    exit 1
fi

# Iterate over consumers and apply overlays, if the directory exists
for consumer in "${CONSUMERS[@]}"; do
    # Build the complete path to the overlay directory
    consumer_dir="$BASE_DIR/consumers/overlays/$ENV/$consumer"
    
    if [ ! -d "$consumer_dir" ]; then
        echo "Warning: Directory '$consumer_dir' not found. Skipping consumer '$consumer'."
        continue
    fi

    echo "Creating consumer: '$consumer' for environment: '$ENV'"
    kubectl apply -k "$consumer_dir"
done

