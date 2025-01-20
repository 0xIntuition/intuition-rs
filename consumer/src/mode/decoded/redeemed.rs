use super::utils::get_or_create_account;
use crate::{
    error::ConsumerError,
    mode::types::DecodedConsumerContext,
    schemas::types::DecodedMessage,
    EthMultiVault::{EthMultiVaultInstance, Redeemed},
};
use alloy::{
    eips::BlockId,
    primitives::{Uint, U256},
    providers::RootProvider,
    transports::http::Http,
};
use models::{
    account::Account,
    claim::Claim,
    event::{Event, EventType},
    position::Position,
    predicate_object::PredicateObject,
    redemption::Redemption,
    signal::Signal,
    traits::{Deletable, SimpleCrud},
    triple::Triple,
    types::U256Wrapper,
    vault::Vault,
};
use reqwest::Client;
use std::str::FromStr;

impl Redeemed {
    /// This function creates an `Event` for the `Redeemed` event
    async fn create_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        vault: &Vault,
    ) -> Result<(), ConsumerError> {
        if let Some(triple_id) = vault.triple_id.clone() {
            Event::builder()
                .id(DecodedMessage::event_id(event))
                .event_type(EventType::Redeemed)
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .redemption_id(DecodedMessage::event_id(event))
                .triple_id(triple_id)
                .build()
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;
        } else {
            Event::builder()
                .id(DecodedMessage::event_id(event))
                .event_type(EventType::Redeemed)
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .redemption_id(DecodedMessage::event_id(event))
                .atom_id(
                    vault
                        .atom_id
                        .clone()
                        .ok_or(ConsumerError::VaultAtomNotFound)?,
                )
                .build()
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;
        }
        Ok(())
    }

    // Helper methods to break down the complexity:
    async fn create_redemption_record(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        sender_account: &Account,
        receiver_account: &Account,
        event: &DecodedMessage,
    ) -> Result<Redemption, ConsumerError> {
        Redemption::builder()
            .id(DecodedMessage::event_id(event))
            .sender_id(sender_account.id.clone())
            .receiver_id(receiver_account.id.clone())
            .sender_total_shares_in_vault(self.senderTotalSharesInVault)
            .assets_for_receiver(self.assetsForReceiver)
            .shares_redeemed_by_sender(self.sharesRedeemedBySender)
            .exit_fee(self.exitFee)
            .vault_id(U256Wrapper::from(self.vaultId))
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

    /// This function creates a `Signal` for the `Redeemed` event
    async fn create_signal(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        vault: &Vault,
    ) -> Result<(), ConsumerError> {
        if let Some(triple_id) = vault.triple_id.clone() {
            Signal::builder()
                .id(DecodedMessage::event_id(event))
                .account_id(self.sender.to_string().to_lowercase())
                // This is the equivalent of multiplying the assets for receiver by -1
                .delta(U256Wrapper::from(
                    U256::ZERO.saturating_sub(self.assetsForReceiver),
                ))
                .triple_id(triple_id)
                .redemption_id(DecodedMessage::event_id(event))
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
                // This is the equivalent of multiplying the assets for receiver by -1
                .delta(U256Wrapper::from(
                    U256::ZERO.saturating_sub(self.assetsForReceiver),
                ))
                .atom_id(
                    vault
                        .atom_id
                        .clone()
                        .ok_or(ConsumerError::VaultAtomNotFound)?,
                )
                .redemption_id(DecodedMessage::event_id(event))
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

    /// This function handles the creation of a `Redeemed`
    pub async fn handle_redeemed_creation(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        // 1. Set up accounts
        let sender_account =
            get_or_create_account(self.sender.to_string(), decoded_consumer_context).await?;
        let receiver_account =
            get_or_create_account(self.receiver.to_string(), decoded_consumer_context).await?;

        // 2. Create redemption record
        self.create_redemption_record(
            decoded_consumer_context,
            &sender_account,
            &receiver_account,
            event,
        )
        .await?;

        // 3. Get vault and current share price
        let current_share_price = self
            .fetch_current_share_price(&decoded_consumer_context.base_client, event)
            .await?;
        let vault = Vault::find_by_id(
            U256Wrapper::from(self.vaultId),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .ok_or(ConsumerError::VaultNotFound)?;

        // 4. Handle position and claims based on remaining shares
        if self.senderTotalSharesInVault == Uint::from(0) {
            self.handle_zero_shares(&vault, &sender_account, decoded_consumer_context)
                .await?;
        } else {
            self.handle_remaining_shares(&vault, &sender_account, decoded_consumer_context)
                .await?;
        }

        // 5. Update vault stats
        self.update_vault_stats(decoded_consumer_context, current_share_price)
            .await?;

        // 6. Create event record
        self.create_event(decoded_consumer_context, event, &vault)
            .await?;

        // 7. Create signal record
        self.create_signal(decoded_consumer_context, event, &vault)
            .await?;

        Ok(())
    }

    /// This function handles the remaining shares
    async fn handle_remaining_shares(
        &self,
        vault: &Vault,
        sender_account: &Account,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        // Update position
        if let Some(mut position) = Position::find_by_id(
            format!("{}-{}", vault.id, sender_account.id.to_lowercase()),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        {
            position.shares = U256Wrapper::from(self.senderTotalSharesInVault);
            position
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;
        }

        // Update claim if triple exists
        if let Some(triple_id) = &vault.triple_id {
            if let Some(triple) = Triple::find_by_id(
                triple_id.clone(),
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await?
            {
                if let Some(mut claim) = Claim::find_by_id(
                    format!("{}-{}", triple.id, sender_account.id.to_lowercase()),
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?
                {
                    claim.shares = if vault.id == triple.vault_id {
                        U256Wrapper::from(self.senderTotalSharesInVault)
                    } else {
                        claim.shares
                    };
                    claim.counter_shares = if vault.id == triple.counter_vault_id {
                        U256Wrapper::from(self.senderTotalSharesInVault)
                    } else {
                        claim.counter_shares
                    };
                    claim
                        .upsert(
                            &decoded_consumer_context.pg_pool,
                            &decoded_consumer_context.backend_schema,
                        )
                        .await?;
                }
            }
        }
        Ok(())
    }

    /// This function handles the deletion of a position and related claims if the sender has no shares left
    async fn handle_zero_shares(
        &self,
        vault: &Vault,
        sender_account: &Account,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        // Delete position
        let position_id = format!("{}-{}", vault.id, sender_account.id.to_lowercase());
        Position::delete(
            position_id,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await
        .map_err(|e| ConsumerError::DeletePosition(e.to_string()))?;

        // Handle triple-related cleanup if exists
        if let Some(triple_id) = &vault.triple_id {
            if let Some(triple) = Triple::find_by_id(
                triple_id.clone(),
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await?
            {
                // Delete claim
                let claim_id = format!("{}-{}", triple.id, sender_account.id.to_lowercase());
                Claim::delete(
                    claim_id,
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await
                .map_err(|e| ConsumerError::DeleteClaim(e.to_string()))?;

                // Update predicate object
                if let Some(mut predicate_object) = PredicateObject::find_by_id(
                    format!("{}-{}", triple.predicate_id, triple.object_id),
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?
                {
                    predicate_object.claim_count -= 1;
                    predicate_object
                        .upsert(
                            &decoded_consumer_context.pg_pool,
                            &decoded_consumer_context.backend_schema,
                        )
                        .await?;
                }
            }
        }
        Ok(())
    }

    /// This function updates the vault stats
    async fn update_vault_stats(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        current_share_price: U256,
    ) -> Result<(), ConsumerError> {
        if let Some(mut vault) = Vault::find_by_id(
            U256Wrapper::from(self.vaultId),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        {
            vault.total_shares -= U256Wrapper::from(self.sharesRedeemedBySender);
            vault.current_share_price = U256Wrapper::from(current_share_price);
            vault.position_count -= 1;
            vault
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;
            Ok(())
        } else {
            Err(ConsumerError::VaultNotFound)
        }
    }
}
