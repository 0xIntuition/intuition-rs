use crate::{error::LibError, ipfs::IPFSResolver};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use log::{info, warn};
use models::cached_image::CachedImage;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Maximum allowed decoded image size in bytes (50 MB).
/// Used for validating image data before processing.
pub const MAX_IMAGE_SIZE: usize = 50 * 1024 * 1024;

/// Maximum HTTP body size to accept (accounts for base64 encoding overhead).
/// Base64 encoding adds ~33% overhead, so we allow 67MB to accept 50MB decoded images.
/// Formula: MAX_IMAGE_SIZE * 4 / 3, rounded up with extra margin for JSON wrapper.
pub const MAX_BODY_SIZE: usize = 70 * 1024 * 1024;

/// Custom UUID namespace for generating deterministic image content hashes.
/// This namespace is specific to this application to avoid collisions with other systems
/// that might use UUID v5 with standard namespaces.
/// Generated using: uuid v5 of "intuition.systems/image-guard/content" in DNS namespace.
const IMAGE_CONTENT_NAMESPACE: Uuid = Uuid::from_bytes([
    0x7a, 0x2c, 0x8f, 0x4e, 0x3b, 0x1d, 0x5a, 0x9c, 0xb6, 0x0e, 0x7f, 0x2d, 0x4c, 0x8a, 0x1b, 0x3e,
]);

/// Maximum image size for download (10MB).
/// This limit prevents memory exhaustion and DoS attacks from oversized images.
/// Images exceeding this size will return LibError::ImageTooLarge.
pub const MAX_DOWNLOAD_IMAGE_SIZE: usize = 10 * 1024 * 1024;

/// Timeout duration for image downloads (30 seconds).
/// Prevents hanging requests from malicious or slow endpoints.
const DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Represents the name and extension of an image
pub struct ImageOutput {
    pub name: String,
    pub extension: String,
}

/// Represents parsed data from a data URL
pub struct DataUrlParsed {
    pub mime_type: String,
    pub extension: String,
    pub data: Vec<u8>,
}

/// Represents parsed data URL info with deterministic naming
pub struct DataUrlInfo {
    pub parsed: DataUrlParsed,
    pub name: String,
    pub cache_key: String,
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
        if content_length as usize > MAX_DOWNLOAD_IMAGE_SIZE {
            return Err(LibError::ImageTooLarge(
                content_length as usize,
                MAX_DOWNLOAD_IMAGE_SIZE,
            ));
        }
    }

    let bytes = response.bytes().await?.to_vec();

    // Verify actual size after download (in case Content-Length was missing)
    if bytes.len() > MAX_DOWNLOAD_IMAGE_SIZE {
        return Err(LibError::ImageTooLarge(
            bytes.len(),
            MAX_DOWNLOAD_IMAGE_SIZE,
        ));
    }

    Ok(bytes)
}

/// Validates that image data has valid magic bytes for any supported image format.
/// Returns true if the data starts with valid image magic bytes.
pub fn is_valid_image_data(data: &[u8]) -> bool {
    if data.len() < 12 {
        return false;
    }

    data.starts_with(&[0xFF, 0xD8, 0xFF]) || // JPEG
    data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) || // PNG
    data.starts_with(&[0x47, 0x49, 0x46]) || // GIF
    data.starts_with(&[0x42, 0x4D]) || // BMP
    data.starts_with(&[0x49, 0x49, 0x2A, 0x00]) || // TIFF little-endian
    data.starts_with(&[0x4D, 0x4D, 0x00, 0x2A]) || // TIFF big-endian
    (data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP")) // WebP
}

/// Validates that image data magic bytes match the claimed extension.
/// Returns true if the magic bytes are valid for the given extension.
pub fn validate_magic_bytes_for_extension(data: &[u8], extension: &str) -> bool {
    if data.len() < 12 {
        return false;
    }

    match extension {
        "jpg" | "jpeg" => data.starts_with(&[0xFF, 0xD8, 0xFF]),
        "png" => data.starts_with(&[0x89, 0x50, 0x4E, 0x47]),
        "gif" => data.starts_with(&[0x47, 0x49, 0x46]),
        "bmp" => data.starts_with(&[0x42, 0x4D]),
        "tiff" => {
            data.starts_with(&[0x49, 0x49, 0x2A, 0x00])
                || data.starts_with(&[0x4D, 0x4D, 0x00, 0x2A])
        }
        "webp" => data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP"),
        _ => false,
    }
}
impl Image {
    /// Returns true if the URL is a data URL (base64 encoded)
    pub fn is_data_url(&self) -> bool {
        self.url.starts_with("data:")
    }

