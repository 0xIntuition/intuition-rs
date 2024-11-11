use anyhow::{Context, Result};
use log::info;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use zip::ZipArchive;

use crate::app_context::SqsProducer;
use crate::types::RawLog;

/// This struct is responsible for processing a zip file containing records of our contract events
/// and feeding them to the queue.
pub struct ProcessZip<'a> {
    pub zip_path: &'a Path,
}

impl<'a> ProcessZip<'a> {
    /// Creates a new ProcessZip instance
    pub fn new(zip_path: &'a Path) -> Self {
        Self { zip_path }
    }

    /// Processes a zip file containing event logs and deserializes them into RawLog objects.
    ///
    /// # Returns
    /// A vector of RawLog objects parsed from .txt files in the zip archive.
    /// Processing stops if a line starts with "Stream" or "Latest".
    ///
    /// # Errors
    /// Returns error if:
    /// - Cannot open or read the zip file
    /// - Cannot parse a line as valid JSON
    /// - Cannot deserialize JSON into RawLog
    pub async fn process_file(&self) -> Result<Vec<RawLog>> {
        let file = File::open(self.zip_path).context("Failed to open zip file")?;
        let mut archive = ZipArchive::new(file).context("Failed to create ZIP archive")?;
        let mut results = Vec::new();

        for i in 0..archive.len() {
            let file = archive.by_index(i).context("Failed to read zip entry")?;

            // Only process .txt files
            if !file.name().ends_with(".txt") {
                continue;
            }

            let reader = BufReader::new(file);
            for line in reader.lines() {
                let line = line.context("Failed to read line")?;

                // Early return on control lines
                if line.starts_with("Stream") || line.starts_with("Latest") {
                    return Ok(results);
                }

                results.push(serde_json::from_str(&line).context("Failed to deserialize line")?);
            }
        }

        info!("Finished processing zip file!");

        Ok(results)
    }

    /// Processes a zip file, reading each line from contained txt files
    /// and deserializing them into a Vec of RawLog, then sending them to the queue
    pub async fn process_file_and_send_to_queue(&self, sqs: &SqsProducer) -> Result<Vec<RawLog>> {
        let results = self.process_file().await?;
        for raw_log in &results {
            sqs.send_message(serde_json::to_string(&raw_log)?).await?;
            info!("Sent a raw log to the queue! {raw_log:#?}");
        }

        info!("Finished sending messages to queue!");

        Ok(results)
    }
}
