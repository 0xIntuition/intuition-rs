use std::str::FromStr;

use crate::{
    account::{Account, AccountType},
    atom::{Atom, AtomResolvingStatus, AtomType},
    deposit::Deposit,
    event::{Event, EventType},
    fee_transfer::FeeTransfer,
    organization::Organization,
    person::Person,
    position::Position,
    predicate_object::PredicateObject,
    redemption::Redemption,
    signal::Signal,
    stats::Stats,
    stats_hour::StatsHour,
    thing::Thing,
    traits::SimpleCrud,
    triple::Triple,
    types::U256Wrapper,
    vault::Vault,
};
use rand::{
    distributions::{Alphanumeric, DistString},
    Rng,
};
use sqlx::{postgres::PgPoolOptions, PgPool};

pub const TEST_SCHEMA: &str = "base_sepolia_backend";

/// This function sets up a test database connection pool.
pub async fn setup_test_db() -> PgPool {
    let database_url = "postgres://testuser:test@localhost:5435/storage";
    PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
        .expect("Failed to create pool")
}

/// This function creates a random string.
pub fn create_random_string() -> String {
    Alphanumeric.sample_string(&mut rand::thread_rng(), 16)
}

/// This function creates a random number.
pub fn create_random_number() -> i32 {
    rand::thread_rng().gen_range(0..i32::MAX)
}

/// This function creates a random U256Wrapper.
pub fn create_random_u256wrapper() -> U256Wrapper {
    U256Wrapper::from_str(&create_random_number().to_string()).unwrap_or_default()
}

/// This function creates a test account.   
pub async fn create_test_account() -> Account {
    Account::builder()
        .id(create_random_string())
        .label(create_random_string())
        .image("https://example.com/image.jpg".to_string())
        .account_type(AccountType::Default)
        .build()
}

/// This function creates a test account and stores it in the database.
pub async fn create_test_account_db(pool: &PgPool) -> Account {
    let account = create_test_account().await;
    account
        .upsert(pool, TEST_SCHEMA)
        .await
        .expect("Failed to store account")
}

/// This function creates a test atom.
pub fn create_test_atom(wallet_id: String, creator_id: String) -> Atom {
    Atom::builder()
        .id(create_random_u256wrapper())
        .wallet_id(wallet_id)
        .creator_id(creator_id)
        .vault_id(create_random_u256wrapper())
        .data(create_random_string())
        .raw_data(create_random_string())
        .atom_type(AtomType::Thing)
        .emoji("🧪".to_string())
        .label(create_random_string())
        .block_number(create_random_u256wrapper())
        .block_timestamp(create_random_number())
        .transaction_hash(vec![5u8])
        .resolving_status(AtomResolvingStatus::Pending)
        .build()
}

/// This function creates a test triple.
pub fn create_test_triple(
    creator_id: String,
    subject_id: U256Wrapper,
    predicate_id: U256Wrapper,
    object_id: U256Wrapper,
) -> Triple {
    Triple::builder()
        .id(create_random_u256wrapper())
        .creator_id(creator_id)
        .subject_id(subject_id)
        .predicate_id(predicate_id)
        .object_id(object_id)
        .vault_id(create_random_u256wrapper())
        .counter_vault_id(create_random_u256wrapper())
        .block_number(create_random_u256wrapper())
        .block_timestamp(create_random_number())
        .transaction_hash(vec![5u8])
        .build()
}

/// This function creates a test vault with an atom_id.
pub fn create_test_vault_with_atom(atom_id: U256Wrapper) -> Vault {
    Vault::builder()
        .id(create_random_u256wrapper())
        .atom_id(atom_id)
        .total_shares(create_random_u256wrapper())
        .current_share_price(create_random_u256wrapper())
        .position_count(1)
        .build()
}

/// This function creates a test vault with a triple_id.
pub fn create_test_vault_with_triple(triple_id: U256Wrapper) -> Vault {
    Vault::builder()
        .id(create_random_u256wrapper())
        .triple_id(triple_id)
        .total_shares(create_random_u256wrapper())
        .current_share_price(create_random_u256wrapper())
        .position_count(1)
        .build()
}

