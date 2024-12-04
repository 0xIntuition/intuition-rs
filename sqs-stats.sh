#!/bin/bash

ENDPOINT="http://localhost:4566"
export AWS_PAGER="" # Disable output pager

QUEUE_URLS=(
  "http://sqs.us-east-1.localhost.localstack.cloud:4566/000000000000/raw_logs"
  "http://sqs.us-east-1.localhost.localstack.cloud:4566/000000000000/decoded_logs"
  "http://sqs.us-east-1.localhost.localstack.cloud:4566/000000000000/resolver"
)

for QUEUE_URL in "${QUEUE_URLS[@]}"; do
  echo "Attributes for queue: $QUEUE_URL"
  aws --endpoint-url=$ENDPOINT sqs get-queue-attributes \
    --queue-url $QUEUE_URL \
    --attribute-names ApproximateNumberOfMessages ApproximateNumberOfMessagesNotVisible
  echo ""
done
