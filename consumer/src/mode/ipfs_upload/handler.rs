use log::warn;
use shared_utils::image::Image;

use crate::{error::ConsumerError, mode::types::IpfsUploadConsumerContext};

use super::types::IpfsUploadMessage;

impl IpfsUploadMessage {
    pub async fn process(
        &self,
        ipfs_upload_consumer_context: &IpfsUploadConsumerContext,
    ) -> Result<(), ConsumerError> {
        if !self.image.is_empty() {
            let image_upload = Image::download_image_classify_and_store(
                self.image.clone(),
                ipfs_upload_consumer_context.reqwest_client.clone(),
                ipfs_upload_consumer_context.image_guard_url.clone(),
            )
            .await;
            if let Err(e) = image_upload {
                warn!("Failed to upload image: {}", e);
            }
        }
        Ok(())
    }
}
