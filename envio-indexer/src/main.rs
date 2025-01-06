use app::App;
use clap::{Parser, ValueEnum};
use error::IndexerError;
use models::raw_logs::RawLog;
use tokio::time::{sleep, Duration};

mod app;
mod error;

/// The network to index. Currently only Base Sepolia is supported.
#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
enum Network {
    BaseSepolia,
}

/// The CLI arguments
#[derive(Parser)]
pub struct Args {
    /// The network to index
    #[arg(short, long)]
    network: Network,
}

#[tokio::main]
async fn main() -> Result<(), IndexerError> {
    let app = App::init()?;

    // Get the current height of the server, that represents the last block indexed
    let height = app.client.get_height().await?;
    println!("server height is {}", height);

    // Get the query for the given network
    let mut query = app.query()?;

    loop {
        let res = app.client.get_events(query.clone()).await?;

        for batch in res.data {
            println!("batch: {:?}", batch);
            for event in batch {
                let raw_log = RawLog::try_from(event)?;
                println!("raw_log: {:?}", raw_log);
            }
        }

        println!("scanned up to block {}", res.next_block);

        if let Some(archive_height) = res.archive_height {
            if archive_height < res.next_block {
                // wait if we are at the head
                // notice we use explicit get_height in order to not waste data requests.
                // get_height is lighter compared to spamming data requests at the tip.
                while app.client.get_height().await? < res.next_block {
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }

        // continue query from next_block
        query.from_block = res.next_block;
    }
}
