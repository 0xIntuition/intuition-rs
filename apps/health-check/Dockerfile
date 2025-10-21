# Build stage
FROM rust:1.90-slim-bookworm as builder

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./

# Copy all workspace members
COPY apps ./apps

# Install OpenSSL development packages and pkg-config
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Build the health-check binary
RUN cargo build --release -p health-check

# Runtime stage
FROM debian:bookworm-slim

# Install CA certificates and OpenSSL
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/health-check /app/health-check

# Set environment variables
ENV RUST_LOG=info

EXPOSE 3000

CMD ["/app/health-check"]