/// This function creates a test vault and stores it in the database.
pub async fn create_test_vault_db(pool: &PgPool, vault: Vault) -> Vault {
    vault
        .upsert(pool, TEST_SCHEMA)
        .await
        .expect("Failed to store vault")
}

/// This function creates a test triple and stores it in the database.
pub async fn create_test_triple_db(pool: &PgPool, triple: Triple) -> Triple {
    triple
        .upsert(pool, TEST_SCHEMA)
        .await
        .expect("Failed to store triple")
}

/// This function creates a test atom and stores it in the database.
pub async fn create_test_atom_db(pool: &PgPool) -> Atom {
    // Step 1: Create test Accounts
    let stored_wallet = create_test_account_db(pool).await;
    let stored_creator = create_test_account_db(pool).await;

    // Step 3: Create a test Atom
    let test_atom = create_test_atom(stored_wallet.id, stored_creator.id);
    test_atom
        .upsert(pool, TEST_SCHEMA)
        .await
        .expect("Failed to store atom")
}

/// This function creates a test deposit.
pub fn create_test_deposit(
    sender_id: String,
    receiver_id: String,
    vault_id: U256Wrapper,
) -> Deposit {
    Deposit::builder()
        .id(create_random_string())
        .sender_id(sender_id)
        .receiver_id(receiver_id)
        .receiver_total_shares_in_vault(create_random_u256wrapper())
        .sender_assets_after_total_fees(create_random_u256wrapper())
        .shares_for_receiver(create_random_u256wrapper())
        .entry_fee(create_random_u256wrapper())
        .vault_id(vault_id)
        .is_triple(false)
        .is_atom_wallet(false)
        .block_number(create_random_u256wrapper())
        .block_timestamp(create_random_number())
        .transaction_hash(vec![5u8])
        .build()
}

/// This function creates a test deposit and stores it in the database.
pub async fn create_test_deposit_db(pool: &PgPool, deposit: Deposit) -> Deposit {
    deposit
        .upsert(pool, TEST_SCHEMA)
        .await
        .expect("Failed to store deposit")
}

/// This function creates a test event with an atom.
pub fn create_test_event_with_atom(atom_id: U256Wrapper) -> Event {
    Event::builder()
        .id(create_random_string())
        .atom_id(atom_id)
        .event_type(EventType::AtomCreated)
        .block_number(create_random_u256wrapper())
        .block_timestamp(create_random_number())
        .transaction_hash(vec![5u8])
        .build()
}

/// This function creates a test event with a triple.
pub fn create_test_event_with_triple(triple_id: U256Wrapper) -> Event {
    Event::builder()
        .id(create_random_string())
        .triple_id(triple_id)
        .event_type(EventType::TripleCreated)
        .block_number(create_random_u256wrapper())
        .block_timestamp(create_random_number())
        .transaction_hash(vec![5u8])
        .build()
}

/// This function creates a test event and stores it in the database.
pub async fn create_test_event_db(pool: &PgPool, event: Event) -> Event {
    event
        .upsert(pool, TEST_SCHEMA)
        .await
        .expect("Failed to store event")
}

/// This function creates a test fee transfer.
pub fn create_test_fee_transfer(sender_id: String, receiver_id: String) -> FeeTransfer {
    FeeTransfer::builder()
        .id(create_random_string())
        .sender_id(sender_id)
        .receiver_id(receiver_id)
        .amount(create_random_u256wrapper())
        .block_number(create_random_u256wrapper())
        .block_timestamp(create_random_number())
        .transaction_hash(vec![5u8])
        .build()
}

/// This function creates a test fee transfer and stores it in the database.
pub async fn create_test_fee_transfer_db(pool: &PgPool, fee_transfer: FeeTransfer) -> FeeTransfer {
    fee_transfer
        .upsert(pool, TEST_SCHEMA)
        .await
        .expect("Failed to store fee transfer")
}

