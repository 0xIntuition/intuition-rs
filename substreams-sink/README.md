## Substreams Sink Rust

This repository show cases a functional base Rust project that consumes a Substreams `.spkg` (local or remote file).

To run:

```bash
export SUBSTREAMS_API_TOKEN="<StreamingFast API Token>" 
cargo run -- https://base-mainnet.streamingfast.io:443 https://spkg.io/streamingfast/ethereum-common-v0.3.0.spkg filtered_events 21752643:21757305
```

### Details

The presented Rust project contains a `SubstreamsStream` wrapper that handles automatic reconnection in case of error. It is implemented as a Rust `TryStream` which enable consuming the retryable stream easily using standard Rust syntax:

```rust
let stream = SubstreamsStream::new(...);

loop {
   match stream.next().await {
      None => { /* Stream completed, reached end block */ },
      Some(Ok(BlockResponse::New(data))) => { /* Got a BlockScopedData message */ },
      Some(Err(err)) => { /* Fatal error or retry limit reached */ },
   }
}
```

The `main.rs` file accepts three arguments: the substreams endpoint (in the form `http(s)?://<url>:<port>`), the location of the `.spkg` file to use for the request, and the output module's name to stream from.

#### Incomplete Implementation

##### Cursor Persistence

For now `cursor` handling is not properly loaded/saved to database, something that would be required on a production system to ensure the stream is resumed at the right location and that a block is never miss.

Should be implemented in [main.rs](./src/main.rs) in `persist_cursor` function and in `load_persisted_cursor`.

> **Warning** If you don't implement cursor persistence, if your process restart, it will start back from specified `start_block` (currently hard-coded to `0`).

##### Logging

We use `println` statements within [main.rs](./src/main.rs) and [substreams_stream.rs](./src/substreams_stream.rs) to demo what is happening within the codebase. Those, specially in [substreams_stream.rs](./src/substreams_stream.rs), should be replaced to be logged to a logging system.

Also, more places could be instrumented to log extra details.

##### Block Undo Signal

`BlockUndoSignal` must be treated as "delete every data that has been recorded after block height specified by block in BlockUndoSignal". In the example above, this means you must delete changes done by `Block #7b` and `Block #6b`. The exact details depends on your own logic. If for example all your added record contain a block number, a simple way is to do `delete all records where block_num > 5` which is the block num received in the `BlockUndoSignal` (this is true for append only records, so when only `INSERT` are allowed).

This is left to be implemented by you how to deal with that.

> **Warning** It's done using `unimplemented!` macro which will panic if an undo signal is received, so be warned for a production system. Undo signals on Ethereum Mainnet happen around 5-10 times a day, even less so you might miss the fact that they exist when testing.

### Protobuf Generation

Protobuf generation is done using [buf](https://buf.build/) which can be installed with:

```bash
brew install bufbuild/buf/buf
```

> **Note** See [other installation methods](https://buf.build/docs/installation/) if not on Mac/Linux (`brew` can be installed on Linux).

#### From a pre-built `.spkg`

If you have an `.spkg` that already contains your Protobuf definitions, you can use it directly. It contains Substreams system Protobuf definitions so you can generate everything in one shot:

```bash
buf generate --exclude-path="google" <path/to/substream.spkg>#format=bin
```

> **Note** An `.spkg` contains recursively all Proto definitions, some you may not desire. You can exclude generation of some element via `--exclude-path="google"` flag. You can specify many separated by a comma.

#### Just Substreams

You can generate against published [Substreams Buf Module](https://buf.build/streamingfast/substreams):

```bash
buf generate buf.build/streamingfast/substreams --include-imports
```

This will include only Substreams system Protobufs to decode packages and perform RPC operations.
