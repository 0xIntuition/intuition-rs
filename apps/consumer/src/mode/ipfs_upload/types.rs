use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct IpfsUploadMessage {
    pub image: String,
}
