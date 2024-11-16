use crate::error::ConsumerError;
use log::warn;
use reqwest::Client;
use std::env;
use std::time::Duration;

/// Fetches a file from IPFS using configured gateway
pub async fn fetch_from_ipfs(cid: &str) -> Result<String, ConsumerError> {
    let gateway_url = env::var("IPFS_GATEWAY_URL")
        .map_err(|_| ConsumerError::Ipfs("IPFS_GATEWAY_URL not set".into()))?;

    let url = format!("{}/ipfs/{}", gateway_url, cid);

    let client = Client::new();
    let mut attempts = 0;
    let response = loop {
        attempts += 1;
        match client
            .get(&url)
            .timeout(Duration::from_millis(3000))
            .send()
            .await
        {
            Ok(resp) => break Ok(resp),
            Err(e) if attempts < 10 => {
                if e.is_timeout() {
                    warn!("IPFS request timed out, retrying... (attempt {})", attempts);
                } else {
                    warn!("Network error: {}, retrying... (attempt {})", e, attempts);
                }
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
