use super::utils::get_absolute_triple_id;
use crate::{
    mode::{decoded::utils::get_or_create_account, types::DecodedConsumerContext},
    schemas::types::DecodedMessage,
    ConsumerError,
    EthMultiVault::{Deposited, EthMultiVaultInstance},
};
use alloy::{eips::BlockId, primitives::U256, providers::RootProvider, transports::http::Http};
use futures::executor::block_on;
use models::{
    claim::Claim,
    deposit::Deposit,
    event::{Event, EventType},
    position::Position,
    predicate_object::PredicateObject,
    signal::Signal,
    traits::SimpleCrud,
    triple::Triple,
    types::U256Wrapper,
    vault::Vault,
};
use reqwest::Client;
use sqlx::PgPool;
use std::str::FromStr;
use tracing::info;
impl Deposited {
    /// This function creates a claim and predicate object
    async fn create_claim_and_predicate_object(
        &self,
        pg_pool: &PgPool,
        triple: &Triple,
    ) -> Result<(), ConsumerError> {
        // Create claim
        Claim::builder()
            .id(self.format_claim_id())
            .account_id(self.receiver.to_string())
            .triple_id(triple.id.clone())
            .subject_id(triple.subject_id.clone())
            .predicate_id(triple.predicate_id.clone())
            .object_id(triple.object_id.clone())
            .vault_id(triple.vault_id.clone())
            .counter_vault_id(triple.counter_vault_id.clone())
            .shares(self.sharesForReceiver)
            .counter_shares(self.sharesForReceiver)
            .build()
            .upsert(pg_pool)
            .await?;

        // Update or create predicate object
        let predicate_object_id = format!("{}-{}", triple.predicate_id, triple.object_id);
        match PredicateObject::find_by_id(predicate_object_id, pg_pool).await? {
            Some(mut po) => {
                po.claim_count += 1;
                po.upsert(pg_pool).await?
            }
            None => {
                PredicateObject::builder()
                    .id(format!("{}-{}", triple.predicate_id, triple.object_id))
                    .predicate_id(triple.predicate_id.clone())
                    .object_id(triple.object_id.clone())
                    .claim_count(1)
                    .triple_count(1)
                    .build()
                    .upsert(pg_pool)
                    .await?
            }
        };

        Ok(())
    }

