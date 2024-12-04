#!/bin/sh
echo "Initializing localstack SQS..."
awslocal sqs create-queue --queue-name raw_logs
awslocal sqs create-queue --queue-name decoded_logs
awslocal sqs create-queue --queue-name resolver
awslocal sqs create-queue --queue-name ipfs_upload