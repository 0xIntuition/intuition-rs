# Single stage build to avoid Docker layer corruption issues
FROM rust:1.89.0-slim-bookworm

# Install system dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    cmake \
    libclang-dev \
    libssl-dev \
    pkg-config \
    curl \
    capnproto \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create nonroot user
RUN groupadd --system nonroot --gid 65532 \
 && useradd --system -g nonroot --uid 65532 -M -s /sbin/nologin nonroot

WORKDIR /app

# Copy source code
COPY . .

# Build all binaries
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin consumer
RUN cargo build --release --bin rpc-proxy  
RUN cargo build --release --bin histocrawler
RUN cargo build --release --bin image-guard

# Move binaries to final location and set ownership
RUN mv /app/target/release/consumer /app/consumer && \
    mv /app/target/release/rpc-proxy /app/rpc-proxy && \
    mv /app/target/release/histocrawler /app/histocrawler && \
    mv /app/target/release/image-guard /app/image-guard && \
    chown nonroot:nonroot /app/consumer /app/rpc-proxy /app/histocrawler /app/image-guard

# Clean up build artifacts to reduce image size
RUN rm -rf /app/target /app/Cargo.toml /app/Cargo.lock /app/apps /app/infrastructure /app/integration-tests /app/scripts /app/docker /app/architecture-def.json /app/CLAUDE.md /app/README.md /app/LICENSE /app/Makefile.toml

USER nonroot:nonroot
ENV RUST_LOG=info
CMD ["/app/consumer"]
