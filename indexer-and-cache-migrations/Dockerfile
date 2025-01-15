FROM rust:1.84-slim

WORKDIR /app

# Copy migrations from workspace
COPY indexer-and-cache-migrations ./migrations

# Install build and runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libssl-dev \
    pkg-config \
    capnproto \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install sqlx-cli --no-default-features --features native-tls,postgres

CMD ["sh", "-c", "sqlx migrate run --database-url $DATABASE_URL"]