    /// Parses a data URL and returns the decoded bytes along with metadata
    /// Data URL format: data:[<mediatype>][;base64],<data>
    /// Example: data:image/jpeg;base64,/9j/4AAQ...
    /// Note: Only base64-encoded data URLs are supported
    pub fn parse_data_url(&self) -> Result<DataUrlParsed, LibError> {
        if !self.is_data_url() {
            return Err(LibError::InvalidDataUrl);
        }

        // Remove "data:" prefix
        let without_prefix = self
            .url
            .strip_prefix("data:")
            .ok_or(LibError::InvalidDataUrl)?;

        // Split by comma to separate metadata from data
        let (metadata, base64_data) = without_prefix
            .split_once(',')
            .ok_or(LibError::InvalidDataUrl)?;

        // Verify this is a base64-encoded data URL (per RFC 2397, non-base64 URLs use URL encoding)
        if !metadata.contains(";base64") {
            return Err(LibError::InvalidDataUrl);
        }

        // Parse the metadata (e.g., "image/jpeg;base64") and normalize to lowercase
        let mime_type = metadata
            .split(';')
            .next()
            .ok_or(LibError::InvalidDataUrl)?
            .to_lowercase();

        // Validate mime type is not empty
        if mime_type.is_empty() {
            return Err(LibError::InvalidDataUrl);
        }

        // Extract extension from mime type (e.g., "image/jpeg" -> "jpeg")
        let extension = mime_type
            .split('/')
            .nth(1)
            .ok_or(LibError::InvalidDataUrl)?
            .to_string(); // Already lowercase from mime_type

        // Validate extension is not empty
        if extension.is_empty() {
            return Err(LibError::InvalidDataUrl);
        }

        // Normalize composite extensions (e.g., "svg+xml" -> "svg")
        // This must be done before the whitelist check
        // Note: split('+').next() always returns Some for non-empty strings
        let extension = extension
            .split('+')
            .next()
            .unwrap_or(&extension)
            .to_string();

        // Whitelist allowed image extensions (raster formats only)
        // SVG is intentionally excluded due to security concerns (can contain embedded JavaScript)
        const ALLOWED_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff"];
        if !ALLOWED_EXTENSIONS.contains(&extension.as_str()) {
            return Err(LibError::InvalidInput(format!(
                "Unsupported image extension: {}. Allowed: {:?}",
                extension, ALLOWED_EXTENSIONS
            )));
        }

        // Validate size before allocating memory for decoding
        // Base64 encoded data is ~4/3 the size of decoded data, so check early
        // Use saturating_mul to prevent integer overflow on malicious input
        let estimated_decoded_size = base64_data.len().saturating_mul(3) / 4;
        if estimated_decoded_size > MAX_IMAGE_SIZE {
            return Err(LibError::InvalidInput(format!(
                "Estimated image size {} bytes exceeds maximum allowed size of {} bytes",
                estimated_decoded_size, MAX_IMAGE_SIZE
            )));
        }

        // Decode base64 data (strip whitespace that some encoders add)
        // Use bytes iterator to avoid allocating an intermediate String
        let base64_clean: Vec<u8> = base64_data
            .bytes()
            .filter(|b| !b.is_ascii_whitespace())
            .collect();
        let data = BASE64
            .decode(&base64_clean)
            .map_err(|e: base64::DecodeError| LibError::Base64Decode(e.to_string()))?;

        // Validate decoded data is not empty and within size limits
        if data.is_empty() {
            return Err(LibError::InvalidInput("Decoded image data is empty".into()));
        }
        if data.len() > MAX_IMAGE_SIZE {
            return Err(LibError::InvalidInput(format!(
                "Decoded image size {} bytes exceeds maximum allowed size of {} bytes",
                data.len(),
                MAX_IMAGE_SIZE
            )));
        }

        // Validate magic bytes match the claimed MIME type
        if !validate_magic_bytes_for_extension(&data, &extension) {
            return Err(LibError::InvalidInput(format!(
                "Image content does not match claimed type: {}",
                mime_type
            )));
        }

        Ok(DataUrlParsed {
            mime_type,
            extension,
            data,
        })
    }

