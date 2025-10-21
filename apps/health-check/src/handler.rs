use crate::{
    error::ApiError,
    models::{
        HasuraGraphQLRequest, HasuraGraphQLResponse, HealthCheckRequest, HealthCheckResponse,
    },
};
use axum::Json;
use k8s_openapi::api::core::v1::Pod;
use kube::{Api, Client, api::ListParams};
use log::{error, info};

const STATS_QUERY: &str = r#"
query Stats {
  stats {
    last_processed_block_number
    last_processed_block_timestamp
    last_updated
    total_accounts
    total_atoms
    total_fees
    total_positions
    total_signals
    total_triples
  }
}
"#;

pub async fn health_check_handler(
    Json(payload): Json<HealthCheckRequest>,
) -> Result<Json<HealthCheckResponse>, ApiError> {
    info!("Received health check request: {:?}", payload);

    // Validate the request
    payload.validate().map_err(ApiError::BadRequest)?;

    // For now, we only handle the environment case
    if let Some(environment) = &payload.environment {
        // Get contract address from k8s
        let contract_address = get_contract_address_from_k8s(environment).await.ok();

        match query_hasura_stats(environment).await {
            Ok(stats) => Ok(Json(HealthCheckResponse {
                status: "ok".to_string(),
                server_stats: Some(stats),
                contract_address,
            })),
            Err(ApiError::HasuraQueryFailed(err)) => {
                // Return offline status with error details instead of failing
                let error_json = serde_json::json!({
                    "error": err
                });
                Ok(Json(HealthCheckResponse {
                    status: "offline".to_string(),
                    server_stats: Some(error_json),
                    contract_address,
                }))
            }
            Err(e) => Err(e),
        }
    } else {
        // contract_address case - not implemented yet
        Err(ApiError::BadRequest(
            "contract_address handling is not yet implemented. Please use 'environment' instead."
                .to_string(),
        ))
    }
}

async fn query_hasura_stats(environment: &str) -> Result<serde_json::Value, ApiError> {
    let hasura_url = format!("http://{}-graphql-engine:8080/v1/graphql", environment);

    info!("Querying Hasura at: {}", hasura_url);

    let client = reqwest::Client::new();
    let graphql_request = HasuraGraphQLRequest {
        query: STATS_QUERY.to_string(),
    };

    let response = client
        .post(&hasura_url)
        .header("content-type", "application/json")
        .json(&graphql_request)
        .send()
        .await
        .map_err(|e| {
            error!("Failed to send request to Hasura: {}", e);
            ApiError::HasuraQueryFailed(format!("Failed to connect to Hasura: {}", e))
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        error!("Hasura returned error status {}: {}", status, error_text);
        return Err(ApiError::HasuraQueryFailed(format!(
            "Hasura returned status {}: {}",
            status, error_text
        )));
    }

    let hasura_response: HasuraGraphQLResponse = response.json().await.map_err(|e| {
        error!("Failed to parse Hasura response: {}", e);
        ApiError::HasuraQueryFailed(format!("Failed to parse response: {}", e))
    })?;

    if let Some(errors) = hasura_response.errors {
        error!("Hasura returned errors: {:?}", errors);
        return Err(ApiError::HasuraQueryFailed(format!(
            "GraphQL errors: {:?}",
            errors
        )));
    }

    let data = hasura_response.data.ok_or_else(|| {
        error!("Hasura response missing data field");
        ApiError::HasuraQueryFailed("Response missing data field".to_string())
    })?;

    // Extract the stats array and return the first element, or the whole data if extraction fails
    if let Some(first_stat) = data
        .get("stats")
        .and_then(|s| s.as_array())
        .and_then(|a| a.first())
    {
        return Ok(first_stat.clone());
    }

    // Fallback to returning the whole data object
    Ok(data)
}

async fn get_contract_address_from_k8s(environment: &str) -> Result<String, ApiError> {
    let namespace = environment;

    info!(
        "Fetching contract address from pod with label component=decoded-consumer,env={} in namespace {}",
        environment, namespace
    );

    let client = Client::try_default().await.map_err(|e| {
        error!("Failed to create k8s client: {}", e);
        ApiError::InternalServerError(format!("Failed to create k8s client: {}", e))
    })?;

    let pods: Api<Pod> = Api::namespaced(client, namespace);

    // List pods with the component and env labels
    let lp =
        ListParams::default().labels(&format!("component=decoded-consumer,env={}", environment));
    let pod_list = pods.list(&lp).await.map_err(|e| {
        error!(
            "Failed to list pods with label component=decoded-consumer,env={}: {}",
            environment, e
        );
        ApiError::InternalServerError(format!("Failed to list pods: {}", e))
    })?;

    let pod = pod_list.items.first().ok_or_else(|| {
        error!(
            "No pods found with label component=decoded-consumer,env={} in namespace {}",
            environment, namespace
        );
        ApiError::InternalServerError(format!("No consumer pods found in namespace {}", namespace))
    })?;

    // Find the INTUITION_CONTRACT_ADDRESS env var in the pod spec
    let contract_address = pod
        .spec
        .as_ref()
        .and_then(|spec| spec.containers.first())
        .and_then(|container| container.env.as_ref())
        .and_then(|env_vars| {
            env_vars.iter().find_map(|env| {
                if env.name == "INTUITION_CONTRACT_ADDRESS" {
                    env.value.clone()
                } else {
                    None
                }
            })
        })
        .ok_or_else(|| {
            error!(
                "INTUITION_CONTRACT_ADDRESS not found in decoded-consumer pod for environment {}",
                environment
            );
            ApiError::InternalServerError("INTUITION_CONTRACT_ADDRESS not found in pod".to_string())
        })?;

    info!(
        "Found contract address {} for environment {}",
        contract_address, environment
    );

    Ok(contract_address)
}
