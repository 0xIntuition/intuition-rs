use serde::{Deserialize, Serialize};
use shared_utils::postgres::PostgresEnv;
use utoipa::ToSchema;

use crate::error::ApiError;

#[derive(Deserialize)]
pub struct Env {
    pub api_port: u16,
    pub ipfs_gateway_url: String,
    pub ipfs_upload_url: String,
    pub pinata_api_jwt: String,
    pub hf_token: String,
    #[serde(flatten)]
    pub postgres: PostgresEnv,
}

/// A multipart request with an image
#[derive(ToSchema)]
pub struct MultipartRequest {
    #[schema(format = Binary)]
    _image: String,
}

#[derive(serde::Deserialize, Debug, Serialize)]
pub struct ClassificationScore {
    pub label: String,
    pub score: f32,
}

/// This struct is used to parse the classification scores from the FalconSai API.
/// It is used to extract the normal and nsfw scores from the response.
#[derive(Debug, Serialize, ToSchema, Default)]
pub struct ClassificationScoreParsed {
    pub normal: f32,
    pub nsfw: f32,
}

impl ClassificationScoreParsed {
    pub fn from(scores: Vec<ClassificationScore>) -> Result<Self, ApiError> {
        if scores.len() != 2 {
            return Err(ApiError::ExternalService(
                "Expected 2 classification scores".into(),
            ));
        }

        let normal_score = scores
            .iter()
            .find(|s| s.label == "normal")
            .ok_or(ApiError::ExternalService("Missing 'normal' score".into()))?
            .score;

        let nsfw_score = scores
            .iter()
            .find(|s| s.label == "nsfw")
            .ok_or(ApiError::ExternalService("Missing 'nsfw' score".into()))?
            .score;

        Ok(ClassificationScoreParsed {
            normal: normal_score,
            nsfw: nsfw_score,
        })
    }
}
