use crate::{
    error::LibError,
    types::{MultiPartHandler, MultiPartHandlerJson},
};
use macon::Builder;
use reqwest::{
    Client, Response, StatusCode,
    multipart::{Form, Part},
};
use serde::Deserialize;
use std::time::Duration;
use tokio::time::sleep;
use tracing::warn;

/// The base delays for the retry mechanism and timeouts
pub const BASE_DELAY: Duration = Duration::from_secs(1);
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(5);
pub const PIN_TIMEOUT: Duration = Duration::from_secs(10);
pub const PINATA_API_URL: &str = "https://api.pinata.cloud";
pub const RETRY_ATTEMPTS: i32 = 3;
const MAX_IPFS_ERROR_BODY_LOG: usize = 2048;

fn truncate_for_log(body: &str, max_len: usize) -> String {
    if body.is_empty() {
        return String::new();
    }
    if body.len() <= max_len {
        return body.to_string();
    }
    let mut end = max_len;
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...[truncated]", &body[..end])
}

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
    pub pinata_gateway_token: Option<String>,
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
    pub async fn fetch_from_ipfs(&self, cid: &str) -> Result<Response, LibError> {
        let mut attempts = 0;

        let response = loop {
            attempts += 1;
            match self.fetch_from_ipfs_request(cid).await {
                Ok(body) => {
                    break Ok(body);
                }
                Err(e) => match self.handle_fetch_error(e.to_string(), attempts).await {
                    Ok(()) => continue,
                    Err(e) => break Err(e),
                },
            }
        }?;

        Ok(response)
    }

    /// Returns the list of IPFS nodes to fetch from in a form of a tuple.
    /// The first element is the node URL, the second element is a boolean
    /// indicating whether the node requires a pinata gateway token.
    /// The first node is the main node, the second node is the aquamarine node.
    /// The aquamarine node is used as a failover node. This node requires a
    /// pinata gateway token.
    fn get_ipfs_nodes(&self) -> Vec<(&str, Option<String>)> {
        vec![
            (&self.ipfs_fetch_url, None),
            (
                "https://aquamarine-tragic-mockingbird-747.mypinata.cloud",
                self.pinata_gateway_token.clone(),
            ),
        ]
    }

    /// Fetches a file from IPFS using the configured gateway.
    async fn fetch_from_ipfs_request(&self, cid: &str) -> Result<Response, LibError> {
        let nodes = self.get_ipfs_nodes();
        for (node, pinata_token) in nodes {
            match self.try_fetch_from_node(cid, node, &pinata_token).await {
                Ok(resp) => {
                    return Ok(resp);
                }
                Err(e) => warn!("IPFS fetch from {} failed: {}", node, e),
            }
        }

        Err(LibError::ResourceNotFoundError(
            "Resource does not exist".into(),
        ))
    }

    async fn try_fetch_from_node(
        &self,
        cid: &str,
        node: &str,
        token: &Option<String>,
    ) -> Result<Response, LibError> {
        let url = self.format_ipfs_fetch_url(cid, node, token);

        // Debug log the URL being called
        tracing::debug!("Attempting to fetch from URL: {}", url);

        self.http_client
            .get(&url)
            .timeout(self.fetch_timeout.unwrap_or(FETCH_TIMEOUT))
            .send()
            .await
            .map_err(|e| {
                // Log detailed error info
                tracing::error!("Request failed: {:#?}", e);
                if e.is_timeout() {
                    tracing::error!("Request timed out");
                }
                if e.is_connect() {
                    tracing::error!("Connection error");
                }
                if e.is_request() {
                    tracing::error!("Invalid request");
                }
                e.into()
            })
    }

    /// Formats the URL to add a remote pin to Pinata
    fn format_add_remote_pin_to_pinata(&self, cid: &str, name: &str) -> String {
        format!(
            "{}/api/v0/pin/remote/add?arg={}&service=Pinata&name={}",
            self.ipfs_upload_url, cid, name
        )
    }

    /// Formats the URL to fetch IPFS data
    fn format_ipfs_fetch_url(
        &self,
        cid: &str,
        ipfs_node: &str,
        pinata_gateway_token: &Option<String>,
    ) -> String {
        if let Some(token) = pinata_gateway_token {
            format!("{}/ipfs/{}?pinataGatewayToken={}", ipfs_node, cid, token)
        } else {
            format!("{}/ipfs/{}", ipfs_node, cid)
        }
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
    async fn handle_fetch_error(&self, e: String, attempts: i32) -> Result<(), LibError> {
        if attempts < self.retry_attempts.unwrap_or(RETRY_ATTEMPTS) {
            if e.contains("timed out") {
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
            Err(match e.contains("timed out") {
                true => LibError::TimeoutError("IPFS request timed out".into()),
                false => LibError::NetworkError(e),
            })
        }
    }

    /// Handles the error response for IPFS uploads
    async fn handle_upload_error_response(
        &self,
        status: StatusCode,
        attempts: i32,
        body: &str,
    ) -> Result<bool, LibError> {
        if !status.is_success() {
            let body_snippet = truncate_for_log(body, MAX_IPFS_ERROR_BODY_LOG);
            if body_snippet.is_empty() {
                warn!("IPFS upload failed with status {}", status);
            } else {
                warn!(
                    "IPFS upload failed with status {} body={}",
                    status, body_snippet
                );
            }

            if attempts < self.retry_attempts.unwrap_or(RETRY_ATTEMPTS) {
                let backoff = self
                    .base_delay
                    .unwrap_or(BASE_DELAY)
                    .mul_f64(2_f64.powi(attempts - 1));
                sleep(backoff).await;
                return Ok(true); // true means "should continue"
            }
            return Err(LibError::NetworkError(format!(
                "Upload failed with status {} body={}",
                status, body_snippet
            )));
        }
        Ok(false) // false means "don't continue"
    }

    /// Handles the retry error for IPFS uploads
    async fn handle_upload_retry_error(
        &self,
        e: LibError,
        attempts: i32,
    ) -> Result<(), LibError> {
        // Check if this is a timeout error for proper categorization
        let is_timeout = matches!(&e, LibError::Reqwest(req_err) if req_err.is_timeout());

        if attempts < self.retry_attempts.unwrap_or(RETRY_ATTEMPTS) {
            if is_timeout {
                warn!("Upload timed out, retrying... (attempt {})", attempts);
            } else {
                warn!("Upload error: {}, retrying... (attempt {})", e, attempts);
            }
            let backoff = self
                .base_delay
                .unwrap_or(BASE_DELAY)
                .mul_f64(2_f64.powi(attempts - 1));
            sleep(backoff).await;
            Ok(())
        } else if is_timeout {
            Err(LibError::TimeoutError("IPFS upload timed out".into()))
        } else {
            Err(e)
        }
    }

    /// Formats the multipart form to upload a file to IPFS.
    /// The field name "file" is required by the IPFS HTTP API.
    fn multipart_form(
        &self,
        data: bytes::Bytes,
        name: &str,
        content_type: &str,
    ) -> Result<Form, LibError> {
        Ok(Form::new().part(
            "file",
            Part::stream(data)
                .file_name(name.to_owned())
                .mime_str(content_type)
                .map_err(|e| LibError::InvalidInput(format!("Invalid MIME type: {}", e)))?,
        ))
    }

    /// Formats the multipart form to upload a json to IPFS
    fn multipart_form_json(&self, multi_part_handler: MultiPartHandlerJson) -> Form {
        Form::new().part(
            multi_part_handler.name.clone(),
            Part::bytes(serde_json::to_vec(&multi_part_handler.data).unwrap())
                .file_name(multi_part_handler.name.clone())
                .mime_str("application/json")
                .unwrap(),
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

            // Clone data only when needed for the request (Bytes clone is cheap - just Arc increment)
            match self
                .upload_to_ipfs_request(
                    multi_part_handler.data.clone(),
                    &multi_part_handler.name,
                    &multi_part_handler.content_type,
                )
                .await
            {
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();

                    if self
                        .handle_upload_error_response(status, attempts, &body)
                        .await?
                    {
                        continue;
                    }

                    // Attempt to parse JSON from the body
                    let result: IpfsResponse = serde_json::from_str(&body).map_err(|e| {
                        warn!(
                            "Failed to parse JSON response (status {}): {} body={}",
                            status,
                            e,
                            truncate_for_log(&body, MAX_IPFS_ERROR_BODY_LOG)
                        );
                        LibError::NetworkError(format!(
                            "Invalid JSON (status {}): {}",
                            status,
                            truncate_for_log(&body, MAX_IPFS_ERROR_BODY_LOG)
                        ))
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

    /// Uploads and pins a file to IPFS using the configured gateway
    /// Returns an [`IpfsResponse`] with the `name`, `hash` and `size` of
    /// the uploaded file.
    pub async fn upload_json_to_ipfs_and_pin(
        &self,
        multi_part_handler: MultiPartHandlerJson,
    ) -> Result<IpfsResponse, LibError> {
        let mut attempts = 0;

        loop {
            attempts += 1;

            match self
                .upload_json_to_ipfs_request(multi_part_handler.clone())
                .await
            {
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();

                    if self
                        .handle_upload_error_response(status, attempts, &body)
                        .await?
                    {
                        continue;
                    }

                    // Attempt to parse JSON from the body
                    let result: IpfsResponse = serde_json::from_str(&body).map_err(|e| {
                        warn!(
                            "Failed to parse JSON response (status {}): {} body={}",
                            status,
                            e,
                            truncate_for_log(&body, MAX_IPFS_ERROR_BODY_LOG)
                        );
                        LibError::NetworkError(format!(
                            "Invalid JSON (status {}): {}",
                            status,
                            truncate_for_log(&body, MAX_IPFS_ERROR_BODY_LOG)
                        ))
                    })?;

                    // Pin the CID to local IPFS
                    self.pin_with_cid(&result.hash).await?;
                    // Add a remote pin to Pinata
                    self.add_remote_pin_to_pinata(&result.hash, &multi_part_handler.name)
                        .await?;

                    return Ok(result);
                }
                Err(e) => match self.handle_upload_retry_error(e.into(), attempts).await {
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
        data: bytes::Bytes,
        name: &str,
        content_type: &str,
    ) -> Result<Response, LibError> {
        let url = self.format_ipfs_upload_url();
        tracing::info!(
            "Uploading to IPFS: url={}, file_name={}, content_type={}, size={}",
            url,
            name,
            content_type,
            data.len()
        );
        let form = self.multipart_form(data, name, content_type)?;
        let result = self.http_client.post(&url).multipart(form).send().await;
        if let Err(ref e) = result {
            tracing::error!(
                "IPFS upload request failed: url={}, is_connect={}, is_timeout={}, is_request={}, is_body={}, error={:#}",
                url,
                e.is_connect(),
                e.is_timeout(),
                e.is_request(),
                e.is_body(),
                e
            );
        }
        result.map_err(LibError::from)
    }

    /// Sends a request to upload a file to IPFS
    async fn upload_json_to_ipfs_request(
        &self,
        multi_part_handler: MultiPartHandlerJson,
    ) -> Result<Response, reqwest::Error> {
        self.http_client
            .post(self.format_ipfs_upload_url())
            .multipart(self.multipart_form_json(multi_part_handler.clone()))
            .send()
            .await
    }
}
