#!/bin/bash

# Define arrays for environments and services
ENVIRONMENTS=(
    "dev-base-sepolia"
    "dev-base-mainnet"
    "prod-base-sepolia"
    "prod-base-mainnet"
    "prod-linea-mainnet"
)

CONSUMERS=(
    "raw-consumer"
    "decoded-consumer"
    "resolver-consumer"
    "ipfs-upload-consumer"
)

HISTOFLUX=(
    "histoflux"
)

MIGRATIONS=(
    "hasura-migrations"
    "indexer-and-cache-migration"
)

# Function to get image version for a pod
get_version() {
    local env=$1
    local pod=$2

    local pod_prefix="${env}-${pod}-"
    local pod_name
    pod_name=$(kubectl get pods --no-headers | awk -v prefix="$pod_prefix" '$1 ~ "^"prefix {print $1; exit}')

    if [[ -z "$pod_name" ]]; then
        echo "N/A"
        return
    fi

    local full_image
    full_image=$(kubectl get pod "$pod_name" -o jsonpath='{.spec.containers[0].image}' 2>/dev/null || echo "N/A")

    if [[ "$full_image" == "N/A" ]]; then
        echo "N/A"
    else
        # Extract the final segment (e.g., consumer:2.0.18)
        local image_and_tag="${full_image##*/}"
        # Extract just the version after the colon (e.g., 2.0.18)
        local version="${image_and_tag#*:}"
        echo "$version"
    fi
}

# Function to print header
print_header() {
    printf "\n%-20s" "Service"
    for env in "${ENVIRONMENTS[@]}"; do
        printf "%-25s" "$env"
    done
    echo -e "\n${line}"
}

# Function to check if all versions match
check_versions_match() {
    local versions=("$@")
    local first_version=${versions[0]}
    
    for version in "${versions[@]}"; do
        if [[ "$version" != "$first_version" ]]; then
            return 1
        fi
    done
    return 0
}

# Print results for a service category
print_category() {
    local title=$1
    shift
    local services=("$@")
    
    echo -e "\n=== $title ==="
    line=$(printf '=%.0s' {1..145})
    print_header

    for service in "${services[@]}"; do
        local display_service="$service"
        if [ ${#service} -gt 12 ]; then
            display_service="${service:0:9}..."
        fi
        printf "%-20s" "$display_service"
        versions=()
        
        for env in "${ENVIRONMENTS[@]}"; do
            version=$(get_version "$env" "$service")
            versions+=("$version")
            printf "%-25s" "$version"
        done
        
        if check_versions_match "${versions[@]}"; then
            echo "  ✅"
        else
            echo "  ❌"
        fi
    done
}

# Main execution
echo "Collecting versions across environments..."

print_category "CONSUMERS" "${CONSUMERS[@]}"
print_category "HISTOFLUX" "${HISTOFLUX[@]}"
print_category "MIGRATIONS" "${MIGRATIONS[@]}"
