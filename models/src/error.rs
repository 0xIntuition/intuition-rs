use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("Conversion error: {0}")]
    ConversionError(String),
    #[error("Database connection error: {0}")]
    DatabaseConnectionError(String),
    #[error("Failed to delete data: {0}")]
    DeleteError(String),
    #[error("Failed to insert data: {0}")]
    InsertError(String),
    #[error("Invalid atom type: {0}")]
    InvalidAtomType(String),
    #[error("Failed to query data: {0}")]
    QueryError(String),
    #[error(transparent)]
    SqlError(#[from] sqlx::Error),
    #[error("Unexpected null value: {0}")]
    UnexpectedNull(String),
}
