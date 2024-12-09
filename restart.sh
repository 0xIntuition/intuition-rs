#!/bin/bash

source .env
docker compose down -v
docker compose up -d
