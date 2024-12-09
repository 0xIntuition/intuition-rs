use std::fmt::Display;

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema, Default)]
pub enum ClassificationModel {
    #[default]
    FalconsaiNsfwImageDetection,
}

impl Display for ClassificationModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClassificationModel::FalconsaiNsfwImageDetection => {
                write!(
                    f,
                    "Fine-Tuned Vision Transformer (ViT) for NSFW Image Classification"
                )
            }
        }
    }
}
/// Represents a multi-part handler
#[derive(Clone, Debug)]
pub struct MultiPartHandler {
    pub name: String,
    pub data: Bytes,
    pub content_type: String,
}
