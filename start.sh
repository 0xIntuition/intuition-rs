#!/bin/bash
source .env

# Start shared services
docker compose -f docker-compose-shared.yml up database sqs ipfs safe-content graphql-engine local-migrations indexer-migrations hasura-migrations prometheus -d --wait --force-recreate


# First arg is indexer schema
INDEXER_SCHEMA="$1"
CONTRACT_ADDRESS=$(docker compose -f docker-compose-shared.yml exec database psql -U testuser -d storage -c "SELECT contract_address FROM histocrawler.app_config WHERE indexer_schema = '$INDEXER_SCHEMA'" -tA)
if [ -n "$CONTRACT_ADDRESS" ]; then
    echo "Contract address: $CONTRACT_ADDRESS"
    export INTUITION_CONTRACT_ADDRESS=$CONTRACT_ADDRESS
    export INDEXER_SCHEMA=$INDEXER_SCHEMA
fi

# If started with arg histo_local_1_5 deploy contract to local geth and get contract address
if [ "$INDEXER_SCHEMA" == "histo_local_1_5" ]; then
    docker compose -f docker-compose-shared.yml up contract-deployer geth -d --wait --force-recreate

    # Select contract_address from histocrawler.app_config wait until it changes from 0x63B90A9c109fF8f137916026876171ffeEdEe714 or empty
    while [ "$CONTRACT_ADDRESS" == "0x63B90A9c109fF8f137916026876171ffeEdEe714" ] || [ -z "$CONTRACT_ADDRESS" ]; do
        CONTRACT_ADDRESS=$(docker compose -f docker-compose-shared.yml exec database psql -U testuser -d storage -c "SELECT contract_address FROM histocrawler.app_config WHERE indexer_schema = 'histo_base_sepolia_1_5'" -tA)
        sleep 1
    done

    echo "export VITE_INTUITION_CONTRACT_ADDRESS=$CONTRACT_ADDRESS"

    # Set env vars
    export INTUITION_CONTRACT_ADDRESS=$CONTRACT_ADDRESS
    export INDEXER_SCHEMA="histo_base_sepolia_1_5"
    export BASE_SEPOLIA_RPC_URL="http://geth:8545"
    export BASE_MAINNET_RPC_URL="http://geth:8545"
fi


if [ "$2" == "test" ]; then
    echo "Starting integration tests"
    docker compose -f docker-compose-apps.yml up integration-tests -d --force-recreate
fi

# Start apps
docker compose -f docker-compose-apps.yml up raw_consumer resolver_consumer consumer-api ipfs_upload_consumer decoded_consumer api prod-rpc-proxy histocrawler histoflux -d --force-recreate
