#!/bin/bash

# Define arrays for environments and services
ENVIRONMENTS=(
    "dev-base-sepolia"
    "dev-base-mainnet"
    "prod-base-sepolia-v2"
    "prod-base-mainnet-v2"
    "prod-linea-mainnet"
    "prod-linea-mainnet-v2"
    "prod-linea-sepolia"
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

# Add caching associative array
declare -A VERSION_CACHE

# Function to get image version for a pod
get_version() {
    local env=$1
    local pod=$2
    local key="${env}_${pod}"
    
    if [[ -n "${VERSION_CACHE[$key]}" ]]; then
        echo "${VERSION_CACHE[$key]}"
        return
    fi
    
    local pod_prefix="${env}-${pod}-"
    local pod_name
    pod_name=$(kubectl get pods --no-headers | awk -v prefix="$pod_prefix" '$1 ~ "^"prefix {print $1; exit}')
    
    if [[ -z "$pod_name" ]]; then
        VERSION_CACHE[$key]="N/A"
        echo "N/A"
        return
    fi

    local full_image
    full_image=$(kubectl get pod "$pod_name" -o jsonpath='{.spec.containers[0].image}' 2>/dev/null || echo "N/A")

    if [[ "$full_image" == "N/A" ]]; then
        VERSION_CACHE[$key]="N/A"
        echo "N/A"
    else
        local image_and_tag="${full_image##*/}"
        local version="${image_and_tag#*:}"
        VERSION_CACHE[$key]="$version"
        echo "$version"
    fi
}

# Function to print header for the old (vertical) table
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

# --- New function: print table with environments as rows ---
print_category_by_env() {
    local title=$1
    shift
    local services=("$@")
    
    echo -e "\n=== $title ==="
    # Compute dynamic table width based on number of columns (Environment + each service)
    local col_width=25
    local num_columns=$(( ${#services[@]} + 1 ))
    local total_width=$(( num_columns * col_width ))
    local line
    line=$(printf '=%.0s' $(seq 1 $total_width))
    
    # Print header row: first column "Environment", then each service (truncated as needed)
    printf "\n%-25s" "Environment"
    for service in "${services[@]}"; do
        local display_service="$service"
        if [ ${#service} -gt 12 ]; then
            display_service="${service:0:9}..."
        fi
        printf "%-25s" "$display_service"
    done
    echo ""
    echo "$line"
    
    # For each environment, print a row with version information for each service
    for env in "${ENVIRONMENTS[@]}"; do
        printf "\n%-25s" "$env"
        for service in "${services[@]}"; do
            version=$(get_version "$env" "$service")
            printf "%-25s" "$version"
        done
        echo ""
    done

    # Print status row: checks each service across all environments
    printf "\n%-25s" "Status"
    for service in "${services[@]}"; do 
        versions=()
        for env in "${ENVIRONMENTS[@]}"; do
            version=$(get_version "$env" "$service")
            versions+=("$version")
        done
        if check_versions_match "${versions[@]}"; then
            printf "%-25s" "  ✅"
        else
            printf "%-25s" "  ❌"
        fi
    done
    echo ""
}

# Main execution
echo "Collecting versions across environments..."

# Uncomment the layout you want:

# Existing vertical layout per service:
# print_category "CONSUMERS" "${CONSUMERS[@]}"
# print_category "HISTOFLUX" "${HISTOFLUX[@]}"
# print_category "MIGRATIONS" "${MIGRATIONS[@]}"

# New horizontal layout with environments as rows:
print_category_by_env "CONSUMERS" "${CONSUMERS[@]}"
print_category_by_env "HISTOFLUX" "${HISTOFLUX[@]}"
print_category_by_env "MIGRATIONS" "${MIGRATIONS[@]}"
