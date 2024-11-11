use anyhow::{format_err, Context, Error};
// use chrono::DateTime;
use crate::pb::sf::substreams::v1::module::Input;
use aws_sdk_sqs::Client as AWSClient;
use futures03::StreamExt;
use macon::Builder;
use pb::sf::substreams::rpc::v2::{BlockScopedData, BlockUndoSignal};
use pb::sf::substreams::v1::module::input::{Input as InputEnum, Map, Params};
use pb::sf::substreams::v1::Package;
use prost::Message;
use serde::{Deserialize, Serialize};
use std::{env, process::exit, sync::Arc};
use substreams::SubstreamsEndpoint;
use substreams_stream::{BlockResponse, SubstreamsStream};

mod pb;
mod substreams;
mod substreams_stream;

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenvy::dotenv().ok();
    let args = env::args();
    if args.len() < 4 || args.len() > 5 {
        println!("usage: stream <endpoint> <spkg> <module> [<start>:<stop>]");
        println!();
        println!("The environment variable SUBSTREAMS_API_TOKEN must be set also");
        println!("and should contain a valid Substream API token.");
        exit(1);
    }

    let endpoint_url = env::args().nth(1).unwrap();
    let package_file = env::args().nth(2).unwrap();
    let module_name = env::args().nth(3).unwrap();

    let token_env = env::var("SUBSTREAMS_API_TOKEN").unwrap_or("".to_string());
    let mut token: Option<String> = None;
    if token_env.len() > 0 {
        token = Some(token_env);
    }

    let package = read_package(&package_file).await?;
    let block_range = read_block_range(&package, &module_name)?;
    let endpoint = Arc::new(SubstreamsEndpoint::new(&endpoint_url, token).await?);

    let cursor: Option<String> = load_persisted_cursor()?;

    let mut mutable_modules = package.modules.as_ref().unwrap().clone();

    mutable_modules.modules.iter_mut().for_each(|module| {
        if module.name == "filtered_events" {
            module.inputs.clear();
            module.inputs.push(Input {
                input: Some(InputEnum::Params(Params {
                    value: "evt_addr:0x430bbf52503bd4801e51182f4cb9f8f534225de5".to_string(),
                })),
            });
            module.inputs.push(Input {
                input: Some(InputEnum::Map(Map {
                    module_name: "all_events".to_string(),
                })),
            });
        }
    });

    let mut stream = SubstreamsStream::new(
        endpoint.clone(),
        cursor,
        Some(mutable_modules),
        module_name.to_string(),
        block_range.0,
        block_range.1,
    );

    // AWS SQS client
    let shared_config = aws_config::from_env().load().await;
    let aws_client = AWSClient::new(&shared_config);

    loop {
        match stream.next().await {
            None => {
                println!("Stream consumed");
                break;
            }
            Some(Ok(BlockResponse::New(data))) => {
                process_block_scoped_data(&data, &aws_client).await?;
                persist_cursor(data.cursor)?;
            }
            Some(Ok(BlockResponse::Undo(undo_signal))) => {
                process_block_undo_signal(&undo_signal)?;
                persist_cursor(undo_signal.last_valid_cursor)?;
            }
            Some(Err(err)) => {
                println!();
                println!("Stream terminated with error");
                println!("{:?}", err);
                exit(1);
            }
        }
    }

    Ok(())
}

#[derive(Debug, Deserialize, Serialize, Builder)]
pub struct RawLog {
    // pub gs_id: String,
    pub block_number: u64,
    // pub block_hash: String,
    pub transaction_hash: String,
    pub transaction_index: u64,
    pub log_index: u64,
    pub address: String,
    pub data: String,
    pub topics: Vec<String>,
    pub block_timestamp: i64,
}

