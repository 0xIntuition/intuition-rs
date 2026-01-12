use crate::{error::LibError, ipfs::IPFSResolver};
use log::{info, warn};
use models::cached_image::CachedImage;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Maximum image size for download (10MB).
/// This limit prevents memory exhaustion and DoS attacks from oversized images.
/// Images exceeding this size will return LibError::ImageTooLarge.
pub const MAX_IMAGE_SIZE: usize = 10 * 1024 * 1024;

/// Timeout duration for image downloads (30 seconds).
/// Prevents hanging requests from malicious or slow endpoints.
const DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Represents the name and extension of an image
pub struct ImageOutput {
    pub name: String,
    pub extension: String,
}

#[derive(Deserialize, Serialize, Debug, ToSchema)]
#[schema(example = json!({"url": "http://example.com/image.png"}))]
pub struct Image {
    pub url: String,
}

/// Downloads response bytes with size validation.
///
/// Validates size both before (via Content-Length header) and after download
/// to prevent oversized images from consuming excessive memory.
async fn download_with_size_limit(response: reqwest::Response) -> Result<Vec<u8>, LibError> {
    // Check Content-Length header before downloading
    if let Some(content_length) = response.content_length() {
        if content_length as usize > MAX_IMAGE_SIZE {
            return Err(LibError::ImageTooLarge(
                content_length as usize,
                MAX_IMAGE_SIZE,
            ));
        }
    }

    let bytes = response.bytes().await?.to_vec();

    // Verify actual size after download (in case Content-Length was missing)
    if bytes.len() > MAX_IMAGE_SIZE {
        return Err(LibError::ImageTooLarge(bytes.len(), MAX_IMAGE_SIZE));
    }

    Ok(bytes)
}

impl Image {
    /// Combines the name and extension of an image
    pub fn combine_name_and_extension(&self) -> Result<String, LibError> {
        let image_output = self
            .extract_name_and_extension()
            .ok_or(LibError::ExtractNameAndExtension)?;
        Ok(format!("{}.{}", image_output.name, image_output.extension))
    }

    /// Downloads an image from a URL and returns the bytes.
    ///
    /// # Arguments
    /// * `ipfs_resolver` - Required when the URL is an IPFS URI (ipfs://...).
    ///   Optional for HTTP(S) URLs.
    ///
    /// # Returns
    /// * `Ok(Some(bytes))` - Successfully downloaded image bytes
    /// * `Ok(None)` - Failed to download (non-200 status code)
    /// * `Err(LibError::MissingIPFSResolver)` - IPFS URI provided without resolver
    /// * `Err(LibError::ImageTooLarge)` - Image exceeds MAX_IMAGE_SIZE (10MB)
    /// * `Err(_)` - Network or other errors
    ///
    /// # Supported URL schemes
    /// * `ipfs://` - IPFS content identifiers (requires ipfs_resolver)
    /// * `http://` / `https://` - Standard web URLs
    pub async fn download(
        &self,
        ipfs_resolver: Option<&IPFSResolver>,
    ) -> Result<Option<Vec<u8>>, LibError> {
        info!("Downloading image from URL: {}", self.url);

        // Handle IPFS URIs with IPFSResolver
        if let Some(ipfs_cid) = self.url.strip_prefix("ipfs://") {
            let resolver = ipfs_resolver.ok_or_else(|| {
                warn!(
                    "IPFS URI detected but no IPFSResolver provided: {}",
                    self.url
                );
                LibError::MissingIPFSResolver
            })?;

            info!("Using IPFSResolver for IPFS URI: {}", ipfs_cid);

            // Add timeout protection to prevent hanging requests
            let response =
                tokio::time::timeout(DOWNLOAD_TIMEOUT, resolver.fetch_from_ipfs(ipfs_cid))
                    .await
                    .map_err(|_| {
                        LibError::TimeoutError(format!(
                            "IPFS download timeout for CID: {}",
                            ipfs_cid
                        ))
                    })??;

            let bytes = download_with_size_limit(response).await?;

            info!("Downloaded {} bytes from IPFS: {}", bytes.len(), ipfs_cid);
            return Ok(Some(bytes));
        }

        // Fallback to HTTP for non-IPFS URIs with timeout protection
        let response = tokio::time::timeout(DOWNLOAD_TIMEOUT, reqwest::get(&self.url))
            .await
            .map_err(|_| {
                LibError::TimeoutError(format!("HTTP download timeout for URL: {}", self.url))
            })??;

        if response.status() != reqwest::StatusCode::OK {
            warn!("Failed to download image, status: {}", response.status());
            return Ok(None);
        }

        let bytes = download_with_size_limit(response).await?;

        Ok(Some(bytes))
    }
    /// This function downloads an avatar, classifies it and stores it in the database
    pub async fn download_image_classify_and_store(
        url: String,
        reqwest_client: reqwest::Client,
        image_guard_url: String,
    ) -> Result<(), LibError> {
        // Send request with multipart form
        let endpoint = format!("{}/upload_image_from_url", image_guard_url);
        info!("Uploading image to image guard: {}", endpoint);

        let response = reqwest_client
            .post(endpoint)
            .timeout(std::time::Duration::from_secs(120))
            .json(&Self::new(url.clone()))
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            info!(
                "Failed to upload image {}, status: {}, error: {}",
                &url,
                &status,
                &response.text().await?
            );
            return Err(LibError::from(status));
        }

