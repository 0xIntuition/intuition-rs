#!/bin/bash

docker compose -f docker-compose-apps.yml down --volumes
docker compose -f docker-compose-shared.yml down --volumes
