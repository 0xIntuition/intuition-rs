use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("Failed to upsert data for book: {0}")]
    BookUpsertError(String),
    #[error("Failed to upsert data for byte object: {0}")]
    ByteObjectUpsertError(String),
    #[error("Failed to upsert data for cached image: {0}")]
    CachedImageUpsertError(String),
    #[error("Failed to upsert data for caip10: {0}")]
    Caip10UpsertError(String),
    #[error("Failed to upsert data for claim: {0}")]
    ClaimUpsertError(String),
    #[error("Failed to insert data for failed log: {0}")]
    FailedLogInsertError(String),
    #[error("Failed to upsert data for initialize: {0}")]
    InitializeUpsertError(String),
    #[error("Failed to upsert data for json object: {0}")]
    JsonObjectUpsertError(String),
    #[error("Failed to upsert data for organization: {0}")]
    OrganizationUpsertError(String),
    #[error("Failed to upsert data for person: {0}")]
    PersonUpsertError(String),
    #[error("Failed to upsert data for stats hour: {0}")]
    StatsHourUpsertError(String),
    #[error("Failed to upsert data for redemption: {0}")]
    RedemptionUpsertError(String),
    #[error("Failed to insert data for share price change: {0}")]
    SharePriceChangeInsertError(String),
    #[error("Failed to upsert data for share price change: {0}")]
    SharePriceChangeUpsertError(String),
    #[error("Failed to upsert data for stats: {0}")]
    StatsUpsertError(String),
    #[error("Failed to upsert data for text object: {0}")]
    TextObjectUpsertError(String),
    #[error("Failed to upsert data for thing: {0}")]
    ThingUpsertError(String),
    #[error("Failed to upsert data for triple term: {0}")]
    TripleTermUpsertError(String),
    #[error("Failed to upsert data for triple vault: {0}")]
    TripleVaultUpsertError(String),
    #[error("Failed to insert data for vault from share price: {0}")]
    VaultInsertFromSharePriceError(String),
    #[error("Failed to insert data for vault: {0}")]
    VaultInsertError(String),
    #[error("Failed to upsert data for vault: {0}")]
    VaultUpsertError(String),
    #[error("Failed to upsert data for term: {0}")]
    TermUpsertError(String),
    #[error("Failed to insert data for account: {0}")]
    AccountInsertError(String),
    #[error("Failed to insert data for atom value: {0}")]
    AtomValueInsertError(String),
    #[error("Conversion error: {0}")]
    ConversionError(String),
    #[error("Database connection error: {0}")]
    DatabaseConnectionError(String),
    #[error("Failed to insert data for deposit: {0}")]
    DepositInsertError(String),
    #[error("Failed to insert data for event: {0}")]
    EventInsertError(String),
    #[error("Decoding error: {0}")]
    DecodingError(String),
    #[error("Failed to delete data: {0}")]
    DeleteError(String),
    #[error(transparent)]
    FromUtf8Error(#[from] std::string::FromUtf8Error),
    #[error("Failed to insert data: {0}")]
    InsertError(String),
    #[error("Failed to insert data for fee transfer: {0}")]
    FeeTransferInsertError(String),
    #[error("Failed to insert data for protocol fee accrued: {0}")]
    ProtocolFeeAccruedInsertError(String),
    #[error("Failed to insert data for position: {0}")]
    PositionInsertError(String),
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
    #[error("Failed to update data: {0}")]
    UpdateError(String),
}
