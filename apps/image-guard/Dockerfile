# Stage 1 - Generate recipe file
FROM rust:1.85-slim AS chef
RUN cargo install cargo-chef
WORKDIR /usr/src/app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2 - Build dependencies and application
FROM chef AS builder
COPY --from=planner /usr/src/app/recipe.json recipe.json

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    curl \
    capnproto \
    && rm -rf /var/lib/apt/lists/*

RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release -p image-guard

# Stage 3 - Final runtime image
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Install runtime dependencies only
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy only the compiled binary
COPY --from=builder /usr/src/app/target/release/image-guard ./api

# Create a non-root user
RUN useradd -m appuser && \
    chown appuser:appuser /app/api

USER appuser

ENV RUST_LOG=info
CMD ["./api"]
