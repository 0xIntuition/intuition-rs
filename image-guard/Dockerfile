# Builder stage
FROM rust:1.76-slim as builder

WORKDIR /usr/src/app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests first (better layer caching)
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Now copy the real source code
COPY src ./src

# Build the actual binary
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies only
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy only the compiled binary
COPY --from=builder /usr/src/app/target/release/your-binary-name ./api

# Create a non-root user
RUN useradd -m appuser && \
    chown appuser:appuser /app
USER appuser

EXPOSE 3000
CMD ["./api"]
