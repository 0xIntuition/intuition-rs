# intuition-rust

This workspace contains the following crates:


* `cli`: contains the code to run the intuition TUI client.
* `consumer`: contains the code to RAW, DECODED and RESOLVER consumers.
* `envio-indexer`: contains the code to index the base-sepolia events of our contract using envio.
* `hasura`: contains the migrations and hasura config.
* `histoflux`: streams historical/live events from the database to an SQS queue.`
* `image-guard`: contains the code to guard the images.
* `models`: contains the domain models for the intuition data as basic traits for the data.
* `rpc-proxy`: contains the code to proxy the RPC calls to their respective networks, caching the results of the `eth_call` method for the `currentSharePrice` function of the `EthMultiVault` contract.
* `substreams-sink`: contains the code to consume the Substreams events.


Besides that, we have a `docker-compose.yml` file to run the full pipeline locally, a `Makefile` to run some commands using `cargo make` and the `LICENSE` file.

Note that all of the crates are under intensive development, so the code is subject to change. Also, notice that if you want to index base events you need to 
uncomment the `substreams-sink` crate in the `docker-compose.yml` file and comment the `envio-indexer` crate. We are figuring out the best process to handle this.


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

## Running the local pipeline

There is a `.env.sample` file that you need to use as a template to create the `.env` file. First, you need to set the values for following variables:

* `PINATA_GATEWAY_TOKEN`: You can get the token from [Pinata](https://app.pinata.cloud/developers/gateway-settings)
* `PINATA_API_JWT`: You can get the token from [Pinata](https://app.pinata.cloud/developers/api-keys)
* `RPC_URL_MAINNET`: We are currently using Alchemy. You can create new ones using the [Alchemy dashboard](https://dashboard.alchemy.com/)
* `RPC_URL_BASE`: We are currently using Alchemy. You can create new ones using the [Alchemy dashboard](https://dashboard.alchemy.com/apps)
* `AWS_ACCESS_KEY_ID`: You can get the values from your [AWS account](https://us-east-1.console.aws.amazon.com/iam/home?region=us-east-1#/users)
* `AWS_SECRET_ACCESS_KEY`: You can get the values from your [AWS account](https://us-east-1.console.aws.amazon.com/iam/home?region=us-east-1#/users)
* `HF_TOKEN`: You can get the token from [Hugging Face](https://huggingface.co/settings/tokens)
* `SUBSTREAMS_API_TOKEN`: You can get the token from [Substreams](https://thegraph.market/auth/substreams-devenv)  
* `HYPERSYNC_TOKEN`: You can get the token from [Envio](https://envio.dev/app/api-tokens)

After filling all of the variables, you can run the following commands:

### Using published docker images

```
./start.sh
```

#### Runing cli tool to verify latest data

```
./cli.sh
```

Later, you can use `./stop.sh` to stop all services or `./restart.sh` to restart all services and clear attached volumes

### Building docker images from source code

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

`RUST_LOG=info cargo run --bin consumer --features local --mode raw --local` (or `cargo make raw-consumer-local`)

If you want to run the local decoded consumer connected to the local SQS queue you can run

`RUST_LOG=info cargo run --bin consumer --features local --mode decoded --local` (or `cargo make decoded-consumer-local`)

We use feature flags to differentiate between the local and the remote execution environment.

Also note that you need to set the right environment variables for the queues (`RAW_CONSUMER_QUEUE_URL` and `DECODED_CONSUMER_QUEUE_URL`) in order to switch between the local and the remote execution environment.

## Conveniences

* `cargo make start-docker-and-migrate` to start the docker compose and run the migrations.
* `cargo make clippy` to run clippy
* `cargo make fmt` to run rustfmt

You can check all of the available commands in `.cargo/makefiles`. 

## Running with kubernetes (on macos)

First you need to install `minikube`:

```
brew install minikube
```

Install k9s

```
brew install k9s
```

Then we need to create the secrets. At this step it's expected that you have a `.env` file with the correct values set. The only thing you need to keep in mind is that we need to remove the `"` from the values, e.g., `DATABASE_URL="postgres://testuser:test@database:5435/storage"` should be `DATABASE_URL=postgres://testuser:test@database:5435/storage`.

```
kubectl create secret generic secrets --from-env-file=.env
```

Then you can start the minikube cluster:

```
minikube start
```

Then you can apply the kubernetes manifests:

```
kubectl apply -k kube_files/
```

To restart the services you can run:

```
kubectl rollout restart deployment
```

or 

```
kubectl delete deployment --all 
```

There is a `devops` folder that contains yaml files to deploy our stack to both Minikube and AWS EKS.


## Using local ethereum node

Add the following to your `.env` file:

```
BASE_MAINNET_RPC_URL=http://geth:8545
BASE_SEPOLIA_RPC_URL=http://geth:8545
INTUITION_CONTRACT_ADDRESS=0x04056c43d0498b22f7a0c60d4c3584fb5fa881cc
START_BLOCK=0
```

Then you can create local data by running the following command:

```
cd integration-tests
npm install
npm run create-predicates
```
