use crate::{
    ConsumerError,
    EthMultiVault::Deposited,
    mode::{decoded_v1_5::utils::get_or_create_account, types::DecodedConsumerContext},
    schemas::types::DecodedMessage,
    traits::VaultManager,
};
use alloy::primitives::U256;
use async_trait::async_trait;
use futures::executor::block_on;
use models::{
    claim::Claim,
    deposit::Deposit,
    event::{Event, EventType},
    position::Position,
    predicate_object::PredicateObject,
    share_price_change::SharePriceChanged,
    signal::Signal,
    term::TermType,
    traits::SimpleCrud,
    triple::Triple,
    types::U256Wrapper,
    vault::Vault,
};
use std::str::FromStr;
use tracing::info;

use super::utils::get_or_create_vault;

#[async_trait]
impl VaultManager for &Deposited {
    fn term_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.vaultId))
    }

    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from_str("1")?)
    }

    async fn total_shares(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(SharePriceChanged::fetch_current_share_price(
            U256Wrapper::from(self.vaultId),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .total_shares)
    }

    async fn current_share_price(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(SharePriceChanged::fetch_current_share_price(
            U256Wrapper::from(self.vaultId),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .share_price)
    }

    async fn position_count(
        &self,
        _decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<i32, ConsumerError> {
        Ok(0)
    }
}

impl Deposited {
    /// This function creates a claim and predicate object
    async fn create_claim_and_predicate_object(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        triple: &Triple,
    ) -> Result<(), ConsumerError> {
        // Create claim
        Claim::builder()
            .id(self.format_claim_id())
            .account_id(self.receiver.to_string())
            .triple_id(triple.term_id.clone())
            .subject_id(triple.subject_id.clone())
            .predicate_id(triple.predicate_id.clone())
            .object_id(triple.object_id.clone())
            .term_id(triple.term_id.clone())
            .counter_term_id(triple.counter_term_id.clone())
            .curve_id(U256Wrapper::from_str("1")?)
            .counter_curve_id(U256Wrapper::from_str("1")?)
            .shares(if U256Wrapper::from(self.vaultId) == triple.term_id {
                self.receiverTotalSharesInVault
            } else {
                U256::from(0)
            })
            .counter_shares(
                if U256Wrapper::from(self.vaultId) == triple.counter_term_id {
                    self.receiverTotalSharesInVault
                } else {
                    U256::from(0)
                },
            )
            .build()
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await?;

        // Update or create predicate object
        let predicate_object_id = format!("{}-{}", triple.predicate_id, triple.object_id);
        match PredicateObject::find_by_id(
            predicate_object_id,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        {
            Some(mut po) => {
                po.claim_count += 1;
                po.upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;
            }
            None => {
                PredicateObject::builder()
                    .id(format!("{}-{}", triple.predicate_id, triple.object_id))
                    .predicate_id(triple.predicate_id.clone())
                    .object_id(triple.object_id.clone())
                    .claim_count(1)
                    .triple_count(1)
                    .build()
                    .upsert(
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.backend_schema,
                    )
                    .await?;
            }
        };

        Ok(())
    }

    /// This function creates a deposit
    async fn create_deposit(
        &self,
        event: &DecodedMessage,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<Deposit, ConsumerError> {
        Deposit::builder()
            .id(DecodedMessage::event_id(event))
            .sender_id(self.sender.to_string())
            .receiver_id(self.receiver.to_string())
            .receiver_total_shares_in_vault(U256Wrapper::from(self.receiverTotalSharesInVault))
            .sender_assets_after_total_fees(U256Wrapper::from(self.senderAssetsAfterTotalFees))
            .shares_for_receiver(U256Wrapper::from(self.sharesForReceiver))
            .entry_fee(U256Wrapper::from(self.entryFee))
            .term_id(U256Wrapper::from(self.vaultId))
            .curve_id(U256Wrapper::from_str("1")?)
            .is_triple(self.isTriple)
            .is_atom_wallet(self.isAtomWallet)
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
            .build()
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function creates an `Event` for the `Deposited` event
    async fn create_event(
        &self,
        event: &DecodedMessage,
        decoded_consumer_context: &DecodedConsumerContext,
        deposit_id: String,
    ) -> Result<Event, ConsumerError> {
        // Create the event
        let event = if self.isTriple {
            Event::builder()
                .id(DecodedMessage::event_id(event))
                .event_type(EventType::Deposited)
                .deposit_id(deposit_id)
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .triple_id(U256Wrapper::from(self.vaultId))
                .build()
        } else {
            Event::builder()
                .id(DecodedMessage::event_id(event))
                .event_type(EventType::Deposited)
                .deposit_id(deposit_id)
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .atom_id(U256Wrapper::from(self.vaultId))
                .build()
        };

        event
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function creates a new position
    async fn create_new_position(
        &self,
        position_id: String,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<Position, ConsumerError> {
        Position::builder()
            .id(position_id.clone())
            .account_id(self.receiver.to_string())
            .term_id(U256Wrapper::from(self.vaultId))
            .curve_id(U256Wrapper::from_str("1")?)
            .shares(self.receiverTotalSharesInVault)
            .build()
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function creates a `Signal` for the `Deposited` event
    async fn create_signal(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        vault: &Vault,
    ) -> Result<(), ConsumerError> {
        if self.senderAssetsAfterTotalFees > U256::from(0) {
            if !self.isTriple {
                Signal::builder()
                    .id(DecodedMessage::event_id(event))
                    .account_id(self.sender.to_string().to_lowercase())
                    .delta(U256Wrapper::from(self.senderAssetsAfterTotalFees))
                    .atom_id(vault.term_id.clone())
                    .deposit_id(DecodedMessage::event_id(event))
                    .block_number(U256Wrapper::try_from(event.block_number)?)
                    .block_timestamp(event.block_timestamp)
                    .transaction_hash(event.transaction_hash.clone())
                    .build()
                    .upsert(
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.backend_schema,
                    )
                    .await?;
            } else {
                Signal::builder()
                    .id(DecodedMessage::event_id(event))
                    .account_id(self.sender.to_string().to_lowercase())
                    .delta(U256Wrapper::from(self.senderAssetsAfterTotalFees))
                    .triple_id(vault.term_id.clone())
                    .deposit_id(DecodedMessage::event_id(event))
                    .block_number(U256Wrapper::try_from(event.block_number)?)
                    .block_timestamp(event.block_timestamp)
                    .transaction_hash(event.transaction_hash.clone())
                    .build()
                    .upsert(
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.backend_schema,
                    )
                    .await?;
            }
        } else {
            info!("Sender assets after total fees is 0, nothing to do.");
        }
        Ok(())
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
    pub fn format_position_id(&self) -> String {
        format!(
            "{}-1-{}",
            self.vaultId,
            self.receiver.to_string().to_lowercase()
        )
    }

    /// This function handles the creation of a `Deposit`
    pub async fn handle_deposit_creation(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        // Initialize accounts and vault. We need to block on this because it's async and
        // we need to ensure that the accounts and vault are initialized before we proceed
        let vault = block_on(self.initialize_accounts_and_vault(decoded_consumer_context))?;

        // Create deposit record
        let deposit = self.create_deposit(event, decoded_consumer_context).await?;

        // Handle position and related entities
        self.handle_position_and_claims(decoded_consumer_context, &vault)
            .await?;

        // Create event
        self.create_event(event, decoded_consumer_context, deposit.id)
            .await?;

        // Create signal
        self.create_signal(decoded_consumer_context, event, &vault)
            .await?;

        Ok(())
    }

    /// This function handles an existing position
    async fn handle_existing_position(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        position_id: &str,
        triple: Option<Triple>,
        vault: &Vault,
    ) -> Result<(), ConsumerError> {
        // Update or create position
        self.update_position(decoded_consumer_context, position_id)
            .await?;

        // Handle triple-related updates if present
        if let Some(triple) = triple {
            self.update_claim(decoded_consumer_context, &triple, vault)
                .await?;
        }

        Ok(())
    }

    /// This function handles the creation of a new position
    async fn handle_new_position(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        position_id: &str,
        triple: Option<Triple>,
    ) -> Result<(), ConsumerError> {
        self.create_new_position(position_id.to_string(), decoded_consumer_context)
            .await?;

        if let Some(triple) = triple {
            self.create_claim_and_predicate_object(decoded_consumer_context, &triple)
                .await?;
        }

        Ok(())
    }

    /// This function handles the position and claims
    async fn handle_position_and_claims(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        vault: &Vault,
    ) -> Result<(), ConsumerError> {
        let position_id = self.format_position_id();
        let triple = Triple::find_by_id(
            U256Wrapper::from(self.vaultId),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;
        let position = Position::find_by_id(
            position_id.clone(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        if position.is_none() && self.receiverTotalSharesInVault > U256::from(0) {
            self.handle_new_position(decoded_consumer_context, &position_id, triple)
                .await?;
        } else if position.is_some() && self.receiverTotalSharesInVault > U256::from(0) {
            self.handle_existing_position(decoded_consumer_context, &position_id, triple, vault)
                .await?;
        } else {
            info!("No need to update position or claims.");
        }
        Ok(())
    }

    /// This function initializes the accounts and vault
    async fn initialize_accounts_and_vault(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<Vault, ConsumerError> {
        // Create accounts concurrently
        let (sender, receiver) = futures::join!(
            get_or_create_account(self.sender.to_string(), decoded_consumer_context),
            get_or_create_account(self.receiver.to_string(), decoded_consumer_context)
        );
        sender?;
        receiver?;

        get_or_create_vault(
            self,
            decoded_consumer_context,
            if self.isTriple {
                TermType::Triple
            } else {
                TermType::Atom
            },
        )
        .await
    }

    /// This function updates the claim
    async fn update_claim(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        triple: &Triple,
        vault: &Vault,
    ) -> Result<Claim, ConsumerError> {
        let claim_id = format!(
            "{}-{}",
            triple.term_id,
            self.receiver.to_string().to_lowercase()
        );

        let claim = match Claim::find_by_id(
            claim_id.clone(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        {
            Some(mut claim) => {
                if vault.term_id == triple.term_id {
                    claim.shares = U256Wrapper::from(self.sharesForReceiver);
                } else {
                    claim.counter_shares = U256Wrapper::from(self.sharesForReceiver);
                }
                claim
            }
            None => Claim::builder()
                .id(claim_id)
                .account_id(self.receiver.to_string())
                .triple_id(triple.term_id.clone())
                .subject_id(triple.subject_id.clone())
                .predicate_id(triple.predicate_id.clone())
                .object_id(triple.object_id.clone())
                .term_id(triple.term_id.clone())
                .counter_term_id(triple.counter_term_id.clone())
                .curve_id(U256Wrapper::from_str("1")?)
                .counter_curve_id(U256Wrapper::from_str("1")?)
                .shares(U256Wrapper::from(self.sharesForReceiver))
                .counter_shares(U256Wrapper::from(self.sharesForReceiver))
                .build(),
        };

        claim
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function updates the position
    async fn update_position(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        position_id: &str,
    ) -> Result<Position, ConsumerError> {
        let position = match Position::find_by_id(
            position_id.to_string(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        {
            Some(mut position) => {
                position.shares = U256Wrapper::from(self.receiverTotalSharesInVault);
                position
            }
            None => return Err(ConsumerError::PositionNotFound),
        };

        position
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await
            .map_err(ConsumerError::ModelError)
    }
}