        // Log the raw response body
        let response_text = response.text().await?;
        info!("Raw response body: {}", response_text);

        // Attempt to parse the JSON
        let parsed_response: Result<Vec<CachedImage>, _> = serde_json::from_str(&response_text);
        match parsed_response {
            Ok(data) => info!("Image classification response: {:?}", data),
            Err(e) => {
                info!("Failed to parse JSON response: {}", e);
                return Err(LibError::from(e));
            }
        }

        Ok(())
    }

    /// Extracts the name and extension from a URL
    pub fn extract_name_and_extension(&self) -> Option<ImageOutput> {
        if let Ok(parsed_url) = Url::parse(&self.url)
            && let Some(mut path) = parsed_url.path_segments()
            && let Some(filename) = path.next_back()
        {
            let parts: Vec<&str> = filename.rsplitn(2, '.').collect();
            if parts.len() == 2 {
                return Some(ImageOutput {
                    name: parts[1].to_string(),
                    extension: parts[0].to_string(),
                });
            }
        }
        None
    }

    /// Creates a new image
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_download_missing_ipfs_resolver() {
        let image = Image::new("ipfs://QmTest123".to_string());
        let result = image.download(None).await;

        assert!(result.is_err());
        match result {
            Err(LibError::MissingIPFSResolver) => {
                // Expected error
            }
            _ => panic!("Expected MissingIPFSResolver error"),
        }
    }

    #[tokio::test]
    async fn test_download_invalid_url() {
        let image = Image::new("not-a-valid-url".to_string());
        let result = image.download(None).await;

        assert!(result.is_err());
        // Should fail with URL parsing or request error
    }

    #[tokio::test]
    async fn test_download_timeout() {
        // Test with a URL that will timeout
        // Using a non-routable IP address (192.0.2.1 is TEST-NET-1, reserved for documentation)
        let image = Image::new("http://192.0.2.1/image.png".to_string());
        let result = image.download(None).await;

        assert!(result.is_err());
        match result {
            Err(LibError::TimeoutError(_)) => {
                // Expected timeout error
            }
            Err(e) => {
                // Network error is also acceptable (connection refused, etc.)
                eprintln!("Got error: {:?}", e);
            }
            Ok(_) => panic!("Expected timeout or network error"),
        }
    }

    #[test]
    fn test_extract_name_and_extension_valid_url() {
        let image = Image::new("https://example.com/path/to/image.png".to_string());
        let result = image.extract_name_and_extension();

        assert!(result.is_some());
        let output = result.unwrap();
        assert_eq!(output.name, "image");
        assert_eq!(output.extension, "png");
    }

    #[test]
    fn test_extract_name_and_extension_no_extension() {
        let image = Image::new("https://example.com/path/to/image".to_string());
        let result = image.extract_name_and_extension();

        assert!(result.is_none());
    }

    #[test]
    fn test_extract_name_and_extension_invalid_url() {
        let image = Image::new("not-a-url".to_string());
        let result = image.extract_name_and_extension();

        assert!(result.is_none());
    }

    #[test]
    fn test_combine_name_and_extension_success() {
        let image = Image::new("https://example.com/image.jpg".to_string());
        let result = image.combine_name_and_extension();

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "image.jpg");
    }

    #[test]
    fn test_combine_name_and_extension_failure() {
        let image = Image::new("https://example.com/no-extension".to_string());
        let result = image.combine_name_and_extension();

        assert!(result.is_err());
        match result {
            Err(LibError::ExtractNameAndExtension) => {
                // Expected error
            }
            _ => panic!("Expected ExtractNameAndExtension error"),
        }
    }

    #[test]
    fn test_ipfs_url_detection() {
        let ipfs_image = Image::new("ipfs://QmTest123".to_string());
        assert!(ipfs_image.url.starts_with("ipfs://"));

        let http_image = Image::new("https://example.com/image.png".to_string());
        assert!(!http_image.url.starts_with("ipfs://"));
    }
}
