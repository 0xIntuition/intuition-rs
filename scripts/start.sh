#!/bin/bash
source .env

# Start shared services
docker compose -f docker/docker-compose-shared.yml up database drizzle-studio pgai-installer vectorizer-worker redis redis-setup ipfs safe-content graphql-engine local-migrations indexer-migrations hasura-migrations prometheus  -d --wait --force-recreate

export INITIAL_CONTRACT_VERSION="v2"
# First arg is indexer schema
INDEXER_SCHEMA="$1"
CONTRACT_ADDRESS=$(docker compose -f docker/docker-compose-shared.yml exec database psql -U postgres -d storage -c "SELECT contract_address FROM histocrawler.app_config WHERE indexer_schema = '$INDEXER_SCHEMA'" -tA)
if [ -n "$CONTRACT_ADDRESS" ]; then
    export INTUITION_CONTRACT_ADDRESS=$CONTRACT_ADDRESS
    export INDEXER_SCHEMA=$INDEXER_SCHEMA
fi

# If started with arg histo_local_1_5 deploy contract to local geth and get contract address
if [ "$INDEXER_SCHEMA" == "local" ]; then
    docker compose -f docker/docker-compose-shared.yml up contract-deployer-2-0 geth -d --wait 

    docker compose -f infrastructure/blockscout/docker-compose.yml up -d --wait

    docker compose -f docker/docker-compose-shared.yml up contract-verifier-2-0 -d 
    
    # Select contract_address from histocrawler.app_config wait until it changes from 0x63B90A9c109fF8f137916026876171ffeEdEe714 or empty
    while [ "$CONTRACT_ADDRESS" == "0xB4375293a13017BCe71a034bB588786A3D3C7295" ] || [ -z "$CONTRACT_ADDRESS" ]; do
        CONTRACT_ADDRESS=$(docker compose -f docker/docker-compose-shared.yml exec database psql -U postgres -d storage -c "SELECT contract_address FROM histocrawler.app_config WHERE indexer_schema = 'local'" -tA)
        sleep 1
    done
    
    echo -e "\nTo run integration tests in a different terminal, run:"
    echo -e "\n\nexport VITE_INTUITION_CONTRACT_ADDRESS=$CONTRACT_ADDRESS"
    echo "cd integration-tests"
    echo "pnpm test src/create-person.test.ts"

    echo -e "\nExplore the contract on blockscout:"
    echo -e "http://localhost/address/$CONTRACT_ADDRESS?tab=read_write_proxy\n\n"
    
    # Set env vars
    export VITE_INTUITION_CONTRACT_ADDRESS=$CONTRACT_ADDRESS
    export INTUITION_CONTRACT_ADDRESS=$CONTRACT_ADDRESS
    export INDEXER_SCHEMA="local"
    export BASE_SEPOLIA_RPC_URL="http://geth:8545"
    export BASE_MAINNET_RPC_URL="http://geth:8545"
fi

if [ "$2" == "test" ]; then
    echo "Starting integration tests"
    docker compose -f docker/docker-compose-apps.yml up integration-tests -d --force-recreate
fi

# Start apps
docker compose -f docker/docker-compose-apps.yml up resolver_consumer ipfs_upload_consumer decoded_consumer api prod-rpc-proxy histocrawler -d --force-recreate

echo -e "\nGraphQL: http://localhost:8080/console"
echo -e "Database: https://local.drizzle.studio/"
echo -e "Kibana: http://localhost:5601"
echo -e "Elasticsearch: http://localhost:9200"
echo -e "\nTo set up Kibana with pre-configured searches, run: ./setup-kibana.sh\n"
