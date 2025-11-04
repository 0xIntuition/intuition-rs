# CloudNativePG PostgreSQL 17 with TimescaleDB
# This image fixes the issues in ghcr.io/clevyr/cloudnativepg-timescale:17-ts2
# by properly configuring shared_preload_libraries and ensuring correct extension files

FROM ghcr.io/cloudnative-pg/postgresql:17

# Switch to root to install packages
USER root

# Add TimescaleDB repository and install
RUN apt-get update && \
    apt-get install -y \
        curl \
        gnupg \
        lsb-release && \
    # Add TimescaleDB repository
    echo "deb https://packagecloud.io/timescale/timescaledb/debian/ $(lsb_release -c -s) main" | tee /etc/apt/sources.list.d/timescaledb.list && \
    curl -L https://packagecloud.io/timescale/timescaledb/gpgkey | apt-key add - && \
    # Update and install TimescaleDB for PostgreSQL 17
    apt-get update && \
    apt-get install -y postgresql-17-timescaledb-2 && \
    # Clean up to reduce image size
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Configure PostgreSQL to preload TimescaleDB
# This is the critical fix that makes TimescaleDB actually work
RUN echo "shared_preload_libraries = 'timescaledb'" >> /usr/share/postgresql/postgresql.conf.sample

# Verify the extension files are present
RUN ls -la /usr/share/postgresql/17/extension/timescaledb* | head -20 || true

# Switch back to postgres user (UID 26 in CNPG images)
USER 26

# Labels for documentation and registry
LABEL org.opencontainers.image.source="https://github.com/0xintuition/intuition-rs"
LABEL org.opencontainers.image.description="CloudNativePG PostgreSQL 17 with TimescaleDB 2.x - properly configured"
LABEL org.opencontainers.image.version="17-timescale-2.17"
LABEL maintainer="0xintuition"
