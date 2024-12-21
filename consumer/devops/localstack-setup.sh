#!/bin/sh
echo "Initializing localstack SQS..."
awslocal sqs create-queue --queue-name raw_logs.fifo --attributes FifoQueue=true,ContentBasedDeduplication=true
awslocal sqs create-queue --queue-name decoded_logs.fifo --attributes FifoQueue=true,ContentBasedDeduplication=true
awslocal sqs create-queue --queue-name resolver
awslocal sqs create-queue --queue-name ipfs_upload --attributes VisibilityTimeout=600
