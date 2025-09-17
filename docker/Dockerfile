# Stage 1 - Generate recipe file
FROM rust:1.89.0-slim-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2 - Build dependencies and application
FROM planner AS builder
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
RUN cargo build --release --bin consumer \
 && cargo build --release --bin rpc-proxy \
 && cargo build --release --bin histocrawler \
 && cargo build --release --bin image-guard

# Stage 3 - Final runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 \
 && rm -rf /var/lib/apt/lists/*

RUN groupadd --system nonroot --gid 65532 \
 && useradd --system -g nonroot --uid 65532 -M -s /sbin/nologin nonroot

WORKDIR /app
COPY --from=builder --chown=nonroot:nonroot /app/target/release/consumer /app/consumer
COPY --from=builder --chown=nonroot:nonroot /app/target/release/consumer-api /app/consumer-api
COPY --from=builder --chown=nonroot:nonroot /app/target/release/rpc-proxy /app/rpc-proxy
COPY --from=builder --chown=nonroot:nonroot /app/target/release/histocrawler /app/histocrawler
COPY --from=builder --chown=nonroot:nonroot /app/target/release/image-guard /app/image-guard

USER nonroot:nonroot
ENV RUST_LOG=info
CMD ["/app/consumer"]
