FROM rust AS chef
RUN cargo install cargo-chef
WORKDIR /app
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN apt-get update && apt install cmake -y && apt-get install libclang-dev -y && rm -rf /var/lib/apt/lists/*
RUN cargo chef cook --release --recipe-path recipe.json
# Copy the sources
COPY . ./consumer
WORKDIR /app/consumer

# Note that we need to run `cargo sqlx prepare --workspace` to generate a new sqlx.toml file
# every time we change the database schema.
ENV SQLX_OFFLINE=true
# Copy over the cached dependencies
# Build the app
RUN cargo build --release --bin consumer --features local
# Use a slim Dockerfile with just our app to publish
FROM ubuntu:latest AS app
COPY --from=builder /app/consumer/target/release/consumer /
RUN apt-get update && apt install -y libssl3 && apt-get install ca-certificates -y

ENV RUST_LOG=info
