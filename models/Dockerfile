# Stage 1: Build virtualenv with pgai
FROM python:3.11-slim AS builder

WORKDIR /app

# Install pgai in venv
RUN python -m venv /opt/venv && \
    . /opt/venv/bin/activate && \
    pip install --no-cache-dir pgai==0.10.5

# Stage 2: Final runtime image
FROM python:3.11-slim

# Copy the virtualenv from builder
COPY --from=builder /opt/venv /opt/venv

# Use venv binaries
ENV PATH="/opt/venv/bin:$PATH"

WORKDIR /app

ENTRYPOINT ["sh", "-c", "python -m pgai install -d ${POSTGRES_URL}"]
