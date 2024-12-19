#! /bin/bash

echo "Deleting substreams-sink deployment..."
kubectl delete deployment substreams-sink
kubectl delete pvc substreams-data

echo "Done!"
