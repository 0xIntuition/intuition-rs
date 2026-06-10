use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Response returned by the `/classify` endpoint.
///
/// `classified` is `false` when no content classifier is configured
/// (i.e., the service is running in db-only mode). In that case `safe`,
/// `score`, and `model` are omitted entirely — no verdict was produced, and
/// callers must not interpret the response as either safe or unsafe.
#[derive(Debug, Serialize, ToSchema)]
pub struct ClassificationResponse {
    /// Whether the image passed the safety threshold. Omitted when no
    /// classifier produced a verdict.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safe: Option<bool>,
    /// JSON-serialised classification scores (e.g. `{"normal":0.82,"nsfw":0.18}`).
    /// Omitted when no classifier produced a verdict.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<String>,
    /// Name of the classification model used. Omitted when no classifier
    /// produced a verdict.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// `true` when a classifier was active and produced a real verdict;
    /// `false` when the service is running without a classifier configured.
    pub classified: bool,
}

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
