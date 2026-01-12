# Stage 1 - Generate recipe file
FROM rust:1.89.0-slim-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2 - Build dependencies and application
FROM rust:1.89.0-slim-bookworm AS builder
RUN cargo install cargo-chef
WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    cmake \
    libclang-dev \
    libssl-dev \
    pkg-config \
    curl \
    capnproto \
    && rm -rf /var/lib/apt/lists/*

RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin chart-api

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

# Workdir
WORKDIR /app

# Copy binary from builder (with ownership set at copy time)
COPY --from=builder --chown=appuser:appuser /app/target/release/chart-api /app/chart-api

# Use non-root user
USER appuser

# Set runtime configs
ENV RUST_LOG=info

CMD ["/app/chart-api"]
