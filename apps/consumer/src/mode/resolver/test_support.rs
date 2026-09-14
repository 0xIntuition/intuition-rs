//! Helpers for DB-backed consumer tests.
//!
//! These tests run against a real Postgres because the bugs they guard are ordering
//! and atomicity bugs between SQL statements. They are skipped (with a message) when
//! `TEST_DATABASE_URL` is not set:
//!
//! ```text
//! docker run -d --name intuition-test-pg -e POSTGRES_PASSWORD=postgres \
//!   -e POSTGRES_DB=storage -p 55432:5432 postgres:17
//! TEST_DATABASE_URL=postgres://postgres:postgres@127.0.0.1:55432/storage cargo test -p consumer
//! ```
//!
//! Every test gets its own schema (the models are schema-parameterised) containing only
//! the tables the atom resolution path touches, taken from the consolidated Hasura
//! migration, plus the `updated_at` trigger.
use crate::{
    ConsumerArgs,
    app_context::ServerInitialize,
    config::{ContractInstance, ContractVersion, Env},
    error::ConsumerError,
    mode::{
        resolver::types::{ResolverConsumerMessage, ResolverMessageType},
        types::{ConsumerMode, DecodedConsumerContext, ResolverConsumerContext},
    },
    traits::{BasicConsumer, Message},
};
use alloy::{
    primitives::{Address, FixedBytes},
    providers::{DynProvider, ProviderBuilder},
};
use async_trait::async_trait;
use models::{atom::Atom, traits::SimpleCrud, types::FixedBytesWrapper};
use reqwest::Client;
use shared_utils::ipfs::IPFSResolver;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::{
    str::FromStr,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};

const DDL: &str = r#"
CREATE SCHEMA {S};
CREATE TYPE {S}.account_type AS ENUM ('Default', 'AtomWallet', 'ProtocolVault');
CREATE TYPE {S}.event_type AS ENUM ('AtomCreated', 'TripleCreated', 'Deposited', 'Redeemed', 'FeesTransfered', 'Initialized', 'ProtocolFeeAccrued');
CREATE TYPE {S}.atom_type AS ENUM (
  'Unknown', 'Account', 'Thing', 'ThingPredicate', 'Person', 'PersonPredicate',
  'Organization', 'OrganizationPredicate', 'Book', 'LikeAction', 'FollowAction', 'Keywords',
  'Caip10', 'JsonObject', 'TextObject', 'ByteObject', 'Caip22'
);
CREATE TYPE {S}.atom_resolving_status AS ENUM ('Pending', 'Resolved', 'Failed');
CREATE TABLE {S}.account (
  id TEXT PRIMARY KEY NOT NULL,
  atom_id TEXT,
  label TEXT NOT NULL,
  image TEXT,
  type {S}.account_type NOT NULL
);
CREATE TABLE {S}.atom (
  term_id TEXT PRIMARY KEY NOT NULL,
  wallet_id TEXT NOT NULL,
  creator_id TEXT NOT NULL,
  data TEXT,
  raw_data TEXT NOT NULL,
  type {S}.atom_type NOT NULL,
  emoji TEXT,
  label TEXT,
  image TEXT,
  value_id TEXT,
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL,
  resolving_status {S}.atom_resolving_status NOT NULL DEFAULT 'Pending',
  log_index BIGINT NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now()
);
CREATE FUNCTION {S}.touch_atom_updated_at() RETURNS trigger AS $fn$
BEGIN
  NEW.updated_at = now();
  RETURN NEW;
END
$fn$ LANGUAGE plpgsql;
CREATE TRIGGER atom_updated_at_trigger BEFORE UPDATE ON {S}.atom
  FOR EACH ROW EXECUTE FUNCTION {S}.touch_atom_updated_at();
CREATE TABLE {S}.event (
  id TEXT PRIMARY KEY NOT NULL,
  type {S}.event_type NOT NULL,
  atom_id TEXT,
  triple_id TEXT,
  fee_transfer_id TEXT,
  deposit_id TEXT,
  redemption_id TEXT,
  protocol_fee_accrued_id TEXT,
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL
);
CREATE TABLE {S}.thing (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT,
  description TEXT,
  image TEXT,
  url TEXT
);
CREATE TABLE {S}.person (
  id TEXT PRIMARY KEY NOT NULL,
  identifier TEXT,
  name TEXT,
  description TEXT,
  image TEXT,
  url TEXT,
  email TEXT
);
CREATE TABLE {S}.organization (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT,
  description TEXT,
  image TEXT,
  url TEXT,
  email TEXT
);
CREATE TABLE {S}.book (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT,
  description TEXT,
  genre TEXT,
  url TEXT
);
CREATE TABLE {S}.caip10 (
  id TEXT PRIMARY KEY NOT NULL,
  namespace TEXT NOT NULL,
  chain_id INTEGER NOT NULL,
  account_address TEXT NOT NULL
);
CREATE TABLE {S}.json_object (
  id TEXT PRIMARY KEY NOT NULL,
  data JSONB NOT NULL
);
CREATE TABLE {S}.text_object (
  id TEXT PRIMARY KEY NOT NULL,
  data TEXT NOT NULL
);
CREATE TABLE {S}.byte_object (
  id TEXT PRIMARY KEY NOT NULL,
  data BYTEA NOT NULL
);
CREATE TABLE {S}.atom_value (
  id TEXT PRIMARY KEY NOT NULL,
  account_id TEXT,
  thing_id TEXT,
  person_id TEXT,
  organization_id TEXT,
  book_id TEXT,
  caip10_id TEXT,
  json_object_id TEXT,
  text_object_id TEXT,
  byte_object_id TEXT
);
"#;