    /// This function creates a deposit
    async fn create_deposit(
        &self,
        event: &DecodedMessage,
        pg_pool: &PgPool,
    ) -> Result<Deposit, ConsumerError> {
        Deposit::builder()
            .id(event.transaction_hash.to_string())
            .sender_id(self.sender.to_string())
            .receiver_id(self.receiver.to_string())
            .receiver_total_shares_in_vault(U256Wrapper::from(self.receiverTotalSharesInVault))
            .sender_assets_after_total_fees(U256Wrapper::from(self.senderAssetsAfterTotalFees))
            .shares_for_receiver(U256Wrapper::from(self.sharesForReceiver))
            .entry_fee(U256Wrapper::from(self.entryFee))
            .vault_id(self.vaultId)
            .is_triple(self.isTriple)
            .is_atom_wallet(self.isAtomWallet)
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
            .build()
            .upsert(pg_pool)
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function creates an `Event` for the `Deposited` event
    async fn create_event(
        &self,
        event: &DecodedMessage,
        pg_pool: &PgPool,
    ) -> Result<Event, ConsumerError> {
        // Create the event
        let event = if self.isTriple {
            Event::builder()
                .id(DecodedMessage::event_id(event))
                .event_type(EventType::Deposited)
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .triple_id(U256Wrapper::from(self.vaultId))
                .build()
        } else {
            Event::builder()
                .id(DecodedMessage::event_id(event))
                .event_type(EventType::Deposited)
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .atom_id(U256Wrapper::from(self.vaultId))
                .build()
        };

        event
            .upsert(pg_pool)
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function creates a new position
    async fn create_new_position(
        &self,
        position_id: String,
        pg_pool: &PgPool,
    ) -> Result<Position, ConsumerError> {
        Position::builder()
            .id(position_id.clone())
            .account_id(self.receiver.to_string())
            .vault_id(self.vaultId)
            .shares(self.receiverTotalSharesInVault)
            .build()
            .upsert(pg_pool)
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function creates a `Signal` for the `Deposited` event
    async fn create_signal(
        &self,
        pg_pool: &PgPool,
        event: &DecodedMessage,
        vault: &Vault,
    ) -> Result<(), ConsumerError> {
        if self.senderAssetsAfterTotalFees > U256::from(0) {
            if let Some(atom_id) = vault.atom_id.clone() {
                Signal::builder()
                    .id(DecodedMessage::event_id(event))
                    .account_id(self.sender.to_string().to_lowercase())
                    .delta(U256Wrapper::from(self.senderAssetsAfterTotalFees))
                    .atom_id(atom_id)
                    .deposit_id(event.transaction_hash.clone())
                    .block_number(U256Wrapper::try_from(event.block_number)?)
                    .block_timestamp(event.block_timestamp)
                    .transaction_hash(event.transaction_hash.clone())
                    .build()
                    .upsert(pg_pool)
                    .await?;
            } else {
                Signal::builder()
                    .id(DecodedMessage::event_id(event))
                    .account_id(self.sender.to_string().to_lowercase())
                    .delta(U256Wrapper::from(self.senderAssetsAfterTotalFees))
                    .triple_id(
                        vault
                            .triple_id
                            .clone()
                            .ok_or(ConsumerError::TripleNotFound)?,
                    )
                    .deposit_id(event.transaction_hash.clone())
                    .block_number(U256Wrapper::try_from(event.block_number)?)
                    .block_timestamp(event.block_timestamp)
                    .transaction_hash(event.transaction_hash.clone())
                    .build()
                    .upsert(pg_pool)
                    .await?;
            }
        } else {
            info!("Sender assets after total fees is 0, nothing to do.");
        }
        Ok(())
    }

    /// This function fetches the current share price from the vault
    async fn fetch_current_share_price(
        &self,
        web3: &EthMultiVaultInstance<Http<Client>, RootProvider<Http<Client>>>,
        event: &DecodedMessage,
    ) -> Result<U256, ConsumerError> {
        Ok(web3
            .currentSharePrice(self.vaultId)
            .block(BlockId::from_str(&event.block_number.to_string())?)
            .call()
            .await?
            ._0)
    }

    /// This function formats the claim ID
    fn format_claim_id(&self) -> String {
        format!(
            "{}-{}",
            self.vaultId,
            self.receiver.to_string().to_lowercase()
        )
    }

    /// This function formats the position ID
    fn format_position_id(&self) -> String {
        format!(
            "{}-{}",
            self.vaultId,
            self.receiver.to_string().to_lowercase()
        )
    }

    /// This function gets or creates a vault
    async fn get_or_create_vault(
        &self,
        pg_pool: &PgPool,
        id: U256,
        current_share_price: U256,
    ) -> Result<Vault, ConsumerError> {
        match Vault::find_by_id(U256Wrapper::from_str(&id.to_string())?, pg_pool).await? {
            Some(mut vault) => {
                vault.current_share_price = U256Wrapper::from(current_share_price);
                vault.total_shares = vault.total_shares + U256Wrapper::from(self.sharesForReceiver);
                vault
                    .upsert(pg_pool)
                    .await
                    .map_err(ConsumerError::ModelError)
            }
            None => {
                if self.isTriple {
                    Vault::builder()
                        .id(id)
                        .current_share_price(U256Wrapper::from(current_share_price))
                        .position_count(0)
                        .triple_id(get_absolute_triple_id(self.vaultId))
                        .total_shares(U256Wrapper::from(self.sharesForReceiver))
                        .build()
                        .upsert(pg_pool)
                        .await
                        .map_err(ConsumerError::ModelError)
                } else {
                    Vault::builder()
                        .id(id)
                        .current_share_price(U256Wrapper::from(current_share_price))
                        .position_count(0)
                        .atom_id(self.vaultId)
                        .total_shares(U256Wrapper::from(U256::from(0)))
                        .build()
                        .upsert(pg_pool)
                        .await
                        .map_err(ConsumerError::ModelError)
                }
            }
        }
    }

    /// This function handles the creation of a `Deposit`
    pub async fn handle_deposit_creation(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        // Initialize core data
        let current_share_price = self
            .fetch_current_share_price(&decoded_consumer_context.base_client, event)
            .await?;

        // Initialize accounts and vault. We need to block on this because it's async and
        // we need to ensure that the accounts and vault are initialized before we proceed
        let vault = block_on(
            self.initialize_accounts_and_vault(decoded_consumer_context, current_share_price),
        )?;

        // Create deposit record
        self.create_deposit(event, &decoded_consumer_context.pg_pool)
            .await?;

        // Handle position and related entities
        self.handle_position_and_claims(&decoded_consumer_context.pg_pool, &vault)
            .await?;

        // Create event
        self.create_event(event, &decoded_consumer_context.pg_pool)
            .await?;

        // Create signal
        self.create_signal(&decoded_consumer_context.pg_pool, event, &vault)
            .await?;

        Ok(())
    }

    /// This function handles an existing position
    async fn handle_existing_position(
        &self,
        pg_pool: &PgPool,
        position_id: &str,
        triple: Option<Triple>,
        vault: &Vault,
    ) -> Result<(), ConsumerError> {
        // Update or create position
        self.update_position(pg_pool, position_id).await?;

        // Handle triple-related updates if present
        if let Some(triple) = triple {
            self.update_claim(pg_pool, &triple, vault).await?;
        }

        Ok(())
    }

    /// This function handles the creation of a new position
    async fn handle_new_position(
        &self,
        pg_pool: &PgPool,
        position_id: &str,
        triple: Option<Triple>,
    ) -> Result<(), ConsumerError> {
        self.create_new_position(position_id.to_string(), pg_pool)
            .await?;
        self.increment_vault_position_count(pg_pool).await?;

        if let Some(triple) = triple {
            self.create_claim_and_predicate_object(pg_pool, &triple)
                .await?;
        }

        Ok(())
    }

    /// This function handles the position and claims
    async fn handle_position_and_claims(
        &self,
        pg_pool: &PgPool,
        vault: &Vault,
    ) -> Result<(), ConsumerError> {
        let position_id = self.format_position_id();
        let triple = Triple::find_by_id(U256Wrapper::from(self.vaultId), pg_pool).await?;
        let position = Position::find_by_id(position_id.clone(), pg_pool).await?;

        if position.is_none() && self.receiverTotalSharesInVault != U256::from(0) {
            self.handle_new_position(pg_pool, &position_id, triple)
                .await?;
        } else if self.receiverTotalSharesInVault != U256::from(0) {
            self.handle_existing_position(pg_pool, &position_id, triple, vault)
                .await?;
        }

        Ok(())
    }

    /// This function increments the vault's position count
    async fn increment_vault_position_count(&self, pg_pool: &PgPool) -> Result<(), ConsumerError> {
        let mut vault = Vault::find_by_id(U256Wrapper::from(self.vaultId), pg_pool)
            .await?
            .ok_or(ConsumerError::VaultNotFound)?;

        vault.position_count += 1;
        vault
            .upsert(pg_pool)
            .await
            .map_err(ConsumerError::ModelError)?;
        Ok(())
    }

    /// This function initializes the accounts and vault
    async fn initialize_accounts_and_vault(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        current_share_price: U256,
    ) -> Result<Vault, ConsumerError> {
        // Create accounts concurrently
        let (sender, receiver) = futures::join!(
            get_or_create_account(self.sender.to_string(), decoded_consumer_context),
            get_or_create_account(self.receiver.to_string(), decoded_consumer_context)
        );
        sender?;
        receiver?;

        self.get_or_create_vault(
            &decoded_consumer_context.pg_pool,
            self.vaultId,
            current_share_price,
        )
        .await
    }

    /// This function updates the claim
    async fn update_claim(
        &self,
        pg_pool: &PgPool,
        triple: &Triple,
        vault: &Vault,
    ) -> Result<Claim, ConsumerError> {
        let claim_id = format!("{}-{}", triple.id, self.receiver.to_string().to_lowercase());

        let claim = match Claim::find_by_id(claim_id.clone(), pg_pool).await? {
            Some(mut claim) => {
                if vault.id == triple.vault_id {
                    claim.shares = U256Wrapper::from(self.sharesForReceiver);
                } else {
                    claim.counter_shares = U256Wrapper::from(self.sharesForReceiver);
                }
                claim
            }
            None => Claim::builder()
                .id(claim_id)
                .account_id(self.receiver.to_string())
                .triple_id(triple.id.clone())
                .subject_id(triple.subject_id.clone())
                .predicate_id(triple.predicate_id.clone())
                .object_id(triple.object_id.clone())
                .vault_id(triple.vault_id.clone())
                .counter_vault_id(triple.counter_vault_id.clone())
                .shares(U256Wrapper::from(self.sharesForReceiver))
                .counter_shares(U256Wrapper::from(self.sharesForReceiver))
                .build(),
        };

        claim
            .upsert(pg_pool)
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function updates the position
    async fn update_position(
        &self,
        pg_pool: &PgPool,
        position_id: &str,
    ) -> Result<Position, ConsumerError> {
        let position = match Position::find_by_id(position_id.to_string(), pg_pool).await? {
            Some(mut pos) => {
                pos.shares = U256Wrapper::from(self.receiverTotalSharesInVault);
                pos
            }
            None => Position::builder()
                .id(position_id.to_string())
                .account_id(self.receiver.to_string())
                .vault_id(self.vaultId)
                .shares(self.receiverTotalSharesInVault)
                .build(),
        };

        position
            .upsert(pg_pool)
            .await
            .map_err(ConsumerError::ModelError)
    }
}
