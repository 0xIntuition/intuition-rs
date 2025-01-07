# Stage 1: Build
FROM rust:latest AS builder

# Set the working directory
WORKDIR /usr/src/app

# Copy the entire workspace
COPY . .
ENV SQLX_OFFLINE=true
# Install runtime dependencies only
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    capnproto \
    && rm -rf /var/lib/apt/lists/*

# Build the release version of the crate
RUN cargo build --release --package envio-indexer

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Set the working directory
WORKDIR /usr/src/app

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/app/target/release/envio-indexer .

# Set runtime configs
ENV RUST_LOG=info

# Run the binary
ENTRYPOINT ["/usr/src/app/envio-indexer"]
