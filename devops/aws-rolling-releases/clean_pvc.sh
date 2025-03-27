#!/bin/bash
# Script to delete all PVCs except for prometheus-data, ipfs-data, and ipfs-data-new

# PVCs to exclude
EXCLUDE=("prometheus-data" "ipfs-data" "ipfs-data-new")

# Get the PVC names; using --no-headers to skip the header line
PVC_LIST=$(kubectl get pvc --no-headers -o custom-columns=name:.metadata.name)

for pvc in $PVC_LIST; do
    skip=false
    for exc in "${EXCLUDE[@]}"; do
        if [[ "$pvc" == "$exc" ]]; then
            echo "Skipping PVC: $pvc"
            skip=true
            break
        fi
    done

    if [ "$skip" == true ]; then
        continue
    fi

    echo "Deleting PVC: $pvc"
    kubectl delete pvc "$pvc"
done
