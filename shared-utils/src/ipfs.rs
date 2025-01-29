use crate::{error::LibError, types::MultiPartHandler};
use log::warn;
use macon::Builder;
use reqwest::{
    multipart::{Form, Part},
    Client, Response, StatusCode,
};
use serde::Deserialize;
use std::time::Duration;
use tokio::time::sleep;

/// The base delays for the retry mechanism and timeouts
pub const BASE_DELAY: Duration = Duration::from_secs(1);
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(5);
pub const PIN_TIMEOUT: Duration = Duration::from_secs(10);
pub const PINATA_API_URL: &str = "https://api.pinata.cloud";
pub const RETRY_ATTEMPTS: i32 = 3;

/// The response from the IPFS gateway
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
#[derive(Clone, Builder)]
pub struct IPFSResolver {
    pub base_delay: Option<Duration>,
    pub fetch_timeout: Option<Duration>,
    pub http_client: Client,
    pub ipfs_fetch_url: String,
    pub ipfs_upload_url: String,
    pub pin_timeout: Option<Duration>,
    pub pinata_jwt: String,
    pub retry_attempts: Option<i32>,
}

impl IPFSResolver {
    /// Adds a remote pin to Pinata
    async fn add_remote_pin_to_pinata(
        &self,
        cid: &str,
        name: &str,
    ) -> Result<Response, reqwest::Error> {
        self.http_client
            .post(self.format_add_remote_pin_to_pinata(cid, name))
            .send()
            .await
    }
    /// Fetches a file and returns its content as a string from IPFS
    /// using the configured gateway.
    pub async fn fetch_from_ipfs(&self, cid: &str) -> Result<String, LibError> {
        let mut attempts = 0;

        let response = loop {
            attempts += 1;
            match self.fetch_from_ipfs_request(cid).await {
                Ok(resp) => {
                    // Check if the response contains the "resource does not exist" error
                    let body = resp.text().await.unwrap_or_default();
                    if body.contains("resource does not exist") {
                        return Err(LibError::ResourceNotFoundError(
                            "Resource does not exist".into(),
                        ));
                    }
                    break Ok(body);
                }
                Err(e) => match self.handle_fetch_error(e, attempts).await {
                    Ok(()) => continue,
                    Err(e) => break Err(e),
                },
            }
        }?;

        Ok(response)
    }

