FROM ghcr.io/foundry-rs/foundry:stable as foundry

FROM node:20-slim

# Install build essentials and git
RUN apt-get update && apt-get install -y \
    build-essential \
    git \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy Foundry binaries and dependencies from the foundry stage
COPY --from=foundry /usr/local/bin/anvil /usr/local/bin/
COPY --from=foundry /usr/local/bin/cast /usr/local/bin/
COPY --from=foundry /usr/local/bin/forge /usr/local/bin/
COPY --from=foundry /usr/local/bin/chisel /usr/local/bin/

WORKDIR /app

RUN git clone --recurse-submodules -j8 https://github.com/0xIntuition/intuition-beta-contracts.git . 
RUN git checkout simon/deploy-local-geth 
RUN npm install
RUN forge build

# Create a script to run the forge command
RUN echo '#!/bin/sh\n\
forge script script/DeployToLocalGeth.s.sol --rpc-url http://geth:8545 --keystore /data/keystore.json --password-file /data/password.txt --broadcast\n\
' > /app/deploy.sh && chmod +x /app/deploy.sh

# Set the entrypoint to the script
ENTRYPOINT ["/app/deploy.sh"]