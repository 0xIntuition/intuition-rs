# RPC Proxy

This is a proxy for the RPC calls to the Ethereum network. It is used to cache the results of the RPC calls to the Ethereum network. Currently, it is only used to cache the results of the `eth_call` method for the `currentSharePrice` function of the `EthMultiVault` contract, other requests are not cached, but just relayed to the network.

## Usage

To run the proxy, you need to have a PostgreSQL database with the migrations (`indexer-and-cache-migrations`) applied. You can use the `docker-compose.yml` file to start the database and apply the migrations. Make sure to set the correct environment variables in the `.env` file.

### Consumers

Consumers that are using the proxy must adapt their RPC URL to use the proxy. For example, if you are using the proxy in the Base Sepolia network, you need to use the following URL:

```bash
http://rpc-proxy:3008/84532/proxy
```

If you are using the proxy in the Ethereum Mainnet, you need to use the following URL:

```bash
http://rpc-proxy:3008/1/proxy
```

### Running the proxy

```bash
docker compose up
```