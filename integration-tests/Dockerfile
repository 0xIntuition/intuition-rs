FROM node:20-slim

# Install pnpm
RUN npm install -g pnpm@9.15.3

# Set working directory
WORKDIR /app

# Copy package files
COPY package.json pnpm-lock.yaml ./

# Install dependencies
RUN pnpm install

# Copy the rest of the application
COPY . .

# Environment variables for services (defaults that can be overridden)
ENV ETHEREUM_RPC_URL=http://host.docker.internal:8545
ENV GRAPHQL_API_URL=http://host.docker.internal:8080/v1/graphql
ENV IPFS_API_URL=http://host.docker.internal:3000/upload_json_to_ipfs

# Create a startup script
RUN echo '#!/bin/sh \n\
sed -i "s|http://127.0.0.1:8545|${ETHEREUM_RPC_URL}|g" $(find ./src -type f -name "*.ts") \n\
sed -i "s|http://localhost:8080/v1/graphql|${GRAPHQL_API_URL}|g" $(find ./src -type f -name "*.ts") \n\
sed -i "s|http://localhost:3000/upload_json_to_ipfs|${IPFS_API_URL}|g" $(find ./src -type f -name "*.ts") \n\
echo "Running tests with the following configuration:" \n\
echo "Ethereum RPC URL: ${ETHEREUM_RPC_URL}" \n\
echo "GraphQL API URL: ${GRAPHQL_API_URL}" \n\
echo "IPFS API URL: ${IPFS_API_URL}" \n\
exec pnpm vitest run --no-file-parallelism "$@"' > /app/start.sh && chmod +x /app/start.sh

# Run the tests
ENTRYPOINT ["/app/start.sh"] 
