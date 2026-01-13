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

/// Extracts file extension from Content-Type header (e.g., "image/png" -> "png")
pub fn extract_extension_from_content_type(content_type: &str) -> Option<String> {
    let content_type = content_type.to_lowercase();

    // Handle common image MIME types
    if content_type.starts_with("image/") {
        let subtype = content_type.strip_prefix("image/")?;

        // Remove any parameters (e.g., "image/png; charset=utf-8")
        let extension = subtype.split(';').next()?.trim();

        // Map MIME subtypes to common file extensions
        let normalized = match extension {
            "jpeg" => "jpg",
            "svg+xml" => "svg",
            other => other,
        };

        return Some(normalized.to_string());
    }

    None
}

/// Detects image format from magic bytes and returns file extension
pub fn detect_format_from_bytes(data: &[u8]) -> Option<String> {
    match data.get(0..4)? {
        bytes if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) => Some("jpg".to_string()),
        bytes if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) => Some("png".to_string()),
        bytes if bytes.starts_with(&[0x47, 0x49, 0x46]) => Some("gif".to_string()),
        bytes if bytes.starts_with(&[0x42, 0x4D]) => Some("bmp".to_string()),
        bytes if bytes.starts_with(&[0x49, 0x49, 0x2A, 0x00]) => Some("tiff".to_string()),
        bytes if bytes.starts_with(&[0x4D, 0x4D, 0x00, 0x2A]) => Some("tiff".to_string()),
        bytes if bytes.starts_with(&[0x52, 0x49, 0x46, 0x46]) => {
            // WebP starts with RIFF, check for WEBP signature at bytes 8-12
            if data.len() >= 12 && &data[8..12] == b"WEBP" {
                Some("webp".to_string())
            } else {
                None
            }
        }
        _ => None,
    }
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

    /// Downloads an image from a URL and returns the bytes and Content-Type header.
    ///
    /// # Arguments
    /// * `ipfs_resolver` - Required when the URL is an IPFS URI (ipfs://...).
    ///   Optional for HTTP(S) URLs.
    ///
    /// # Returns
    /// * `Ok(Some((bytes, content_type)))` - Successfully downloaded image bytes and optional Content-Type
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
    ) -> Result<Option<(Vec<u8>, Option<String>)>, LibError> {
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

            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            let bytes = download_with_size_limit(response).await?;

            info!("Downloaded {} bytes from IPFS: {}", bytes.len(), ipfs_cid);
            return Ok(Some((bytes, content_type)));
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

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let bytes = download_with_size_limit(response).await?;

        Ok(Some((bytes, content_type)))
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

    #[test]
    fn test_extract_extension_from_content_type() {
        assert_eq!(
            extract_extension_from_content_type("image/png"),
            Some("png".to_string())
        );
        assert_eq!(
            extract_extension_from_content_type("image/jpeg"),
            Some("jpg".to_string())
        );
        assert_eq!(
            extract_extension_from_content_type("image/gif; charset=utf-8"),
            Some("gif".to_string())
        );
        assert_eq!(
            extract_extension_from_content_type("image/svg+xml"),
            Some("svg".to_string())
        );
        assert_eq!(
            extract_extension_from_content_type("IMAGE/PNG"),
            Some("png".to_string())
        );
        assert_eq!(extract_extension_from_content_type("text/html"), None);
        assert_eq!(extract_extension_from_content_type("application/json"), None);
    }

    #[test]
    fn test_detect_format_from_bytes() {
        // PNG magic bytes
        let png_bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(
            detect_format_from_bytes(&png_bytes),
            Some("png".to_string())
        );

        // JPEG magic bytes
        let jpeg_bytes = vec![0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(
            detect_format_from_bytes(&jpeg_bytes),
            Some("jpg".to_string())
        );

        // GIF magic bytes
        let gif_bytes = vec![0x47, 0x49, 0x46, 0x38, 0x39, 0x61];
        assert_eq!(
            detect_format_from_bytes(&gif_bytes),
            Some("gif".to_string())
        );

        // BMP magic bytes
        let bmp_bytes = vec![0x42, 0x4D, 0x00, 0x00];
        assert_eq!(
            detect_format_from_bytes(&bmp_bytes),
            Some("bmp".to_string())
        );

        // WebP magic bytes (RIFF + WEBP signature)
        let webp_bytes = vec![
            0x52, 0x49, 0x46, 0x46, 0x00, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50,
        ];
        assert_eq!(
            detect_format_from_bytes(&webp_bytes),
            Some("webp".to_string())
        );

        // Invalid bytes
        let invalid_bytes = vec![0x00, 0x00, 0x00, 0x00];
        assert_eq!(detect_format_from_bytes(&invalid_bytes), None);

        // Empty bytes
        let empty_bytes: Vec<u8> = vec![];
        assert_eq!(detect_format_from_bytes(&empty_bytes), None);
    }
}
