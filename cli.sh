#!/bin/bash
#!/bin/bash
docker run -i -t \
  --network intuition-rs_default \
  --env INTUITION_URL=http://graphql-engine:8080/v1/graphql \
  ghcr.io/0xintuition/cli:latest cli

