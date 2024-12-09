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
RUN curl -L https://github.com/hasura/graphql-engine/raw/stable/cli/get.sh | bash

# Execute the hasura deploy command
CMD ["hasura", "deploy", "--project", "hasura"]
