# HistoFlux

This crate contains the code for the HistoFlux project. The goal of this project is to process
a zip file containing records of our contract events and feed them to an SQS queue.

## Usage

In order to run the project, you simply need to run the following command:

```bash
RUST_LOG=info cargo run --bin histoflux -- --queue-name <queue-name>
```

or if you want to run it locally:

```bash
RUST_LOG=info cargo run --features local -- --queue-name http://sqs.us-east-1.localhost.localstack.cloud:4566/000000000000/activity
```

## Managing queue state:

You can install some tools to manage the local state of the SQS queue:

- [AwsCLI-Local](https://github.com/localstack/awscli-local)



To check queue status

```bash
awslocal sqs get-queue-attributes --queue-url http://sqs.us-east-1.localhost.localstack.cloud:4566/000000000000/activity --attribute-names All
```

