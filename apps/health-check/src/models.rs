use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct HealthCheckRequest {
    pub contract_address: Option<String>,
    pub environment: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthCheckResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_stats: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_address: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HasuraGraphQLRequest {
    pub query: String,
}

#[derive(Debug, Deserialize)]
pub struct HasuraGraphQLResponse {
    pub data: Option<serde_json::Value>,
    pub errors: Option<Vec<serde_json::Value>>,
}

impl HealthCheckRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.contract_address.is_none() && self.environment.is_none() {
            return Err(
                "At least one of 'contract_address' or 'environment' must be provided".to_string(),
            );
        }
        Ok(())
    }
}
