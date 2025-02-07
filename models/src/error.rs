use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("Conversion error: {0}")]
    ConversionError(String),
    #[error("Database connection error: {0}")]
    DatabaseConnectionError(String),
    #[error("Decoding error: {0}")]
    DecodingError(String),
    #[error("Failed to delete data: {0}")]
    DeleteError(String),
    #[error(transparent)]
    FromUtf8Error(#[from] std::string::FromUtf8Error),
    #[error("Failed to insert data: {0}")]
    InsertError(String),
    #[error("Invalid atom type: {0}")]
    InvalidAtomType(String),
    #[error("Missing field: {0}")]
    MissingField(String),
    #[error("Failed to query data: {0}")]
    QueryError(String),
    #[error("Failed to parse data: {0}")]
    ParseError(String),
    #[error("Failed to serialize data: {0}")]
    SerializeError(String),
    #[error(transparent)]
    SqlError(#[from] sqlx::Error),
    #[error("Unexpected null value: {0}")]
    UnexpectedNull(String),
}
