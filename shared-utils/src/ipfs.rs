use crate::{error::LibError, types::MultiPartImage};
use bytes::Bytes;
use log::warn;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct IpfsResponse {
    pub name: String,
    pub hash: String,
    pub size: String,
}

/// This represents the current IPFS resolver implementation.
/// It's responsible for fetching IPFS data from the configured
/// IPFS gateway. Currently we are using [`reqwest`] as HTTP client
/// and we are implementing a simple exponential backoff retry mechanism
/// to fetch the data from IPFS. You can configure the number of attempts
/// by changing the `IPFS_RETRY_ATTEMPTS` constant.
#[derive(Clone)]
pub struct IPFSResolver {
    pub http_client: Client,
    pub ipfs_gateway_url: String,
    pub retry_attempts: i32,
}

impl IPFSResolver {
    pub fn new(client: Client, ipfs_gateway_url: String, retry_attempts: i32) -> Self {
        Self {
            http_client: client,
            ipfs_gateway_url,
            retry_attempts,
        }
    }

    /// Formats the URL to fetch IPFS data
    pub fn format_url(&self, cid: &str) -> String {
        format!("{}/ipfs/{}", self.ipfs_gateway_url, cid)
    }

    /// Fetches a file and returns its content as a string from IPFS
    /// using the configured gateway.
    pub async fn fetch_from_ipfs(&self, cid: &str) -> Result<String, LibError> {
        let url = self.format_url(cid);

        let mut attempts = 0;
        let base_delay = Duration::from_secs(1);

        let response = loop {
            attempts += 1;
            match self
                .http_client
                .get(&url)
                .timeout(Duration::from_millis(3000))
                .send()
                .await
            {
                Ok(resp) => break Ok(resp),
                Err(e) if attempts < self.retry_attempts => {
                    if e.is_timeout() {
                        warn!("IPFS request timed out, retrying... (attempt {})", attempts);
                    } else {
                        warn!("Network error: {}, retrying... (attempt {})", e, attempts);
                    }

                    // Exponential backoff: 1s, 2s, 4s, 8s, etc.
                    let backoff = base_delay.mul_f64(2_f64.powi(attempts - 1));
                    sleep(backoff).await;
                }
                Err(e) => {
                    break Err(match e.is_timeout() {
                        true => LibError::TimeoutError("IPFS request timed out".into()),
                        false => LibError::NetworkError(e.to_string()),
                    })
                }
            }
        }?;

        response
            .text()
            .await
            .map_err(|e| LibError::NetworkError(e.to_string()))
    }

    /// Uploads and pins a file to IPFS using the configured gateway
    /// Returns the CID of the uploaded file
    pub async fn upload_to_ipfs(
        &self,
        multi_part_image: MultiPartImage,
    ) -> Result<IpfsResponse, LibError> {
        let mut attempts = 0;
        let base_delay = Duration::from_secs(1);

        loop {
            attempts += 1;
            let form = reqwest::multipart::Form::new().part(
                multi_part_image.name.clone(),
                reqwest::multipart::Part::bytes(multi_part_image.image_data.clone().to_vec())
                    .file_name(multi_part_image.name.clone()),
            );

            match self
                .http_client
                .post(format!("{}/api/v0/add", self.ipfs_gateway_url))
                .multipart(form)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();

                    if !status.is_success() {
                        warn!("IPFS upload failed with status {}: {}", status, body);

                        if attempts < self.retry_attempts {
                            let backoff = base_delay.mul_f64(2_f64.powi(attempts - 1));
                            sleep(backoff).await;
                            continue;
                        }
                        return Err(LibError::NetworkError(format!(
                            "Upload failed with status {}",
                            status
                        )));
                    }

                    // Attempt to parse JSON from the body
                    let result: IpfsResponse = serde_json::from_str(&body).map_err(|e| {
                        warn!("Failed to parse JSON response: {}", e);
                        LibError::NetworkError(format!("Invalid JSON: {}", body))
                    })?;

                    self.pin_hash(&result.hash).await?;
                    return Ok(result);
                }
                Err(e) if attempts < self.retry_attempts => {
                    warn!("Upload error: {}, retrying... (attempt {})", e, attempts);
                    let backoff = base_delay.mul_f64(2_f64.powi(attempts - 1));
                    sleep(backoff).await;
                }
                Err(e) => {
                    break Err(match e.is_timeout() {
                        true => LibError::TimeoutError("IPFS upload timed out".into()),
                        false => LibError::NetworkError(e.to_string()),
                    })
                }
            }
        }?
    }

    /// Pins a hash to keep it persistent in IPFS
    async fn pin_hash(&self, hash: &str) -> Result<(), LibError> {
        let mut attempts = 0;
        let base_delay = Duration::from_secs(1);

        loop {
            attempts += 1;
            match self
                .http_client
                .post(format!(
                    "{}/api/v0/pin/add?arg={}",
                    self.ipfs_gateway_url, hash
                ))
                .timeout(Duration::from_secs(10))
                .send()
                .await
            {
                Ok(_) => break Ok(()),
                Err(e) if attempts < self.retry_attempts => {
                    warn!("Pin error: {}, retrying... (attempt {})", e, attempts);
                    let backoff = base_delay.mul_f64(2_f64.powi(attempts - 1));
                    sleep(backoff).await;
                }
                Err(e) => {
                    break Err(match e.is_timeout() {
                        true => LibError::TimeoutError("IPFS pin request timed out".into()),
                        false => LibError::NetworkError(e.to_string()),
                    })
                }
            }
        }
    }
}
