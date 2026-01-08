use crate::error::LibError;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use log::{info, warn};
use models::cached_image::CachedImage;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

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

impl Image {
    /// Returns true if the URL is a data URL (base64 encoded)
    pub fn is_data_url(&self) -> bool {
        self.url.starts_with("data:")
    }

    /// Parses a data URL and returns the decoded bytes along with metadata
    /// Data URL format: data:[<mediatype>][;base64],<data>
    /// Example: data:image/jpeg;base64,/9j/4AAQ...
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

        // Parse the metadata (e.g., "image/jpeg;base64")
        let mime_type = metadata
            .split(';')
            .next()
            .ok_or(LibError::InvalidDataUrl)?
            .to_string();

        // Extract extension from mime type (e.g., "image/jpeg" -> "jpeg")
        let extension = mime_type
            .split('/')
            .nth(1)
            .ok_or(LibError::InvalidDataUrl)?
            .to_string();

        // Decode base64 data
        let data = BASE64
            .decode(base64_data)
            .map_err(|e: base64::DecodeError| LibError::Base64Decode(e.to_string()))?;

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
    /// For data URLs, generates a UUID name and extracts extension from mime type
    pub fn extract_name_and_extension(&self) -> Option<ImageOutput> {
        // Handle data URLs
        if self.is_data_url() {
            if let Ok(parsed) = self.parse_data_url() {
                return Some(ImageOutput {
                    name: Uuid::new_v4().to_string(),
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
}
