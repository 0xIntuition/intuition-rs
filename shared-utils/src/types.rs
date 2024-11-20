use macon::Builder;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum ClassificationModel {
    GPT4o,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClassificationStatus {
    Safe,
    Unsafe,
}

#[derive(Debug, Serialize, Deserialize, Builder)]
pub struct ImageClassificationResponse {
    pub status: ClassificationStatus,
    // full score json returned by the classification service
    pub score: String,
    pub model: ClassificationModel,
    pub date_classified: i64,
    pub url: String,
}
