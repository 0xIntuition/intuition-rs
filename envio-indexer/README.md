# Envio Indexer

This crate is used to index the events of our contracts using Envio. Currently, it is only used to index the base-sepolia events of our contract, but we are working on adding more networks.

You need to export your hypersync token in the `HYPERSYNC_TOKEN` environment variable.
You can create one here: https://envio.dev/app/api-tokens

```bash
export HYPERSYNC_TOKEN=your_token
```

Run with

```bash
cargo run -- --network base-sepolia
```