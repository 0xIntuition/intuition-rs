#!/bin/bash

# Configure pg_stat_statements in postgresql.conf
echo "shared_preload_libraries = 'pg_stat_statements,timescaledb'" >> /home/postgres/pgdata/data/postgresql.conf

# Also add pg_stat_statements configuration
echo "pg_stat_statements.max = 10000" >> /home/postgres/pgdata/data/postgresql.conf
echo "pg_stat_statements.track = all" >> /home/postgres/pgdata/data/postgresql.conf
echo "pg_stat_statements.track_utility = on" >> /home/postgres/pgdata/data/postgresql.conf

echo "PostgreSQL configuration updated for pg_stat_statements"
