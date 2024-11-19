use crate::config::IPFS_RETRY_ATTEMPTS;
use crate::error::ConsumerError;
use log::warn;
use reqwest::Client;
use std::time::Duration;
use tokio::time::sleep;

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
}

impl IPFSResolver {
    pub fn new(client: Client, ipfs_gateway_url: String) -> Self {
        Self {
            http_client: client,
            ipfs_gateway_url,
        }
    }

    /// Formats the URL to fetch IPFS data
    pub fn format_url(&self, cid: &str) -> String {
        format!("{}/ipfs/{}", self.ipfs_gateway_url, cid)
    }

    /// Fetches a file and returns its content as a string from IPFS
    /// using the configured gateway.
    pub async fn fetch_from_ipfs(&self, cid: &str) -> Result<String, ConsumerError> {
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
                Err(e) if attempts < IPFS_RETRY_ATTEMPTS => {
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
                        true => ConsumerError::TimeoutError("IPFS request timed out".into()),
                        false => ConsumerError::NetworkError(e.to_string()),
                    })
                }
            }
        }?;

        response
            .text()
            .await
            .map_err(|e| ConsumerError::NetworkError(e.to_string()))
    }
}
