FROM python:3.11-slim

# Set the workdir
WORKDIR /app

# Install build dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Install pgai
RUN pip install pgai==0.10.5

# Set the entrypoint with environment variable
ENTRYPOINT ["sh", "-c", "python -m pgai install -d ${POSTGRES_URL}"] 