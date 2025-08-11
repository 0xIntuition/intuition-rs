#!/bin/bash

# Clone the repo locally
rm -rf intuition-contracts-v2
git clone --branch deploy-to-geth-3 --depth 1 git@github.com:0xIntuition/intuition-contracts-v2

# Build the Docker image
docker build -f integration-tests/contract-deployer-2-0/Dockerfile.copy -t ghcr.io/0xintuition/contract-deployer-2-0:latest .

# Clean up
rm -rf intuition-contracts-v2
