#!/bin/bash
# Usage: ./nuke.sh <env_prefix>
# Defaults to "dev-base-sepolia" if no argument is provided.

ENV_PREFIX=${1:-dev-base-sepolia}

echo "You are about to delete all components for environment: ${ENV_PREFIX}"
read -p "Are you sure you want to proceed? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]
then
    echo "Operation cancelled"
    exit 1
fi

echo "Deleting GraphQL components..."
kubectl delete deployment ${ENV_PREFIX}-graphql-engine
kubectl delete job ${ENV_PREFIX}-hasura-migrations
kubectl delete job ${ENV_PREFIX}-indexer-and-cache-migration

echo "Deleting Consumers..."
kubectl delete deployment ${ENV_PREFIX}-decoded-consumer
kubectl delete deployment ${ENV_PREFIX}-raw-consumer
kubectl delete deployment ${ENV_PREFIX}-resolver-consumer
kubectl delete deployment ${ENV_PREFIX}-ipfs-upload-consumer

echo "Deleting Histoflux deployment..."
kubectl delete deployment ${ENV_PREFIX}-histoflux

