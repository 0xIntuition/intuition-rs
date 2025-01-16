#!/bin/bash
set -e

echo "Starting database creation with user: $POSTGRES_USER"

# Create base_sepolia_indexer database
if psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" -lqt | cut -d \| -f 1 | grep -qw base_sepolia_indexer; then
    echo "Database base_sepolia_indexer already exists"
else
    echo "Creating base_sepolia_indexer database..."
    psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" -c "CREATE DATABASE base_sepolia_indexer;"
    psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" -c "GRANT ALL PRIVILEGES ON DATABASE base_sepolia_indexer TO $POSTGRES_USER;"
    echo "Database base_sepolia_indexer created successfully"
fi 

# Create base_sepolia_proxy database
if psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" -lqt | cut -d \| -f 1 | grep -qw base_sepolia_proxy; then
    echo "Database base_sepolia_proxy already exists"
else
    echo "Creating base_sepolia_proxy database..."
    psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" -c "CREATE DATABASE base_sepolia_proxy;"
    psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" -c "GRANT ALL PRIVILEGES ON DATABASE base_sepolia_proxy TO $POSTGRES_USER;"
    echo "Database base_sepolia_proxy created successfully"
fi 