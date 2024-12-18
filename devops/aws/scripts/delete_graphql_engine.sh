#! /bin/bash

echo "Deleting graphql-engine deployment..."
kubectl delete deployment graphql-engine

echo "Deleting hasura-migrations job..."
kubectl delete job hasura-migrations

echo "Done!"
