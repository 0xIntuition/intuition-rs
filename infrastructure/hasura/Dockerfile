# Use Ubuntu as the base image
FROM ubuntu:latest

# Install necessary packages
RUN apt-get update && apt-get install -y \
    curl \
    bash \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy the hasura folder into the container
COPY hasura /app/hasura

# Set the working directory
WORKDIR /app

# Download and install hasura
RUN curl -L https://github.com/hasura/graphql-engine/raw/stable/cli/get.sh | VERSION=v2.48.0 bash

# Execute hasura migrate and metadata apply commands
CMD ["bash", "-c", "hasura metadata apply --project hasura && hasura migrate apply --no-transaction --database-name intuition --project hasura && hasura metadata apply --project hasura"]
