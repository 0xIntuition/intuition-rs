#!/bin/bash

# Clone the repo locally
rm -rf intuition-contracts-v2
git clone --depth 1 --branch multivault-migration-mode-script-changes git@github.com:0xIntuition/intuition-contracts-v2.git 

cp intuition-contracts-v2/.env.example intuition-contracts-v2/.env
# Build the Docker image
docker build -f integration-tests/contract-deployer-2-0/Dockerfile.copy -t ghcr.io/0xintuition/contract-deployer-2-0:latest .

# Clean up
rm -rf intuition-contracts-v2