/// A dedicated schema on the test database.
pub struct TestDb {
    pub pool: PgPool,
    pub schema: String,
}

impl TestDb {
    /// Connects and creates a fresh schema. Returns `None` (after printing why) when
    /// `TEST_DATABASE_URL` is not set so the test can skip.
    pub async fn connect() -> Option<Self> {
        let url = match std::env::var("TEST_DATABASE_URL") {
            Ok(url) => url,
            Err(_) => {
                eprintln!("TEST_DATABASE_URL not set; skipping DB-backed test");
                return None;
            }
        };
        let schema = format!("consumer_test_{}", uuid::Uuid::new_v4().simple());
        // sqlx checks enum type names against the name Postgres reports, which is
        // schema-qualified for types outside the search path. Production keeps the
        // enums in `public`; put the test schema on the search path so `atom_type`
        // etc. resolve to bare names there too.
        let search_path = format!("SET search_path TO {schema}, public");
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .after_connect(move |conn, _meta| {
                let search_path = search_path.clone();
                Box::pin(async move {
                    sqlx::Executor::execute(conn, search_path.as_str()).await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await
            .expect("connect to TEST_DATABASE_URL");
        sqlx::raw_sql(&DDL.replace("{S}", &schema))
            .execute(&pool)
            .await
            .expect("create test schema");
        Some(Self { pool, schema })
    }

    /// Drops the schema. Call at the end of a passing test.
    pub async fn drop(self) {
        sqlx::raw_sql(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .execute(&self.pool)
            .await
            .expect("drop test schema");
    }

    /// Inserts an atom row directly (bypassing the model) so `created_at` /
    /// `updated_at` can be placed in the past.
    pub async fn seed_atom(&self, seed: &AtomSeed<'_>) -> FixedBytesWrapper {
        let query = format!(
            r#"
            INSERT INTO {0}.atom (
                term_id, wallet_id, creator_id, data, raw_data, type, emoji, label, image,
                value_id, block_number, created_at, transaction_hash, resolving_status,
                log_index, updated_at
            ) VALUES (
                $1, 'wallet', 'creator', $2, '0x', $3::text::{0}.atom_type, NULL, $4, NULL,
                $1, 1, now() - make_interval(secs => $5), '0xtx',
                $6::text::{0}.atom_resolving_status, 0, now() - make_interval(secs => $7)
            )
            "#,
            self.schema
        );
        sqlx::query(&query)
            .bind(seed.id.clone())
            .bind(seed.data)
            .bind(seed.atom_type)
            .bind(seed.label)
            .bind(seed.created_secs_ago as f64)
            .bind(seed.status)
            .bind(seed.updated_secs_ago as f64)
            .execute(&self.pool)
            .await
            .expect("seed atom");
        seed.id.clone()
    }

    /// Loads an atom that must exist.
    pub async fn atom(&self, id: &FixedBytesWrapper) -> Atom {
        Atom::find_by_id(id.clone(), &self.schema, &self.pool)
            .await
            .expect("query atom")
            .expect("atom exists")
    }
}

/// Builds a 32-byte term id from a single repeated byte.
pub fn term_id(byte: u8) -> FixedBytesWrapper {
    FixedBytesWrapper(FixedBytes::repeat_byte(byte))
}

/// Row to seed through [`TestDb::seed_atom`].
pub struct AtomSeed<'a> {
    pub id: FixedBytesWrapper,
    pub data: Option<&'a str>,
    pub atom_type: &'a str,
    pub status: &'a str,
    pub label: Option<&'a str>,
    pub created_secs_ago: i64,
    pub updated_secs_ago: i64,
}

impl<'a> AtomSeed<'a> {
    /// A `Pending` / `Unknown` atom created and updated "now", like the decoded
    /// consumer's first upsert.
    pub fn new(byte: u8, data: Option<&'a str>) -> Self {
        Self {
            id: term_id(byte),
            data,
            atom_type: "Unknown",
            status: "Pending",
            label: None,
            created_secs_ago: 0,
            updated_secs_ago: 0,
        }
    }
    pub fn atom_type(mut self, atom_type: &'a str) -> Self {
        self.atom_type = atom_type;
        self
    }
    pub fn status(mut self, status: &'a str) -> Self {
        self.status = status;
        self
    }
    pub fn label(mut self, label: Option<&'a str>) -> Self {
        self.label = label;
        self
    }
    pub fn created_secs_ago(mut self, secs: i64) -> Self {
        self.created_secs_ago = secs;
        self
    }
    pub fn updated_secs_ago(mut self, secs: i64) -> Self {
        self.updated_secs_ago = secs;
        self
    }
}

/// A message captured by [`RecordingClient`], together with the atom row as it was in
/// the database at the moment the message was sent (for `Atom` resolver messages).
#[derive(Debug, Clone)]
pub struct SentMessage {
    pub body: String,
    pub atom_snapshot: Option<Atom>,
}

/// `BasicConsumer` stand-in that records outgoing messages instead of touching Redis.
pub struct RecordingClient {
    pool: PgPool,
    schema: String,
    sent: Mutex<Vec<SentMessage>>,
}

impl RecordingClient {
    pub fn new(pool: PgPool, schema: String) -> Arc<Self> {
        Arc::new(Self {
            pool,
            schema,
            sent: Mutex::new(Vec::new()),
        })
    }

    /// Everything sent so far, in order.
    pub fn sent(&self) -> Vec<SentMessage> {
        self.sent.lock().unwrap().clone()
    }

    /// Only the resolver `Atom` messages sent so far, in order.
    pub fn atom_messages(&self) -> Vec<SentMessage> {
        self.sent()
            .into_iter()
            .filter(|m| m.atom_snapshot.is_some() && m.body.contains("\"Atom\""))
            .collect()
    }
}

#[async_trait]
impl BasicConsumer for RecordingClient {
    async fn consume_message(&self, _message: Message) -> Result<(), ConsumerError> {
        Ok(())
    }

    async fn process_messages(&self, _mode: ConsumerMode) -> Result<(), ConsumerError> {
        Ok(())
    }

    async fn receive_message(&self) -> Result<Vec<Message>, ConsumerError> {
        Ok(Vec::new())
    }

    async fn send_message(
        &self,
        message: String,
        _group_id: Option<String>,
    ) -> Result<(), ConsumerError> {
        let atom_snapshot = match serde_json::from_str::<ResolverConsumerMessage>(&message) {
            Ok(ResolverConsumerMessage {
                message: ResolverMessageType::Atom(atom_id),
            }) => {
                Atom::find_by_id(
                    FixedBytesWrapper::from_str(&atom_id)?,
                    &self.schema,
                    &self.pool,
                )
                .await?
            }
            _ => None,
        };
        self.sent.lock().unwrap().push(SentMessage {
            body: message,
            atom_snapshot,
        });
        Ok(())
    }
}

fn env(db: &TestDb) -> Env {
    Env {
        backend_schema: db.schema.clone(),
        rpc_url_base: Some("http://127.0.0.1:9".to_string()),
        intuition_contract_address: Some("0x0000000000000000000000000000000000000001".to_string()),
        ..Default::default()
    }
}

/// A resolver context wired to the test schema. IPFS and RPC point at a closed local
/// port with short timeouts so any accidental network call fails fast.
pub fn resolver_context(db: &TestDb, client: Arc<dyn BasicConsumer>) -> ResolverConsumerContext {
    let provider = DynProvider::new(
        ProviderBuilder::new().connect_http("http://127.0.0.1:9".parse().unwrap()),
    );
    let universal_resolver = Arc::new(crate::UniversalResolver::new(Address::ZERO, provider));
    let ipfs_resolver = IPFSResolver::builder()
        .http_client(Client::new())
        .ipfs_upload_url("http://127.0.0.1:9".to_string())
        .ipfs_fetch_url("http://127.0.0.1:9".to_string())
        .pinata_jwt(String::new())
        .pinata_gateway_token(String::new())
        .fetch_timeout(Duration::from_millis(200))
        .base_delay(Duration::from_millis(10))
        .retry_attempts(1)
        .build();
    ResolverConsumerContext {
        client,
        ipfs_resolver,
        universal_resolver,
        pg_pool: db.pool.clone(),
        server_initialize: ServerInitialize {
            args: ConsumerArgs {
                mode: "resolver".to_string(),
            },
            env: env(db),
        },
    }
}

/// A decoded-consumer context wired to the test schema.
pub fn decoded_context(db: &TestDb, client: Arc<dyn BasicConsumer>) -> DecodedConsumerContext {
    let data = ServerInitialize {
        args: ConsumerArgs {
            mode: "decoded".to_string(),
        },
        env: env(db),
    };
    let base_client =
        Arc::new(ContractInstance::build_client(ContractVersion::V2, &data).expect("client"));
    DecodedConsumerContext {
        client,
        base_client,
        pg_pool: db.pool.clone(),
        backend_schema: db.schema.clone(),
        contract_version: Arc::new(RwLock::new(ContractVersion::V2)),
        initial_contract_version: None,
    }
}