    /// Sends a request to fetch IPFS data. If the request fails, it will
    /// try to fetch the data from the Pinata node.
    /// TODO: improve this
    async fn fetch_from_ipfs_request(&self, cid: &str) -> Result<Response, reqwest::Error> {
        let response = self
            .http_client
            .get(self.format_ipfs_fetch_url(cid, &self.ipfs_fetch_url))
            .timeout(self.fetch_timeout.unwrap_or(FETCH_TIMEOUT))
            .send()
            .await;
        match response {
            Ok(resp) => Ok(resp),
            Err(e) => {
                if e.is_status() && e.status().unwrap() == StatusCode::NOT_FOUND {
                    warn!("IPFS fetch to local node failed: {}", e);
                    warn!("Fetching from Pinata node...");
                    self.http_client
                        .get(self.format_ipfs_fetch_url(cid, "https://gateway.pinata.cloud"))
                        .timeout(self.fetch_timeout.unwrap_or(FETCH_TIMEOUT))
                        .send()
                        .await
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Formats the URL to add a remote pin to Pinata
    fn format_add_remote_pin_to_pinata(&self, cid: &str, name: &str) -> String {
        format!(
            "{}/api/v0/pin/remote/add?arg={}&service=Pinata&name={}",
            self.ipfs_upload_url, cid, name
        )
    }

    /// Formats the URL to fetch IPFS data
    fn format_ipfs_fetch_url(&self, cid: &str, ipfs_node: &str) -> String {
        format!("{}/ipfs/{}", ipfs_node, cid)
    }

    /// Formats the URL to pin a hash to IPFS
    fn format_ipfs_pin_url(&self) -> String {
        format!("{}/pinning/pinByHash", PINATA_API_URL)
    }

    /// Formats the URL to upload a file to IPFS
    fn format_ipfs_upload_url(&self) -> String {
        format!("{}/api/v0/add", self.ipfs_upload_url)
    }

    /// Formats the URL to pin a CID to local IPFS
    fn format_pin_with_cid(&self, cid: &str) -> String {
        format!("{}/api/v0/pin/add?arg={}", self.ipfs_upload_url, cid)
    }

    /// Handles retry logic for pin operations
    async fn handle_existing_file_retry_logic(
        &self,
        e: reqwest::Error,
        attempts: i32,
    ) -> Result<(), LibError> {
        if attempts < self.retry_attempts.unwrap_or(RETRY_ATTEMPTS) {
            warn!("Pin error: {}, retrying... (attempt {})", e, attempts);
            let backoff = self
                .base_delay
                .unwrap_or(BASE_DELAY)
                .mul_f64(2_f64.powi(attempts - 1));
            sleep(backoff).await;
            Ok(())
        } else {
            Err(LibError::NetworkError(e.to_string()))
        }
    }

    /// Handles the error response for IPFS fetches
    async fn handle_fetch_error(&self, e: reqwest::Error, attempts: i32) -> Result<(), LibError> {
        if attempts < self.retry_attempts.unwrap_or(RETRY_ATTEMPTS) {
            if e.is_timeout() {
                warn!("IPFS request timed out, retrying... (attempt {})", attempts);
            } else {
                warn!("Network error: {}, retrying... (attempt {})", e, attempts);
            }
            let backoff = self
                .base_delay
                .unwrap_or(BASE_DELAY)
                .mul_f64(2_f64.powi(attempts - 1));
            sleep(backoff).await;
            Ok(())
        } else {
            Err(match e.is_timeout() {
                true => LibError::TimeoutError("IPFS request timed out".into()),
                false => LibError::NetworkError(e.to_string()),
            })
        }
    }

    /// Handles the error response for IPFS uploads
    async fn handle_upload_error_response(
        &self,
        status: StatusCode,
        attempts: i32,
    ) -> Result<bool, LibError> {
        if !status.is_success() {
            warn!("IPFS upload failed with status {}", status);

            if attempts < self.retry_attempts.unwrap_or(RETRY_ATTEMPTS) {
                let backoff = self
                    .base_delay
                    .unwrap_or(BASE_DELAY)
                    .mul_f64(2_f64.powi(attempts - 1));
                sleep(backoff).await;
                return Ok(true); // true means "should continue"
            }
            return Err(LibError::NetworkError(format!(
                "Upload failed with status {}",
                status
            )));
        }
        Ok(false) // false means "don't continue"
    }

    /// Handles the retry error for IPFS uploads
    async fn handle_upload_retry_error(
        &self,
        e: reqwest::Error,
        attempts: i32,
    ) -> Result<(), LibError> {
        if attempts < self.retry_attempts.unwrap_or(RETRY_ATTEMPTS) {
            warn!("Upload error: {}, retrying... (attempt {})", e, attempts);
            let backoff = self
                .base_delay
                .unwrap_or(BASE_DELAY)
                .mul_f64(2_f64.powi(attempts - 1));
            sleep(backoff).await;
            Ok(())
        } else {
            Err(match e.is_timeout() {
                true => LibError::TimeoutError("IPFS upload timed out".into()),
                false => LibError::NetworkError(e.to_string()),
            })
        }
    }

    /// Formats the multipart form to upload a file to IPFS
    fn multipart_form(&self, multi_part_handler: MultiPartHandler) -> Form {
        Form::new().part(
            multi_part_handler.name.clone(),
            Part::bytes(multi_part_handler.data.clone().to_vec())
                .file_name(multi_part_handler.name.clone()),
        )
    }

    /// Pins an already uploaded file hash to Pinata. Keep in mind that
    /// for this function to work, the file must have been uploaded to IPFS.
    #[allow(dead_code)]
    async fn pin_existing_file_hash(&self, hash: &str) -> Result<(), LibError> {
        let mut attempts = 0;
        loop {
            attempts += 1;
            match self.pin_to_ipfs_request(hash).await {
                Ok(response) => {
                    // Check if the response is successful
                    if !response.status().is_success() {
                        let status = response.status();
                        let error_text = response.text().await.unwrap_or_default();
                        warn!("Pinata pin failed: Status {}, Body: {}", status, error_text);

                        if attempts < self.retry_attempts.unwrap_or(RETRY_ATTEMPTS) {
                            let backoff = self
                                .base_delay
                                .unwrap_or(BASE_DELAY)
                                .mul_f64(2_f64.powi(attempts - 1));
                            sleep(backoff).await;
                            continue;
                        }

                        return Err(LibError::PinataError(format!(
                            "Failed to pin: Status {}, Body: {}",
                            status, error_text
                        )));
                    }
                    break Ok(());
                }
                Err(e) => match self.handle_existing_file_retry_logic(e, attempts).await {
                    Ok(()) => continue,
                    Err(e) => break Err(e),
                },
            }
        }
    }

    /// Pins a hash to keep it persistent in IPFS
    async fn pin_to_ipfs_request(&self, hash: &str) -> Result<Response, reqwest::Error> {
        let json_body = serde_json::json!({
            "hashToPin": hash,
            "pinataMetadata": {
                "name": format!("Pin request for {}", hash)
            }
        });

        self.http_client
            .post(self.format_ipfs_pin_url())
            .header("Authorization", format!("Bearer {}", self.pinata_jwt))
            .header("Content-Type", "application/json")
            .json(&json_body)
            .timeout(self.pin_timeout.unwrap_or(PIN_TIMEOUT))
            .send()
            .await
    }

    /// Uploads and pins a file to IPFS using the configured gateway
    /// Returns an [`IpfsResponse`] with the `name`, `hash` and `size` of
    /// the uploaded file.
    pub async fn upload_to_ipfs_and_pin(
        &self,
        multi_part_handler: MultiPartHandler,
    ) -> Result<IpfsResponse, LibError> {
        let mut attempts = 0;

        loop {
            attempts += 1;

            match self
                .upload_to_ipfs_request(multi_part_handler.clone())
                .await
            {
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();

                    if self.handle_upload_error_response(status, attempts).await? {
                        continue;
                    }

                    // Attempt to parse JSON from the body
                    let result: IpfsResponse = serde_json::from_str(&body).map_err(|e| {
                        warn!("Failed to parse JSON response: {}", e);
                        LibError::NetworkError(format!("Invalid JSON: {}", body))
                    })?;

                    // Pin the CID to local IPFS
                    self.pin_with_cid(&result.hash).await?;
                    // Add a remote pin to Pinata
                    self.add_remote_pin_to_pinata(&result.hash, &multi_part_handler.name)
                        .await?;

                    return Ok(result);
                }
                Err(e) => match self.handle_upload_retry_error(e, attempts).await {
                    Ok(()) => continue,
                    Err(e) => break Err(e),
                },
            }
        }?
    }

    /// Pins a CID to local IPFS
    async fn pin_with_cid(&self, cid: &str) -> Result<Response, reqwest::Error> {
        self.http_client
            .post(self.format_pin_with_cid(cid))
            .send()
            .await
    }

    /// Sends a request to upload a file to IPFS
    async fn upload_to_ipfs_request(
        &self,
        multi_part_handler: MultiPartHandler,
    ) -> Result<Response, reqwest::Error> {
        self.http_client
            .post(self.format_ipfs_upload_url())
            .multipart(self.multipart_form(multi_part_handler.clone()))
            .send()
            .await
    }
}
