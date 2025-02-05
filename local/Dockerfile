FROM rust:1.84-slim

WORKDIR /app

# Install PostgreSQL client
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    postgresql-client \
    && rm -rf /var/lib/apt/lists/*

# Create update script
RUN echo '#!/bin/bash \n\
PGPASSWORD=$POSTGRES_PASSWORD psql -h database -U $POSTGRES_USER -d $POSTGRES_DB -p $POSTGRES_PORT -c \
"UPDATE cursors.histoflux_cursor SET queue_url = '\''http://sqs.us-east-1.localhost.localstack.cloud:4566/000000000000/raw_logs.fifo'\'';"' > /app/update.sh

RUN chmod +x /app/update.sh

CMD ["/app/update.sh"]