async fn process_block_scoped_data(
    data: &BlockScopedData,
    aws_client: &AWSClient,
) -> Result<(), Error> {
    let output = data.output.as_ref().unwrap().map_output.as_ref().unwrap();

    let value = pb::sf::substreams::ethereum::v1::Events::decode(output.value.as_slice())?;

    for event in value.events.iter() {
        let log = event.log.as_ref().unwrap();
        let clock = data.clock.as_ref().unwrap();
        let raw_log = RawLog::builder()
            .block_number(clock.number)
            .transaction_hash(event.tx_hash.to_string())
            .transaction_index(log.block_index)
            .log_index(log.index)
            .address(hex::encode(&log.address))
            .data(hex::encode(&log.data))
            .topics(log.topics.iter().map(hex::encode).collect::<Vec<String>>())
            .block_timestamp(clock.timestamp.unwrap().seconds)
            .build();

        let message = serde_json::to_string(&raw_log)?;
        println!("{:#?}", message);

        aws_client
            .send_message()
            .queue_url("https://sqs.us-west-2.amazonaws.com/064662847354/Substream") // Replace with actual queue URL
            .message_body(message)
            .send()
            .await?;
    }

    Ok(())
}

fn process_block_undo_signal(_undo_signal: &BlockUndoSignal) -> Result<(), anyhow::Error> {
    // `BlockUndoSignal` must be treated as "delete every data that has been recorded after
    // block height specified by block in BlockUndoSignal". In the example above, this means
    // you must delete changes done by `Block #7b` and `Block #6b`. The exact details depends
    // on your own logic. If for example all your added record contain a block number, a
    // simple way is to do `delete all records where block_num > 5` which is the block num
    // received in the `BlockUndoSignal` (this is true for append only records, so when only `INSERT` are allowed).
    unimplemented!("you must implement some kind of block undo handling, or request only final blocks (tweak substreams_stream.rs)")
}

fn persist_cursor(_cursor: String) -> Result<(), anyhow::Error> {
    // FIXME: Handling of the cursor is missing here. It should be saved each time
    // a full block has been correctly processed/persisted. The saving location
    // is your responsibility.
    //
    // By making it persistent, we ensure that if we crash, on startup we are
    // going to read it back from database and start back our SubstreamsStream
    // with it ensuring we are continuously streaming without ever losing a single
    // element.
    Ok(())
}

fn load_persisted_cursor() -> Result<Option<String>, anyhow::Error> {
    // FIXME: Handling of the cursor is missing here. It should be loaded from
    // somewhere (local file, database, cloud storage) and then `SubstreamStream` will
    // be able correctly resume from the right block.
    Ok(None)
}

fn read_block_range(pkg: &Package, module_name: &str) -> Result<(i64, u64), anyhow::Error> {
    let module = pkg
        .modules
        .as_ref()
        .unwrap()
        .modules
        .iter()
        .find(|m| m.name == module_name)
        .ok_or_else(|| format_err!("module '{}' not found in package", module_name))?;

    let mut input: String = "".to_string();
    if let Some(range) = env::args().nth(4) {
        input = range;
    };

    let (prefix, suffix) = match input.split_once(":") {
        Some((prefix, suffix)) => (prefix.to_string(), suffix.to_string()),
        None => ("".to_string(), input),
    };

    let start: i64 = match prefix.as_str() {
        "" => module.initial_block as i64,
        x if x.starts_with("+") => {
            let block_count = x
                .trim_start_matches("+")
                .parse::<u64>()
                .context("argument <stop> is not a valid integer")?;

            (module.initial_block + block_count) as i64
        }
        x => x
            .parse::<i64>()
            .context("argument <start> is not a valid integer")?,
    };

    let stop: u64 = match suffix.as_str() {
        "" => 0,
        "-" => 0,
        x if x.starts_with("+") => {
            let block_count = x
                .trim_start_matches("+")
                .parse::<u64>()
                .context("argument <stop> is not a valid integer")?;

            start as u64 + block_count
        }
        x => x
            .parse::<u64>()
            .context("argument <stop> is not a valid integer")?,
    };

    return Ok((start, stop));
}

async fn read_package(input: &str) -> Result<Package, anyhow::Error> {
    if input.starts_with("http") {
        return read_http_package(input).await;
    }

    // Assume it's a local file
    let content =
        std::fs::read(input).context(format_err!("read package from file '{}'", input))?;
    Package::decode(content.as_ref()).context("decode command")
}

async fn read_http_package(input: &str) -> Result<Package, anyhow::Error> {
    let body = reqwest::get(input).await?.bytes().await?;

    Package::decode(body).context("decode command")
}
