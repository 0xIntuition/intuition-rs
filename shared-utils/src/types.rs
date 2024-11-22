use bytes::Bytes;
use chrono::{DateTime, Utc};
use macon::Builder;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub enum ClassificationModel {
    Falconsai,
    GPT4o,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub enum ClassificationStatus {
    Safe,
    Unsafe,
}

#[derive(Debug, Serialize, Deserialize, Builder, ToSchema)]
pub struct ImageClassificationResponse {
    pub status: ClassificationStatus,
    // full score json returned by the classification service
    pub score: String,
    pub model: ClassificationModel,
    pub date_classified: DateTime<Utc>,
    pub url: String,
}

/// Represents a multi-part handler
#[derive(Clone, Debug)]
pub struct MultiPartHandler {
    pub name: String,
    pub data: Bytes,
    pub content_type: String,
}
