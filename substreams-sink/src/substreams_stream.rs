use crate::substreams::SubstreamsEndpoint;
use crate::{app::App, error::SubstreamError, pb::sf::substreams::v1::Modules, Cli};
use crate::{
    pb::sf::substreams::rpc::v2::{
        response::Message, BlockScopedData, BlockUndoSignal, Request, Response,
    },
    types::PreparedEndpointAndPackage,
};
use anyhow::{anyhow, Error};
use async_stream::try_stream;
use futures03::{Stream, StreamExt};
use log::{info, warn};
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tokio::time::sleep;
use tokio_retry::strategy::ExponentialBackoff;

/// The response from the substreams stream.
pub enum BlockResponse {
    New(BlockScopedData),
    Undo(BlockUndoSignal),
}
/// A struct that implements the substreams stream.
pub struct SubstreamsStream {
    /// The stream of blocks
    stream: Pin<Box<dyn Stream<Item = Result<BlockResponse, Error>> + Send>>,
    /// The CLI arguments
    pub cli: Cli,
}

impl SubstreamsStream {
    pub async fn new(cli: Cli, app: &App) -> Result<Self, SubstreamError> {
        let prepared_endpoint_package = PreparedEndpointAndPackage::new(&cli, app).await?;
        Ok(Self {
            stream: Box::pin(
                stream_blocks(
                    prepared_endpoint_package.endpoint.clone(),
                    app.app_state.load_persisted_cursor().await?,
                    Some(
                        prepared_endpoint_package
                            .mutable_modules(
                                app.env.intuition_contract_address.clone().to_lowercase(),
                            )
                            .await
                            .unwrap(),
                    ),
                    cli.module.to_string(),
                    prepared_endpoint_package.block_range.0,
                    prepared_endpoint_package.block_range.1,
                )
                .await,
            ),
            cli,
        })
    }

    /// Process the stream.
    pub async fn process(&mut self, app: &App) -> Result<(), SubstreamError> {
        loop {
            match self.stream.next().await {
                None => {
                    info!("Stream consumed");
                    break;
                }
                Some(Ok(BlockResponse::New(data))) => {
                    info!("New block: {}", data.final_block_height);
                    app.app_state.process_block_scoped_data(&data).await?;
                    app.app_state
                        .persist_cursor(data.cursor, data.final_block_height, &self.cli)
                        .await?;
                }
                Some(Ok(BlockResponse::Undo(undo_signal))) => {
                    app.app_state
                        .process_block_undo_signal(&undo_signal)
                        .await?;
                    app.app_state
                        .persist_cursor(
                            undo_signal.last_valid_cursor,
                            undo_signal
                                .last_valid_block
                                .ok_or(SubstreamError::BlockNotFound)?
                                .number,
                            &self.cli,
                        )
                        .await?;
                }
                Some(Err(err)) => {
                    warn!("!");
                    warn!("Stream terminated with error");
                    warn!("{:?}", err);
                    std::process::exit(1);
                }
            }
        }
        Ok(())
    }
}

// Create the Stream implementation that streams blocks with auto-reconnection.
async fn stream_blocks(
    endpoint: Arc<SubstreamsEndpoint>,
    cursor: Option<String>,
    modules: Option<Modules>,
    output_module_name: String,
    start_block_num: i64,
    stop_block_num: u64,
) -> impl Stream<Item = Result<BlockResponse, Error>> {
    let mut latest_cursor = cursor.unwrap_or_default();
    let mut backoff = ExponentialBackoff::from_millis(500).max_delay(Duration::from_secs(45));
    let mut last_progress_report = Instant::now();

    try_stream! {
        loop {
            println!("Blockstreams disconnected, connecting (endpoint {}, start block {}, stop block {}, cursor {})",
                &endpoint,
                start_block_num,
                stop_block_num,
                &latest_cursor
            );

            let result = endpoint.clone().substreams(Request {
                start_block_num,
                start_cursor: latest_cursor.clone(),
                stop_block_num,
                final_blocks_only: false,
                modules: modules.clone(),
                output_module: output_module_name.clone(),
                // There is usually no good reason for you to consume the stream development mode (so switching `true`
                // to `false`). If you do switch it, be aware that more than one output module will be send back to you,
                // and the current code in `process_block_scoped_data` (within your 'main.rs' file) expects a single
                // module.
                production_mode: true,
                debug_initial_store_snapshot_for_modules: vec![],
                noop_mode: false,
            }).await;

            match result {
                Ok(stream) => {
                    println!("Blockstreams connected");

                    let mut encountered_error = false;
                    for await response in stream{
                        match process_substreams_response(response, &mut last_progress_report).await {
                            BlockProcessedResult::BlockScopedData(block_scoped_data) => {
                                // Reset backoff because we got a good value from the stream
                                backoff = ExponentialBackoff::from_millis(500).max_delay(Duration::from_secs(45));

                                let cursor = block_scoped_data.cursor.clone();
                                yield BlockResponse::New(block_scoped_data);

                                latest_cursor = cursor;
                            },
                            BlockProcessedResult::BlockUndoSignal(block_undo_signal) => {
                                // Reset backoff because we got a good value from the stream
                                backoff = ExponentialBackoff::from_millis(500).max_delay(Duration::from_secs(45));

                                let cursor = block_undo_signal.last_valid_cursor.clone();
                                yield BlockResponse::Undo(block_undo_signal);

                                latest_cursor = cursor;
                            },
                            BlockProcessedResult::Skip() => {},
                            BlockProcessedResult::TonicError(status) => {
                                // Unauthenticated errors are not retried, we forward the error back to the
                                // stream consumer which handles it
                                if status.code() == tonic::Code::Unauthenticated {
                                    return Err(anyhow::Error::new(status.clone()))?;
                                }

                                println!("Received tonic error {:#}", status);
                                encountered_error = true;
                                break;
                            },
                        }
                    }

                    if !encountered_error {
                        println!("Stream completed, reached end block");
                        return
                    }
                },
                Err(e) => {
                    // We failed to connect and will try again; this is another
                    // case where we actually _want_ to back off in case we keep
                    // having connection errors.

                    println!("Unable to connect to endpoint: {:#}", e);
                }
            }

            // If we reach this point, we must wait a bit before retrying
            if let Some(duration) = backoff.next() {
                sleep(duration).await
            } else {
                return Err(anyhow!("backoff requested to stop retrying, quitting"))?;
            }
        }
    }
}

