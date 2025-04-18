# Stage 1 - Generate recipe file
FROM rust:1.85-slim AS chef
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
    capnproto \
    && rm -rf /var/lib/apt/lists/*

RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin consumer
RUN cargo build --release --bin consumer-api
RUN cargo build --release --bin cli
RUN cargo build --release --bin rpc-proxy
RUN cargo build --release --bin histoflux
RUN cargo build --release --bin histocrawler
RUN cargo build --release --bin image-guard

# Stage 3 - Final runtime image
FROM gcr.io/distroless/cc-debian12

# Copy binary from builder
COPY --from=builder --chown=nonroot:nonroot /app/target/release/consumer /app/consumer
COPY --from=builder --chown=nonroot:nonroot /app/target/release/consumer-api /app/consumer-api
COPY --from=builder --chown=nonroot:nonroot /app/target/release/cli /app/cli
COPY --from=builder --chown=nonroot:nonroot /app/target/release/rpc-proxy /app/rpc-proxy
COPY --from=builder --chown=nonroot:nonroot /app/target/release/histoflux /app/histoflux
COPY --from=builder --chown=nonroot:nonroot /app/target/release/histocrawler /app/histocrawler
COPY --from=builder --chown=nonroot:nonroot /app/target/release/image-guard /app/image-guard

# Use non-root user
USER nonroot:nonroot

# Set runtime configs
ENV RUST_LOG=info
WORKDIR /app

CMD ["/app/consumer"]
