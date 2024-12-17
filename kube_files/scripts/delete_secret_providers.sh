#! /bin/bash

echo "Deleting secret provider class..."
kubectl delete secretproviderclass hasura-aws-secrets -n default
kubectl delete secretproviderclass substreams-aws-secrets -n default
kubectl delete secretproviderclass resolver-aws-secrets -n default
kubectl delete secretproviderclass ipfs-upload-aws-secrets -n default
kubectl delete secretproviderclass raw-aws-secrets -n default
kubectl delete secretproviderclass decoded-aws-secrets -n default

echo "Done!"