enum BlockProcessedResult {
    Skip(),
    BlockScopedData(BlockScopedData),
    BlockUndoSignal(BlockUndoSignal),
    TonicError(tonic::Status),
}

async fn process_substreams_response(
    result: Result<Response, tonic::Status>,
    last_progress_report: &mut Instant,
) -> BlockProcessedResult {
    let response = match result {
        Ok(v) => v,
        Err(e) => return BlockProcessedResult::TonicError(e),
    };

    match response.message {
        Some(Message::Session(session)) => {
            println!(
                "Received session message (Workers {}, Trace ID {})",
                session.max_parallel_workers, &session.trace_id
            );
            BlockProcessedResult::Skip()
        }
        Some(Message::BlockScopedData(block_scoped_data)) => {
            BlockProcessedResult::BlockScopedData(block_scoped_data)
        }
        Some(Message::BlockUndoSignal(block_undo_signal)) => {
            BlockProcessedResult::BlockUndoSignal(block_undo_signal)
        }
        Some(Message::Progress(progress)) => {
            if last_progress_report.elapsed() > Duration::from_secs(30) {
                let processed_bytes = progress.processed_bytes.unwrap_or_default();

                println!(
                    "Latest progress message received (Stages: {}, Jobs: {}, Processed Bytes: [Read: {}, Written: {}])",
                    progress.stages.len(),
                    progress.running_jobs.len(),
                    processed_bytes.total_bytes_read,
                    processed_bytes.total_bytes_written,
                );
                *last_progress_report = Instant::now();
            }

            // The `ModulesProgress` messages goal is to report active parallel processing happening
            // either to fill up backward (relative to your request's start block) some missing state
            // or pre-process forward blocks (again relative).
            //
            // You could log that in trace or accumulate to push as metrics. Here a snippet of code
            // that prints progress to standard out. If your `BlockScopedData` messages seems to never
            // arrive in production mode, it's because progresses is happening but not yet for the output
            // module you requested.
            //
            // let progresses: Vec<_> = progress
            //     .modules
            //     .iter()
            //     .filter_map(|module| {
            //         use crate::pb::sf::substreams::rpc::v2::module_progress::Type;

            //         if let Type::ProcessedRanges(range) = module.r#type.as_ref().unwrap() {
            //             Some(format!(
            //                 "{} @ [{}]",
            //                 module.name,
            //                 range
            //                     .processed_ranges
            //                     .iter()
            //                     .map(|x| x.to_string())
            //                     .collect::<Vec<_>>()
            //                     .join(", ")
            //             ))
            //         } else {
            //             None
            //         }
            //     })
            //     .collect();

            // println!("Progess {}", progresses.join(", "));

            BlockProcessedResult::Skip()
        }
        Some(Message::FatalError(_))
        | Some(Message::DebugSnapshotData(_))
        | Some(Message::DebugSnapshotComplete(_)) => BlockProcessedResult::Skip(),
        None => {
            println!("Got None on substream message");
            BlockProcessedResult::Skip()
        }
    }
}

impl Stream for SubstreamsStream {
    type Item = Result<BlockResponse, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.stream.poll_next_unpin(cx)
    }
}
