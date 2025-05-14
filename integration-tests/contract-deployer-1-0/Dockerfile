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

RUN git clone --branch feat/v1-0-local-deploy --depth 1 --recurse-submodules -j8 https://github.com/0xIntuition/intuition-beta-contracts.git . 
RUN npm install
RUN forge build --via-ir --optimize

# Create a script to run the forge  command
RUN echo '#!/bin/sh\n\
forge script script/DeployWithoutOZ.s.sol --via-ir --optimize --rpc-url http://geth:8545 --keystore /data/keystore.json --password-file /data/password.txt --broadcast\n\
MULTIVAULT_ADDRESS=$(jq -r '\''.transactions[] | select(.contractName=="TransparentUpgradeableProxy") | .contractAddress'\'' ./broadcast/DeployWithoutOZ.s.sol/1337/run-latest.json | head -n 1)\n\
PGPASSWORD=$POSTGRES_PASSWORD psql -h database -U $POSTGRES_USER -d $POSTGRES_DB -p $POSTGRES_PORT -c "UPDATE histocrawler.app_config SET contract_address = '\''$MULTIVAULT_ADDRESS'\'', start_block = 0 WHERE indexer_schema = '\''histo_base_sepolia_1_5'\''"' > /app/deploy.sh && chmod +x /app/deploy.sh

RUN echo '#!/bin/sh\n\
forge script script/DeployWithoutOZ.s.sol --via-ir --optimize --rpc-url http://geth:8545 --keystore /data/keystore.json --password-file /data/password.txt --broadcast --resume --verify --verifier blockscout --verifier-url http://host.docker.internal/api/\n' > /app/verify.sh && chmod +x /app/verify.sh

# Set the entrypoint to the script
ENTRYPOINT ["/app/deploy.sh"]