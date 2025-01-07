# Builder stage
FROM rust:1.81-slim as builder

WORKDIR /usr/src/app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    curl \
    capnproto \
    && rm -rf /var/lib/apt/lists/*

# Copy the entire workspace
COPY . .

# Build the specific package
ENV SQLX_OFFLINE=true
RUN cargo build --release -p image-guard


# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies only
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy only the compiled binary
COPY --from=builder /usr/src/app/target/release/image-guard ./api

# Create a non-root user
RUN useradd -m appuser && \
    chown appuser:appuser /app
USER appuser

ENV RUST_LOG=info
CMD ["./api"]
