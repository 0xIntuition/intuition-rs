use crate::{error::LibError, ipfs::IPFSResolver};
use log::{info, warn};
use models::cached_image::CachedImage;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Maximum image size for download (10MB)
pub const MAX_IMAGE_SIZE: usize = 10 * 1024 * 1024;

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

impl Image {
    /// Combines the name and extension of an image
    pub fn combine_name_and_extension(&self) -> Result<String, LibError> {
        let image_output = self
            .extract_name_and_extension()
            .ok_or(LibError::ExtractNameAndExtension)?;
        Ok(format!("{}.{}", image_output.name, image_output.extension))
    }

    /// This function downloads an image from a URL and returns the bytes
    /// Supports both HTTP(S) URLs and IPFS URIs (ipfs://)
    pub async fn download(&self, ipfs_resolver: Option<&IPFSResolver>) -> Result<Option<Vec<u8>>, LibError> {
        info!("Downloading image from URL: {}", self.url);

        // Handle IPFS URIs with IPFSResolver
        if let Some(ipfs_cid) = self.url.strip_prefix("ipfs://") {
            if let Some(resolver) = ipfs_resolver {
                info!("Using IPFSResolver for IPFS URI: {}", ipfs_cid);
                let response = resolver.fetch_from_ipfs(ipfs_cid).await?;
                let bytes = response.bytes().await?.to_vec();

                if bytes.len() > MAX_IMAGE_SIZE {
                    return Err(LibError::ImageTooLarge(bytes.len(), MAX_IMAGE_SIZE));
                }

                info!("Downloaded {} bytes from IPFS: {}", bytes.len(), ipfs_cid);
                return Ok(Some(bytes));
            } else {
                warn!("IPFS URI detected but no IPFSResolver provided: {}", self.url);
                return Ok(None);
            }
        }

        // Fallback to HTTP for non-IPFS URIs
        let response = reqwest::get(&self.url).await?;
        if response.status() != reqwest::StatusCode::OK {
            warn!("Failed to download image, status: {}", response.status());
            return Ok(None);
        }

        let bytes = response.bytes().await?.to_vec();
        if bytes.len() > MAX_IMAGE_SIZE {
            return Err(LibError::ImageTooLarge(bytes.len(), MAX_IMAGE_SIZE));
        }

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
