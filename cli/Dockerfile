# Build stage
FROM rust:1.81.0-slim-bullseye as builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    capnproto \
    && rm -rf /var/lib/apt/lists/*

# Copy the entire workspace
COPY . .

# Build the CLI binary
RUN cargo build --release --package cli

# Runtime stage
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl1.1 \
    capnproto \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /app/target/release/cli /usr/local/bin/

# Set runtime configs
ENV RUST_LOG=info
ENV TERM=xterm-256color

# Run as non-root user
RUN groupadd -r appuser && useradd -r -g appuser appuser
USER appuser

CMD ["cli"]
