use thiserror::Error;
use warp::reject;

#[derive(Error, Debug)]
pub enum GraphDBError {
    #[error("Atom not found")]
    AtomNotFound,
    #[error(transparent)]
    Env(#[from] envy::Error),
    #[error(transparent)]
    Model(#[from] models::error::ModelError),
    #[error(transparent)]
    Indradb(#[from] indradb::ValidationError),
    #[error(transparent)]
    IndradbError(#[from] indradb::Error),
    #[error("Get triple error: {0}")]
    GetTripleError(String),
    #[error("Postgres connect error: {0}")]
    PostgresConnectError(String),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    SharedUtils(#[from] shared_utils::error::LibError),
    #[error("Template error")]
    TemplateError,
    #[error(transparent)]
    Tera(#[from] tera::Error),
    #[error("Triple count error: {0}")]
    TripleCountError(String),
    #[error(transparent)]
    Uuid(#[from] uuid::Error),
}

impl reject::Reject for GraphDBError {}
