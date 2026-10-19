use crate::{error::ApiError, types::Env};
use shared_utils::postgres::connect_to_db;
use sqlx::{Pool, Postgres};
use std::{sync::Arc, time::Duration};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// How long a `/upload_image_from_url` request waits for a free slot before it
/// is turned away with `503`. Bounded so a burst cannot pile up unbounded
/// queued requests.
pub const UPLOAD_QUEUE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, PartialEq)]
pub enum Flag {
    LocalWithClassification,
    LocalWithDbOnly,
    HfClassification,
}

impl Flag {
    pub fn enabled(env: &Env) -> Self {
        if let Some(true) = env.flag_local_with_classification {
            Flag::LocalWithClassification
        } else if let Some(true) = env.flag_local_with_db_only {
            Flag::LocalWithDbOnly
        } else {
            Flag::HfClassification
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pg_pool: Pool<Postgres>,
    pub image_api_schema: String,
    pub pinata_api_jwt: String,
    pub ipfs_upload_url: String,
    pub ipfs_fetch_url: String,
    pub hf_token: Option<String>,
    pub flag: Flag,
    /// Slots for concurrent `/upload_image_from_url` requests (memory bound).
    pub upload_slots: Arc<Semaphore>,
    /// Maximum wait for a slot, see [`UPLOAD_QUEUE_TIMEOUT`].
    pub upload_queue_timeout: Duration,
}

impl AppState {
    pub async fn new(env: &Env) -> Self {
        Self {
            pg_pool: connect_to_db(&env.indexer_database_url).await.unwrap(),
            image_api_schema: env.image_api_schema.clone(),
            pinata_api_jwt: env.pinata_api_jwt.clone(),
            ipfs_fetch_url: env.ipfs_gateway_url.clone(),
            ipfs_upload_url: env.ipfs_upload_url.clone(),
            hf_token: env.hf_token.clone(),
            flag: Flag::enabled(env),
            upload_slots: Arc::new(Semaphore::new(env.upload_from_url_max_concurrency.max(1))),
            upload_queue_timeout: UPLOAD_QUEUE_TIMEOUT,
        }
    }

    /// Wait (bounded) for a free `/upload_image_from_url` slot. The slot is
    /// released when the returned permit is dropped.
    pub async fn acquire_upload_slot(&self) -> Result<OwnedSemaphorePermit, ApiError> {
        match tokio::time::timeout(
            self.upload_queue_timeout,
            Arc::clone(&self.upload_slots).acquire_owned(),
        )
        .await
        {
            Ok(Ok(permit)) => Ok(permit),
            Ok(Err(_)) | Err(_) => Err(ApiError::Busy),
        }
    }
}
