# HistoFlux

This crate contains the code for the HistoFlux project. The goal of this project is to process historical events from the database and feed them to an SQS queue. 
After the historical events are processed, the project will start listening for new events and feed them to the SQS queue.

## Usage

In order to run the project, you simply need to run the following command:

```bash
RUST_LOG=info cargo run
```

