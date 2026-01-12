use crate::error::LibError;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use log::{info, warn};
use models::cached_image::CachedImage;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Maximum allowed image size in bytes (50 MB).
/// Used for validating image data before processing and for HTTP body limits.
pub const MAX_IMAGE_SIZE: usize = 50 * 1024 * 1024;

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

#[derive(Deserialize, Serialize, Debug, ToSchema)]
#[schema(example = json!({"url": "http://example.com/image.png"}))]
pub struct Image {
    pub url: String,
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
        let without_prefix = self.url.strip_prefix("data:").ok_or(LibError::InvalidDataUrl)?;

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
        let extension = extension.split('+').next().unwrap_or(&extension).to_string();

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
        let estimated_decoded_size = base64_data.len() * 3 / 4;
        if estimated_decoded_size > MAX_IMAGE_SIZE {
            return Err(LibError::InvalidInput(format!(
                "Estimated image size {} bytes exceeds maximum allowed size of {} bytes",
                estimated_decoded_size,
                MAX_IMAGE_SIZE
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

        // Validate decoded data is not empty
        if data.is_empty() {
            return Err(LibError::InvalidInput("Decoded image data is empty".into()));
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

    /// Combines the name and extension of an image
    pub fn combine_name_and_extension(&self) -> Result<String, LibError> {
        let image_output = self
            .extract_name_and_extension()
            .ok_or(LibError::ExtractNameAndExtension)?;
        Ok(format!("{}.{}", image_output.name, image_output.extension))
    }

    /// This function downloads an image from a URL and returns the bytes
    /// For data URLs, it decodes the base64 data directly
    pub async fn download(&self) -> Result<Option<Vec<u8>>, LibError> {
        if self.is_data_url() {
            info!("Decoding data URL");
            let parsed = self.parse_data_url()?;
            return Ok(Some(parsed.data));
        }

        info!("Downloading image from URL: {}", self.url);
        let response = reqwest::get(&self.url).await?;
        if response.status() != reqwest::StatusCode::OK {
            warn!("Failed to download image, status: {}", response.status());
            return Ok(None);
        }
        Ok(Some(response.bytes().await?.to_vec()))
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
            if let Ok(parsed) = self.parse_data_url() {
                // Use UUID v5 with a namespace based on the data content for deterministic naming
                // This ensures the same data URL always produces the same filename
                let name = Uuid::new_v5(&Uuid::NAMESPACE_OID, &parsed.data).to_string();
                return Some(ImageOutput {
                    name,
                    extension: parsed.extension,
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
