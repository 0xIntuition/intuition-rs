use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize)]
pub struct Env {
    pub classification_api_port: u16,
    pub indexer_database_url: String,
    pub flag_hf_classification: Option<bool>,
    pub hf_token: Option<String>,
    pub ipfs_gateway_url: String,
    pub ipfs_upload_url: String,
    pub flag_local_with_classification: Option<bool>,
    pub flag_local_with_db_only: Option<bool>,
    pub pinata_api_jwt: String,
    pub image_api_schema: String,
    /// Serve `/swagger-ui` and `/api-docs` (`ENABLE_SWAGGER=true`). Off by default.
    #[serde(default)]
    pub enable_swagger: bool,
    /// Maximum number of `/upload_image_from_url` requests processed at once
    /// (`UPLOAD_FROM_URL_MAX_CONCURRENCY`, default 4, minimum 1). Each in-flight
    /// request may buffer an image of up to `MAX_IMAGE_SIZE` bytes, so this
    /// bounds the memory the endpoint can hold.
    #[serde(default = "default_upload_from_url_max_concurrency")]
    pub upload_from_url_max_concurrency: usize,
}

/// Default for [`Env::upload_from_url_max_concurrency`].
pub const DEFAULT_UPLOAD_FROM_URL_MAX_CONCURRENCY: usize = 4;

fn default_upload_from_url_max_concurrency() -> usize {
    DEFAULT_UPLOAD_FROM_URL_MAX_CONCURRENCY
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

#[derive(serde::Deserialize, Debug, Serialize, ToSchema)]
pub struct LocalClassificationScore {
    pub is_nsfw: bool,
    pub confidence_percentage: f32,
    pub file_name: String,
}
/// This struct is used to parse the classification scores from the FalconSai API.
/// It is used to extract the normal and nsfw scores from the response.
#[derive(Debug, Serialize, ToSchema, Default)]
pub struct ClassificationScoreParsed {
    pub normal: f32,
    pub nsfw: f32,
}

impl ClassificationScoreParsed {
    /// Create a new `ClassificationScoreParsed` with unknown scores.
    pub fn unknown() -> Self {
        Self {
            normal: 0.0,
            nsfw: 0.0,
        }
    }
}

impl From<Vec<ClassificationScore>> for ClassificationScoreParsed {
    fn from(scores: Vec<ClassificationScore>) -> Self {
        let normal_score = scores
            .iter()
            .find(|s| s.label == "normal")
            .unwrap_or_else(|| {
                panic!("Missing 'normal' score");
            })
            .score;

        let nsfw_score = scores
            .iter()
            .find(|s| s.label == "nsfw")
            .unwrap_or_else(|| {
                panic!("Missing 'nsfw' score");
            })
            .score;

        Self {
            normal: normal_score,
            nsfw: nsfw_score,
        }
    }
}

/// Convert a `LocalClassificationScore` to a `ClassificationScoreParsed`
impl From<LocalClassificationScore> for ClassificationScoreParsed {
    fn from(score: LocalClassificationScore) -> Self {
        if score.is_nsfw {
            Self {
                normal: 0.0,
                nsfw: score.confidence_percentage,
            }
        } else {
            Self {
                normal: score.confidence_percentage,
                nsfw: 0.0,
            }
        }
    }
}
