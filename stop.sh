#!/bin/bash

docker compose -f blockscout/docker-compose.yml down --volumes
docker compose -f docker-compose-apps.yml -f docker-compose-shared.yml down --volumes