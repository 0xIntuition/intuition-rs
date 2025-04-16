#!/bin/bash
source .env

# Start shared services
docker compose -f docker-compose-shared.yml up -d --wait --force-recreate



# Select contract_address from histocrawler.app_config
CONTRACT_ADDRESS=$(docker compose -f docker-compose-shared.yml exec database psql -U testuser -d storage -c "SELECT contract_address FROM histocrawler.app_config WHERE indexer_schema = 'histo_base_sepolia_1_5'" -tA)
echo "Contract address: $CONTRACT_ADDRESS"

# Set env vars
export INTUITION_CONTRACT_ADDRESS=$CONTRACT_ADDRESS

# Start apps
docker compose -f docker-compose-apps.yml up -d --force-recreate
