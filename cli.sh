#!/bin/bash
docker run -i -t --env INTUITION_URL=http://host.docker.internal:8080/v1/graphql  ghcr.io/0xintuition/cli:latest cli
