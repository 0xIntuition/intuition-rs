FROM ubuntu:latest

# Install build essentials and git
RUN apt-get update && apt-get install -y \
    postgresql-client \
    jq \
    build-essential \
    git \
    curl \
    software-properties-common \
    libc6 \
    libstdc++6 \
    nodejs \
    npm \
    && rm -rf /var/lib/apt/lists/*

# Install Foundry
RUN curl -L https://foundry.paradigm.xyz | bash && \
    /root/.foundry/bin/foundryup

# Add Foundry to PATH
ENV PATH="/root/.foundry/bin:${PATH}"

WORKDIR /app

RUN git clone --depth 1 https://github.com/0xIntuition/intuition-contracts-v2 . 
RUN npm install
RUN forge build

# Create a script to run the forge  command
RUN echo '#!/bin/sh\n\
OUTPUT=$(DEPLOYER_LOCAL=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 BROADCASTING=1 ANVIL_ADMIN_ADDRESS=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 ANVIL_CHAIN_ID=31337 forge script script/IntuitionDeploy.s.sol --rpc-url http://reth:8545 --keystore /data/keystore.json --password-file /data/password.txt --broadcast 2>&1)\n\
echo "$OUTPUT"\n\
MULTIVAULT_ADDRESS=$(echo "$OUTPUT" | grep "MultiVault : " | sed -E "s/.*MultiVault : //")\n\
PGPASSWORD=$POSTGRES_PASSWORD psql -h database -U $POSTGRES_USER -d $POSTGRES_DB -p $POSTGRES_PORT -c "UPDATE histocrawler.app_config SET contract_address = '\''$MULTIVAULT_ADDRESS'\'', start_block = 0 WHERE indexer_schema = '\''base_sepolia'\''"' > /app/deploy.sh && chmod +x /app/deploy.sh

RUN echo '#!/bin/sh\n\
forge script script/IntuitionDeploy.s.sol --rpc-url http://reth:8545 --keystore /data/keystore.json --password-file /data/password.txt --broadcast --resume --verify --verifier blockscout --verifier-url http://host.docker.internal/api/\n' > /app/verify.sh && chmod +x /app/verify.sh


# Set the entrypoint to the script
ENTRYPOINT ["/app/deploy.sh"]
