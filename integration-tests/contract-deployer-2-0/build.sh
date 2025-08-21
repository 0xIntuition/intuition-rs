#!/bin/bash

# Clone the repo locally
rm -rf intuition-contracts-v2
git clone --depth 1 git@github.com:0xIntuition/intuition-v2-sol-refactor

cp intuition-v2-sol-refactor/.env.example intuition-v2-sol-refactor/.env
# Build the Docker image
docker build -f integration-tests/contract-deployer-2-0/Dockerfile.copy -t ghcr.io/0xintuition/contract-deployer-2-0:latest .

# Clean up
rm -rf intuition-v2-sol-refactor