    /// Parses a data URL and returns parsed data with deterministic naming metadata.
    pub fn parse_data_url_info(&self) -> Result<DataUrlInfo, LibError> {
        let parsed = self.parse_data_url()?;
        let name = Uuid::new_v5(&IMAGE_CONTENT_NAMESPACE, &parsed.data).to_string();
        let cache_key = format!("data:{}", name);

        Ok(DataUrlInfo {
            parsed,
            name,
            cache_key,
        })
    }

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
    /// * `Err(LibError::ImageTooLarge)` - Image exceeds MAX_DOWNLOAD_IMAGE_SIZE (10MB)
    /// * `Err(_)` - Network or other errors
    ///
    /// # Supported URL schemes
    /// * `data:` - Base64-encoded data URLs
    /// * `ipfs://` - IPFS content identifiers (requires ipfs_resolver)
    /// * `http://` / `https://` - Standard web URLs
    pub async fn download(
        &self,
        ipfs_resolver: Option<&IPFSResolver>,
    ) -> Result<Option<(Vec<u8>, Option<String>)>, LibError> {
        if self.is_data_url() {
            info!("Decoding data URL");
            let parsed = self.parse_data_url()?;
            return Ok(Some((parsed.data, Some(parsed.mime_type))));
        }

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
    /// For data URLs, generates a deterministic UUID name based on the data content
    pub fn extract_name_and_extension(&self) -> Option<ImageOutput> {
        // Handle data URLs
        if self.is_data_url() {
            if let Ok(info) = self.parse_data_url_info() {
                return Some(ImageOutput {
                    name: info.name,
                    extension: info.parsed.extension,
                });
            }
            return None;
        }

        // Handle regular HTTP URLs
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

    /// Returns the cache key for this image, used for database lookups and deduplication.
    ///
    /// For data URLs, returns "data:<uuid>" where uuid is a deterministic UUID v5 generated
    /// from the decoded image content. This ensures:
    /// - Identical image data always produces the same cache key
    /// - The key is short enough for PostgreSQL B-tree indexes (max 8KB)
    /// - No need to store the full base64-encoded data URL in the database
    ///
    /// UUID v5 was chosen over SHA256 because:
    /// - It produces a shorter, fixed-length output (36 chars vs 64 chars)
    /// - Both provide sufficient uniqueness for our deduplication needs
    /// - UUID format is more portable and database-friendly
    ///
    /// For regular HTTP URLs, returns the URL itself as the cache key.
    pub fn cache_key(&self) -> Option<String> {
        if self.is_data_url() {
            self.extract_name_and_extension()
                .map(|output| format!("data:{}", output.name))
        } else {
            Some(self.url.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_data_url() {
        let data_url = Image::new("data:image/png;base64,iVBORw0KGgo=".to_string());
        assert!(data_url.is_data_url());

        let http_url = Image::new("https://example.com/image.png".to_string());
        assert!(!http_url.is_data_url());
    }

    #[test]
    fn test_parse_data_url_valid_png() {
        // Minimal valid PNG (1x1 transparent pixel)
        let png_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
        let image = Image::new(format!("data:image/png;base64,{}", png_b64));

        let result = image.parse_data_url();
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert_eq!(parsed.mime_type, "image/png");
        assert_eq!(parsed.extension, "png");
        assert!(!parsed.data.is_empty());
    }

    #[test]
    fn test_parse_data_url_valid_jpeg() {
        // Minimal JPEG header (at least 12 bytes for magic byte validation)
        // JPEG starts with FF D8 FF
        let jpeg_b64 = "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAA=="; // Valid JPEG header, 24 bytes
        let image = Image::new(format!("data:image/jpeg;base64,{}", jpeg_b64));

        let result = image.parse_data_url();
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert_eq!(parsed.mime_type, "image/jpeg");
        assert_eq!(parsed.extension, "jpeg");
    }

    #[test]
    fn test_parse_data_url_svg_rejected_for_security() {
        // SVG is rejected due to security concerns (can contain embedded JavaScript)
        let svg_b64 = "PHN2Zz48L3N2Zz4="; // <svg></svg>
        let image = Image::new(format!("data:image/svg+xml;base64,{}", svg_b64));

        let result = image.parse_data_url();
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_data_url_rejects_non_base64() {
        let image = Image::new("data:image/png,raw-url-encoded-data".to_string());
        let result = image.parse_data_url();
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_data_url_rejects_empty_mime_type() {
        let image = Image::new("data:;base64,iVBORw0KGgo=".to_string());
        let result = image.parse_data_url();
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_data_url_rejects_invalid_mime_type() {
        let image = Image::new("data:invalid;base64,iVBORw0KGgo=".to_string());
        let result = image.parse_data_url();
        assert!(result.is_err()); // No "/" in mime type
    }

    #[test]
    fn test_parse_data_url_handles_whitespace() {
        // Base64 with whitespace (some encoders add newlines)
        // Use valid PNG (1x1 transparent pixel) with whitespace inserted
        let png_b64_with_ws = "iVBORw0KGgoAAAANSUhEUg\nAAAAEAAAABCAYAAAAfFcSJ\nAAAADUlEQVR42mNk+M9QDw\nADhgGAWjR9awAAAABJRU5E\nrkJggg==";
        let image = Image::new(format!("data:image/png;base64,{}", png_b64_with_ws));

        let result = image.parse_data_url();
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_name_and_extension_data_url_deterministic() {
        let png_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
        let image1 = Image::new(format!("data:image/png;base64,{}", png_b64));
        let image2 = Image::new(format!("data:image/png;base64,{}", png_b64));

        let output1 = image1.extract_name_and_extension().unwrap();
        let output2 = image2.extract_name_and_extension().unwrap();

        // Same content should produce same UUID name
        assert_eq!(output1.name, output2.name);
        assert_eq!(output1.extension, "png");
    }

    #[test]
    fn test_extract_name_and_extension_http_url() {
        let image = Image::new("https://example.com/path/to/image.jpg".to_string());
        let output = image.extract_name_and_extension().unwrap();

        assert_eq!(output.name, "image");
        assert_eq!(output.extension, "jpg");
    }
}

#[cfg(test)]
mod download_tests {
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
        assert_eq!(
            extract_extension_from_content_type("application/json"),
            None
        );
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
