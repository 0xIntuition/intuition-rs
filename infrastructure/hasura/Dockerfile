# Use Ubuntu as the base image
FROM ubuntu:latest

# Docker provides TARGETARCH as amd64/arm64 with BuildKit, matching Hasura's
# CLI release asset names.
ARG TARGETARCH

# Download and install hasura CLI directly.
ADD https://github.com/hasura/graphql-engine/releases/download/v2.48.0/cli-hasura-linux-${TARGETARCH} /usr/local/bin/hasura
RUN chmod +x /usr/local/bin/hasura

# Copy the hasura folder into the container
COPY infrastructure/hasura /app/hasura

# Set the working directory
WORKDIR /app

# Execute hasura migrate and metadata apply commands
CMD ["sh", "-c", "hasura --skip-update-check metadata apply --project hasura && hasura --skip-update-check migrate apply --no-transaction --database-name intuition --project hasura && hasura --skip-update-check metadata apply --project hasura"]
