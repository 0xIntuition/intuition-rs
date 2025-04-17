#!/bin/bash

docker compose -f docker-compose-apps.yml -f docker-compose-shared.yml down --volumes
