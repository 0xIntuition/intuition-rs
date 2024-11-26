# intuition-rust

This workspace contains the following crates:


* `cli`: contains the code to run the intuition TUI client.
* `consumer`: contains the code to RAW, DECODED and RESOLVER consumers.
* `hasura`: contains the migrations and hasura config.
* `histoflux`: streams historical data from our contracts to a queue. Currently supports SQS queues.
* `image-guard`: contains the code to guard the images.
* `models`: contains the domain models for the intuition data as basic traits for the data.
* `substreams-sink`: contains the code to consume the Substreams events.


Besides that, we have a `docker-compose.yml` file to run the full pipeline locally, a `Makefile` to run some commands using `cargo make` and the `LICENSE` file.

Note that all of the crates are under intensive development, so the code is subject to change.


## First steps

In order to be able to use the convenience commands in the Makefile, you need to install `cargo make`: 
* Install cargo make (`cargo install --force cargo-make`)

For Hasura, you need to:
* Install [hasura-cli](https://hasura.io/docs/2.0/hasura-cli/install-hasura-cli/)

And for SQS queues, you need to have AWS configured in your system, so you need to have a file in `˜./.aws/config` with the following content:

```
[default]
aws_access_key_id = YOUR_ACCESS_KEY_ID
aws_secret_access_key = YOUR_SECRET_ACCESS_KEY
```

## Configuring the `/etc/hosts` file

You need to add `database`, `ipfs`, `sqs` and `api` to the `/etc/hosts` file so it looks like this:

```
##
# Host Database
#
# localhost is used to configure the loopback interface
# when the system is booting.  Do not change this entry.
##
127.0.0.1	localhost
255.255.255.255	broadcasthost
::1             localhost
127.0.0.1 	database
127.0.0.1	ipfs
127.0.0.1	sqs
127.0.0.1	api
127.0.0.1	safe-content
```

## Running the local pipeline

There is a `.env.sample` file that you need to use as a template to create the `.env` file. First, you need to set the values for following variables:

* `PINATA_GATEWAY_TOKEN`: You can get the token from [Pinata](https://www.pinata.cloud/keys)
* `PINATA_API_JWT`: You can get the token from [Pinata](https://www.pinata.cloud/keys)
* `RPC_URL_MAINNET`: We are currently using Alchemy. You can create new ones using the [Alchemy dashboard](https://dashboard.alchemy.com/)
* `RPC_URL_BASE_MAINNET`: We are currently using Alchemy. You can create new ones using the [Alchemy dashboard](https://dashboard.alchemy.com/apps)
* `AWS_ACCESS_KEY_ID`: You can get the values from your [AWS account](https://us-east-1.console.aws.amazon.com/iam/home?region=us-east-1#/users)
* `AWS_SECRET_ACCESS_KEY`: You can get the values from your [AWS account](https://us-east-1.console.aws.amazon.com/iam/home?region=us-east-1#/users)
* `HF_TOKEN`: You can get the token from [Hugging Face](https://huggingface.co/settings/tokens)
* `SUBSTREAMS_API_TOKEN`: You can get the token from [Substreams](https://thegraph.market/auth/substreams-devenv)  

After filling all of the variables, you can run the following commands:

```
cp .env.sample .env
source .env
cargo make start-docker-and-migrate

```

## If you need to re-run migrations

```
docker compose down -v
docker compose up -d --force-recreate
cargo make migrate-database
```

## Run tests

```
cargo nextest run
```

## Known issues

None so far.

### Running manually

First you need to copy the `.env.sample` file to `.env` and source it. Make sure you set the correct values for the environment variables.
```
cp .env.sample .env
source .env
```

If you want to run the local raw consumer connected to the real raw SQS queue you can run

`RUST_LOG=info cargo run --bin consumer --mode raw` ( or simply `cargo make raw-consumer`)

If you want to run the local decoded consumer connected to the real decoded SQS queue you can run

`RUST_LOG=info cargo run --bin consumer --mode decoded` ( or simply `cargo make decoded-consumer`)

If you want to run the local raw consumer connected to the local SQS queue you can run

`RUST_LOG=info cargo run --bin consumer --features local --mode raw` (or `cargo make raw-consumer-local`)

If you want to run the local decoded consumer connected to the local SQS queue you can run

`RUST_LOG=info cargo run --bin consumer --features local --mode decoded` (or `cargo make decoded-consumer-local`)

We use feature flags to differentiate between the local and the remote execution environment.

Also note that you need to set the right environment variables for the queues (`RAW_CONSUMER_QUEUE_URL` and `DECODED_CONSUMER_QUEUE_URL`) in order to switch between the local and the remote execution environment.

## Conveniences

* `cargo make start-docker-and-migrate` to start the docker compose and run the migrations.
* `cargo make clippy` to run clippy
* `cargo make fmt` to run rustfmt
