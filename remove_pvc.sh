#!/bin/bash

# Get list of all PV names, skipping the header line
pv_list=$(kubectl get pv | tail -n +2 | awk '{print $1}')

# Check if any PVs were found
if [ -z "$pv_list" ]; then
    echo "No PVs found"
    exit 0
fi

# Process each PV
for pv in $pv_list; do
    echo "Processing $pv..."
    
    # Extract PVC name and namespace from PV
    pvc_info=$(kubectl get pv $pv -o jsonpath='{.spec.claimRef.name}/{.spec.claimRef.namespace}')
    pvc_name=$(echo $pvc_info | cut -d'/' -f1)
    namespace=$(echo $pvc_info | cut -d'/' -f2)
    
    echo "Removing finalizers from PVC $pvc_name in namespace $namespace..."
    kubectl patch pvc $pvc_name -n $namespace -p '{"metadata":{"finalizers":null}}'
    
    echo "Force deleting PVC $pvc_name..."
    kubectl delete pvc $pvc_name -n $namespace --grace-period=0 --force
    
    echo "Removing finalizers from PV $pv..."
    kubectl patch pv $pv -p '{"metadata":{"finalizers":null}}'
    
    echo "Force deleting PV $pv..."
    kubectl delete pv $pv --grace-period=0 --force
    
    echo "Completed processing $pv"
    echo "------------------------"
done

echo "All PVs processed successfully"
