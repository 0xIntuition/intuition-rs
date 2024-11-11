# Consumer

This is the `consumer` crate. It contains the code to consume the SQS queues and insert the messages into the DB. Currently, it has two consumers:
* `raw`: consumes the raw SQS queue, inserts the messages into the DB and then sends a message to the `decoded` consumer to start the processing of the decoded messages.
* `decoded`: consumes the decoded SQS queue, decodes the messages and inserts the result into their corresponding tables in the DB. Note that currently the Atom resolver is part of this consumer, but we are already working on improving that. Atom resolving is the process of fetching the raw data from the IPFS network and inserting it into the DB. IPFS network is very slow, so we don't want to do it as part of the `raw` consumer, as it would slow down the processing of the messages.

## Abstractions

Currently we are using SQS queues to consume and produce messages. This is a good choice for us because SQS queues are very reliable and scalable. However, if we decide to use a different queue system, we just need to change the implementation of the `BasicConsumer` trait.

Currently we support the ingestion of messages from two different sources: Goldsky and Substreams. Again, this is a good choice for us because Goldsky and Substreams are very reliable and scalable systems. However, if we decide to use a different system, we just need to implement the `IntoRawMessage` trait for the new system.

## Environment variables

All the environment variables are stored in the `.env.sample` file. Here is the description of each one of them:

* `AWS_ACCESS_KEY_ID`: the access key id to access the AWS services.
* `AWS_REGION`: the region of the AWS services.
* `AWS_SECRET_ACCESS_KEY`: the secret access key to access the AWS services.
* `CONSUMER_TYPE`: the type of consumer. Currently we support `sqs`.
* `CONTRACT_ADDRESS`: the address of the contract that we want to consume the messages from.
* `DATABASE_URL`: the URL of the database.
* `DATA_SOURCE`: the source of the data. Currently we support `goldsky` and `substreams`.
* `DECODED_LOGS_QUEUE_URL`: the URL of the decoded SQS queue.
* `HASURA_GRAPHQL_ADMIN_SECRET`: the admin secret key to access the Hasura GraphQL engine.
* `HASURA_GRAPHQL_ENDPOINT`: the endpoint of the Hasura GraphQL engine.
* `INDEXING_SOURCE`: the source of the indexing. Currently we support `substreams`.
* `IPFS_GATEWAY_URL`: the URL of the IPFS gateway.
* `LOCALSTACK_URL`: the URL of the Localstack service.
* `OUT_DIR`: the output directory of the consumer.
* `PG_DB`: the name of the database.
* `PG_HOST`: the host of the database.
* `PG_MIN_CONNECTIONS`: the minimum number of connections to the database.
* `PG_PASSWORD`: the password of the database.
* `PG_PORT`: the port of the database.
* `PG_USER`: the user of the database.
* `PINATA_GATEWAY_TOKEN`: the token to access the Pinata gateway.
* `RAW_CONSUMER_QUEUE_URL`: the URL of the raw SQS queue.
* `RPC_URL`: the URL of the RPC service.
* `RUST_LOG`: the log level.
