# Stage 1 - Generate recipe file
FROM rust:slim-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2 - Build dependencies and application
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    cmake \
    libclang-dev \
    libssl-dev \
    pkg-config \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin substreams-sink --features local

# Stage 3 - Final runtime image
FROM debian:bookworm-slim AS runtime

# Create non-root user
RUN groupadd -r appuser && useradd -r -g appuser appuser

# Install runtime dependencies only
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/substreams-sink /app/substreams-sink

# Set ownership
RUN chown appuser:appuser /app/substreams-sink

# Use non-root user
USER appuser

# Set runtime configs
ENV RUST_LOG=info
WORKDIR /app

ENTRYPOINT ["/app/substreams-sink"]