/// This function creates a test redemption.
pub fn create_test_redemption(
    sender_id: String,
    receiver_id: String,
    vault_id: U256Wrapper,
) -> Redemption {
    Redemption::builder()
        .id(create_random_string())
        .sender_id(sender_id)
        .receiver_id(receiver_id)
        .sender_total_shares_in_vault(create_random_u256wrapper())
        .assets_for_receiver(create_random_u256wrapper())
        .shares_redeemed_by_sender(create_random_u256wrapper())
        .exit_fee(create_random_u256wrapper())
        .vault_id(vault_id)
        .block_number(create_random_u256wrapper())
        .block_timestamp(create_random_number())
        .transaction_hash(vec![5u8])
        .build()
}

/// This function creates a test redemption and stores it in the database.
pub async fn create_test_redemption_db(pool: &PgPool, redemption: Redemption) -> Redemption {
    redemption
        .upsert(pool, TEST_SCHEMA)
        .await
        .expect("Failed to store redemption")
}

/// This function creates a test organization.
pub fn create_test_organization() -> Organization {
    Organization::builder()
        .id(create_random_u256wrapper())
        .build()
}

/// This function creates a test organization and stores it in the database.
pub async fn create_test_organization_db(
    pool: &PgPool,
    organization: Organization,
) -> Organization {
    organization
        .upsert(pool, TEST_SCHEMA)
        .await
        .expect("Failed to store organization")
}

/// This function creates a test person.
pub fn create_test_person() -> Person {
    Person::builder().id(create_random_u256wrapper()).build()
}

/// This function creates a test person and stores it in the database.
pub async fn create_test_person_db(pool: &PgPool, person: Person) -> Person {
    person
        .upsert(pool, TEST_SCHEMA)
        .await
        .expect("Failed to store person")
}

/// This function creates a test position.
pub fn create_test_position(account_id: String, vault_id: U256Wrapper) -> Position {
    Position::builder()
        .id(create_random_string())
        .account_id(account_id)
        .vault_id(vault_id)
        .shares(create_random_u256wrapper())
        .build()
}

/// This function creates a test position and stores it in the database.
pub async fn create_test_position_db(pool: &PgPool, position: Position) -> Position {
    position
        .upsert(pool, TEST_SCHEMA)
        .await
        .expect("Failed to store position")
}

/// This function creates a test predicate object.
pub fn create_test_predicate_object(
    predicate_id: U256Wrapper,
    object_id: U256Wrapper,
) -> PredicateObject {
    PredicateObject::builder()
        .id(create_random_string())
        .predicate_id(predicate_id)
        .object_id(object_id)
        .build()
}

/// This function creates a test predicate object and stores it in the database.
pub async fn create_test_predicate_object_db(
    pool: &PgPool,
    predicate_object: PredicateObject,
) -> PredicateObject {
    predicate_object
        .upsert(pool, TEST_SCHEMA)
        .await
        .expect("Failed to store predicate object")
}

/// This function creates a test signal.
pub fn create_test_signal_with_atom_and_deposit(
    account_id: String,
    atom_id: U256Wrapper,
    deposit_id: String,
) -> Signal {
    Signal::builder()
        .id(create_random_string())
        .delta(create_random_u256wrapper())
        .account_id(account_id)
        .atom_id(atom_id)
        .block_number(create_random_u256wrapper())
        .block_timestamp(create_random_number())
        .deposit_id(deposit_id)
        .transaction_hash(vec![5u8])
        .build()
}

/// This function creates a test stats hour.
pub fn create_test_stats_hour() -> StatsHour {
    StatsHour::builder().id(create_random_number()).build()
}

/// This function creates a test stats.
pub fn create_test_stats() -> Stats {
    Stats::builder().id(create_random_number()).build()
}

/// This function creates a test thing.
pub fn create_test_thing() -> Thing {
    Thing::builder().id(create_random_u256wrapper()).build()
}
