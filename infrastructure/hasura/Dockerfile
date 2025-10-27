# Use Ubuntu as the base image
FROM ubuntu:latest

# Install necessary packages
RUN apt-get update && apt-get install -y \
    curl \
    bash \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy the hasura folder into the container
COPY infrastructure/hasura /app/hasura

# Set the working directory
WORKDIR /app

# Download and install hasura CLI directly (architecture-aware)
RUN ARCH=$(uname -m) && \
    if [ "$ARCH" = "x86_64" ]; then \
        HASURA_ARCH="amd64"; \
    elif [ "$ARCH" = "aarch64" ]; then \
        HASURA_ARCH="arm64"; \
    else \
        echo "Unsupported architecture: $ARCH" && exit 1; \
    fi && \
    curl -L "https://github.com/hasura/graphql-engine/releases/download/v2.48.0/cli-hasura-linux-${HASURA_ARCH}" -o /usr/local/bin/hasura && \
    chmod +x /usr/local/bin/hasura

# Execute hasura migrate and metadata apply commands
CMD ["bash", "-c", "hasura metadata apply --project hasura && hasura migrate apply --no-transaction --database-name intuition --project hasura && hasura metadata apply --project hasura"]
