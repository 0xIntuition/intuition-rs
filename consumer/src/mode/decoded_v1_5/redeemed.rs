use crate::{
    EthMultiVaultV1_5::Redeemed,
    error::ConsumerError,
    mode::{types::DecodedConsumerContext, utils::get_or_create_account},
    schemas::types::DecodedMessage,
};
use alloy::primitives::{U256, Uint};
use models::{
    account::Account,
    event::{Event, EventType},
    position::Position,
    predicate_object::PredicateObject,
    redemption::Redemption,
    signal::Signal,
    term::{Term, TermType},
    traits::{Deletable, SimpleCrud},
    triple::Triple,
    types::U256Wrapper,
    vault::Vault,
};
use sqlx::{Postgres, Transaction};
use std::str::FromStr;
use tracing::info;

impl Redeemed {
    /// This function creates an `Event` for the `Redeemed` event
    async fn create_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        vault: &Vault,
    ) -> Result<(), ConsumerError> {
        let term_type = Term::find_by_id(
            vault.term_id.clone(),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        .ok_or(ConsumerError::TermNotFound)?;

        let event_obj = if let TermType::Triple = term_type.term_type {
            Event::builder()
                .id(DecodedMessage::event_id(event))
                .event_type(EventType::Redeemed)
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .redemption_id(DecodedMessage::event_id(event))
                .triple_id(vault.term_id.clone())
                .build()
        } else {
            Event::builder()
                .id(DecodedMessage::event_id(event))
                .event_type(EventType::Redeemed)
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .redemption_id(DecodedMessage::event_id(event))
                .atom_id(vault.term_id.clone())
                .build()
        };
        event_obj
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await?;
        Ok(())
    }

    // Helper methods to break down the complexity:
    async fn create_redemption_record(
        &self,
        backend_schema: &str,
        sender_account: &Account,
        receiver_account: &Account,
        event: &DecodedMessage,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Redemption, ConsumerError> {
        Redemption::builder()
            .id(DecodedMessage::event_id(event))
            .sender_id(sender_account.id.clone())
            .receiver_id(receiver_account.id.clone())
            .sender_total_shares_in_vault(self.senderTotalSharesInVault)
            .assets_for_receiver(self.assetsForReceiver)
            .shares_redeemed_by_sender(self.sharesRedeemedBySender)
            .exit_fee(self.exitFee)
            .term_id(U256Wrapper::from(self.vaultId))
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
            .curve_id(U256Wrapper::from_str("1")?)
            .build()
            .upsert(backend_schema, tx.as_mut())
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
        let term_type = Term::find_by_id(
            vault.term_id.clone(),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        .ok_or(ConsumerError::TermNotFound)?;

        let signal = if let TermType::Triple = term_type.term_type {
            Signal::builder()
                .id(DecodedMessage::event_id(event))
                .account_id(self.sender.to_string().to_lowercase())
                // This is the equivalent of multiplying the assets for receiver by -1
                .delta(U256Wrapper::from(
                    U256::ZERO.saturating_sub(self.assetsForReceiver),
                ))
                .triple_id(vault.term_id.clone())
                .redemption_id(DecodedMessage::event_id(event))
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .term_id(vault.term_id.clone())
                .curve_id(U256Wrapper::from_str("1")?)
                .build()
        } else {
            Signal::builder()
                .id(DecodedMessage::event_id(event))
                .account_id(self.sender.to_string().to_lowercase())
                // This is the equivalent of multiplying the assets for receiver by -1
                .delta(U256Wrapper::from(
                    U256::ZERO.saturating_sub(self.assetsForReceiver),
                ))
                .atom_id(vault.term_id.clone())
                .redemption_id(DecodedMessage::event_id(event))
                .block_number(U256Wrapper::try_from(event.block_number)?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .term_id(vault.term_id.clone())
                .curve_id(U256Wrapper::from_str("1")?)
                .build()
        };
        signal
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await?;
        Ok(())
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
        // 2. Ensure the vault exists
        let vault = Vault::find_by_term_id_and_curve_id(
            U256Wrapper::from(self.vaultId),
            U256Wrapper::from_str("1")?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .ok_or(ConsumerError::VaultNotFound)?;

        let mut tx = decoded_consumer_context.pg_pool.begin().await?;

        // 3. Create redemption record
        self.create_redemption_record(
            &decoded_consumer_context.backend_schema,
            &sender_account,
            &receiver_account,
            event,
            &mut tx,
        )
        .await?;

        // When the redemption fully depletes the sender's shares:
        if self.senderTotalSharesInVault == Uint::from(0) {
            // Build the position ID
            let position_id = format!("{}-1-{}", vault.term_id, sender_account.id.to_lowercase());
            // Call the handler to remove the position
            self.handle_position_redemption(
                &decoded_consumer_context.backend_schema,
                &position_id,
                &mut tx,
            )
            .await?;
            // Cleanup the triple related records
            self.handle_triple_cleanup(&vault, &decoded_consumer_context.backend_schema, &mut tx)
                .await?;
        } else {
            self.handle_remaining_shares(
                &vault,
                &sender_account,
                &decoded_consumer_context.backend_schema,
                &mut tx,
            )
            .await?;
        }

        tx.commit().await?;

        // 4. Create event and signal records
        self.create_event(decoded_consumer_context, event, &vault)
            .await?;

        self.create_signal(decoded_consumer_context, event, &vault)
            .await?;

        Ok(())
    }

    /// This function handles the remaining shares
    async fn handle_remaining_shares(
        &self,
        vault: &Vault,
        sender_account: &Account,
        backend_schema: &str,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), ConsumerError> {
        // Update position
        if let Some(mut position) = Position::find_by_id(
            format!("{}-1-{}", vault.term_id, sender_account.id.to_lowercase()),
            backend_schema,
            tx.as_mut(),
        )
        .await?
        {
            position.shares = U256Wrapper::from(self.senderTotalSharesInVault);
            position.upsert(backend_schema, tx.as_mut()).await?;
        }

        Ok(())
    }

    /// This function handles the deletion of a position and related claims if the sender has no shares left
    async fn handle_triple_cleanup(
        &self,
        vault: &Vault,
        backend_schema: &str,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), ConsumerError> {
        // Handle triple-related cleanup if exists
        if let Some(triple) =
            Triple::find_by_id(vault.term_id.clone(), backend_schema, tx.as_mut()).await?
        {
            // Update predicate object
            if let Some(mut predicate_object) = PredicateObject::find_by_id(
                format!("{}-{}", triple.predicate_id, triple.object_id),
                backend_schema,
                tx.as_mut(),
            )
            .await?
            {
                predicate_object.position_count -= 1;
                predicate_object.upsert(backend_schema, tx.as_mut()).await?;
            }
        } else {
            info!(
                "No triple found for vault: {}, no need to remove claims",
                vault.term_id
            );
        }
        Ok(())
    }

    /// This function handles the deletion of a position
    async fn handle_position_redemption(
        &self,
        backend_schema: &str,
        position_id: &str,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), ConsumerError> {
        // Fetch the position
        let position =
            Position::find_by_id(position_id.to_string(), backend_schema, tx.as_mut()).await?;

        // Only if the position is being closed should we update vault position_count.
        // For instance, if the redemption fully depletes the position:
        if let Some(_pos) = position {
            info!("Position shares are zero, removing position record.");
            // Remove the position record..
            Position::delete(position_id.to_string(), backend_schema, tx.as_mut()).await?;
        }

        Ok(())
    }
}
