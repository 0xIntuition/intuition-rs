use std::fmt::Display;

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema, Default)]
pub enum ClassificationModel {
    #[default]
    Falconsai,
}

impl Display for ClassificationModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClassificationModel::Falconsai => write!(f, "Falconsai"),
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
