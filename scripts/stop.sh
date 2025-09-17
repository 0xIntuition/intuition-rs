#!/bin/bash

# Set dummy environment variables to avoid warnings when stopping
export PINATA_API_JWT="dummy"
export PINATA_GATEWAY_TOKEN="dummy"
export OPENAI_API_KEY="dummy"
export BASE_MAINNET_RPC_URL="dummy"
export BASE_SEPOLIA_RPC_URL="dummy"
export ETHEREUM_MAINNET_RPC_URL="dummy"
export LINEA_MAINNET_RPC_URL="dummy"
export LINEA_SEPOLIA_RPC_URL="dummy"
export TRUST_TESTNET_RPC_URL="dummy"
export TRUST_MAINNET_RPC_URL="dummy"
export INDEXER_SCHEMA="dummy"
export INTUITION_CONTRACT_ADDRESS="dummy"
export INITIAL_CONTRACT_VERSION="dummy"

# Stop blockscout
docker compose -f infrastructure/blockscout/docker-compose.yml down --volumes

# Stop intuition services
docker compose -p intuition -f docker/docker-compose-shared.yml down --volumes
docker compose -p intuition -f docker/docker-compose-apps.yml down --volumes