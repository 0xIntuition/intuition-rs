FROM node:20-slim

# Install build essentials and git
RUN apt-get update && apt-get install -y \
    postgresql-client \
    jq \
    build-essential \
    git \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install Foundry
RUN curl -L https://foundry.paradigm.xyz | bash && \
    /root/.foundry/bin/foundryup

# Add Foundry to PATH
ENV PATH="/root/.foundry/bin:${PATH}"

WORKDIR /app

RUN git clone --recurse-submodules -j8 https://github.com/0xIntuition/intuition-beta-contracts.git . 
RUN npm install
RUN forge build

# Create a script to run the forge  command
RUN echo '#!/bin/sh\n\
forge script script/DeployToLocalGeth.s.sol --rpc-url http://geth:8545 --keystore /data/keystore.json --password-file /data/password.txt --broadcast\n\
MULTIVAULT_ADDRESS=$(jq -r '\''.transactions[] | select(.contractName=="TransparentUpgradeableProxy") | .contractAddress'\'' ./broadcast/DeployToLocalGeth.s.sol/1337/run-latest.json)\n\
PGPASSWORD=$POSTGRES_PASSWORD psql -h database -U $POSTGRES_USER -d $POSTGRES_DB -p $POSTGRES_PORT -c "UPDATE histocrawler.app_config SET contract_address = '\''$MULTIVAULT_ADDRESS'\'', start_block = 0 WHERE indexer_schema = '\''histo_base_sepolia_1_5'\''"' > /app/deploy.sh && chmod +x /app/deploy.sh

# Set the entrypoint to the script
ENTRYPOINT ["/app/deploy.sh"]