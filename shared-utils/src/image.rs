use crate::error::LibError;
use log::info;
use models::image_guard::ImageGuard;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents the name and extension of an image
pub struct ImageOutput {
    pub name: String,
    pub extension: String,
}

#[derive(Deserialize, Serialize, Debug, ToSchema)]
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
    pub async fn download(&self) -> Result<Vec<u8>, LibError> {
        let response = reqwest::get(&self.url).await?;
        Ok(response.bytes().await?.to_vec())
    }
    /// This function downloads an avatar, classifies it and stores it in the database
    pub async fn download_image_classify_and_store(
        url: String,
        reqwest_client: reqwest::Client,
        image_guard_url: String,
    ) -> Result<(), LibError> {
        // Send request with multipart form
        let endpoint = format!("{}/upload_image_from_url", image_guard_url);

        let response: Vec<ImageGuard> = reqwest_client
            .post(endpoint)
            .json(&Image::new(url))
            .send()
            .await?
            .json()
            .await?;

        info!("Image classification response: {response:?}");
        Ok(())
    }

    /// Extracts the name and extension from a URL
    pub fn extract_name_and_extension(&self) -> Option<ImageOutput> {
        if let Ok(parsed_url) = Url::parse(&self.url) {
            if let Some(path) = parsed_url.path_segments() {
                if let Some(filename) = path.last() {
                    let parts: Vec<&str> = filename.rsplitn(2, '.').collect();
                    if parts.len() == 2 {
                        return Some(ImageOutput {
                            name: parts[1].to_string(),
                            extension: parts[0].to_string(),
                        });
                    }
                }
            }
        }
        None
    }

    /// Creates a new image
    pub fn new(url: String) -> Self {
        Self { url }
    }
}
