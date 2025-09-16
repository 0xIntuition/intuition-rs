#!/bin/bash

docker compose -f infrastructure/blockscout/docker-compose.yml down --volumes
docker compose -f docker/docker-compose-apps.yml -f docker/docker-compose-shared.yml down --volumes