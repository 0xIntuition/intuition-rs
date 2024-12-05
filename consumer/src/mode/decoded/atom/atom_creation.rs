use crate::{
    error::ConsumerError,
    mode::{
        decoded::{
            atom::atom_supported_types::get_supported_atom_metadata,
            utils::{get_or_create_account, short_id},
        },
        resolver::types::ResolveAtom,
        types::DecodedConsumerContext,
    },
    schemas::types::DecodedMessage,
    EthMultiVault::AtomCreated,
};
use alloy::{eips::BlockId, primitives::U256};
use log::{info, warn};
use models::{
    account::{Account, AccountType},
    atom::{Atom, AtomResolvingStatus, AtomType},
    event::{Event, EventType},
    traits::SimpleCrud,
    types::U256Wrapper,
    vault::Vault,
};
use sqlx::PgPool;
use std::str::FromStr;

impl AtomCreated {
    /// This function creates an `Event` for the `AtomCreated` event
    async fn create_event(
        &self,
        event: &DecodedMessage,
        pg_pool: &PgPool,
    ) -> Result<Event, ConsumerError> {
        // Create the event
        Event::builder()
            .id(event.transaction_hash.clone())
            .event_type(EventType::AtomCreated)
            .atom_id(self.vaultID)
            .block_number(U256Wrapper::from_str(&event.block_number.to_string())?)
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
            .build()
            .upsert(pg_pool)
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function decodes the atom data
    fn decode_data(&self) -> Result<String, ConsumerError> {
        Ok(String::from_utf8(self.atomData.clone().to_vec())?)
    }

    /// This function verifies if the atom wallet account exists in our DB. If it does, it returns it.
    /// If it does not, it creates it.
    async fn get_or_create_atom_wallet_account(
        &self,
        pg_pool: &PgPool,
    ) -> Result<Account, ConsumerError> {
        Account::find_by_id(self.atomWallet.to_string(), pg_pool)
            .await?
            .unwrap_or_else(|| {
                Account::builder()
                    .id(self.atomWallet.to_string())
                    .label(short_id(&self.atomWallet.to_string()))
                    .account_type(AccountType::AtomWallet)
                    .build()
            })
            .upsert(pg_pool)
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function verifies if the atom exists in our DB. If it does, it returns it.
    /// If it does not, it creates it.
    async fn get_or_create_vault_atom(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<Atom, ConsumerError> {
        if let Some(atom) = Atom::find_by_id(
            U256Wrapper::from_str(&self.vaultID.to_string())?,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        {
            // If the atom exists, return it
            info!("Atom already exists, returning it");
            Ok(atom)
        } else {
            info!("Atom does not exist, creating it");
            let atom_wallet_account = self
                .get_or_create_atom_wallet_account(&decoded_consumer_context.pg_pool)
                .await?;
            let creator_account =
                get_or_create_account(self.creator.to_string(), decoded_consumer_context).await?;
            // Create the `Atom` and upsert it
            Atom::builder()
                .id(U256Wrapper::from_str(
                    &self.vaultID.to_string().to_lowercase(),
                )?)
                .wallet_id(atom_wallet_account.id)
                .creator_id(creator_account.id)
                .vault_id(U256Wrapper::from_str(&self.vaultID.to_string())?)
                .value_id(U256Wrapper::from_str(&self.vaultID.to_string())?)
                .data(self.atomData.to_string())
                .atom_type(AtomType::Unknown)
                .block_number(U256Wrapper::from_str(&event.block_number.to_string())?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .resolving_status(AtomResolvingStatus::Pending)
                .build()
                .upsert(&decoded_consumer_context.pg_pool)
                .await
                .map_err(ConsumerError::ModelError)
        }
    }

    /// This function handles an `AtomCreated` event. This is the most important function
    /// in the atom creation process.
    pub async fn handle_atom_creation(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        decoded_message: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Handling atom creation: {self:#?}");

        // Update the vault current share price
        let (_vault, mut atom) = self
            .update_vault_current_share_price(decoded_consumer_context, decoded_message)
            .await?;

        // decode the hex data from the atomData. The only known problem we can have here
        // is that it can fail to decode UTF-8 string.
        match self.decode_data() {
            Ok(decoded_atom_data) => {
                // get the supported atom metadata
                let supported_atom_metadata = get_supported_atom_metadata(
                    &mut atom,
                    &decoded_atom_data,
                    decoded_consumer_context,
                )
                .await?;

                // Handle the account type
                let resolved_atom = ResolveAtom {
                    atom: atom.clone(),
                    decoded_atom_data,
                };
                supported_atom_metadata
                    .handle_account_type(&resolved_atom, decoded_consumer_context)
                    .await?;

                // Update the atom metadata to reflect the supported atom type
                supported_atom_metadata
                    .update_atom_metadata(&mut atom, &decoded_consumer_context.pg_pool)
                    .await?;

                // Create the event
                self.create_event(decoded_message, &decoded_consumer_context.pg_pool)
                    .await?;

                Ok(())
            }
            Err(error) => {
                warn!(
                    "Failed to decode atom data with error: {}. This is not a critical error, but this atom will not be created.",
                    error
                );
                Ok(())
            }
        }
    }

    /// This function updates the vault current share price and it returns the vault and atom
    async fn update_vault_current_share_price(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(Vault, Atom), ConsumerError> {
        // Get the share price of the atom
        let current_share_price = decoded_consumer_context
            .base_client
            .currentSharePrice(self.vaultID)
            .block(BlockId::from_str(&event.block_number.to_string())?)
            .call()
            .await?;

        // In order to upsert a [`Vault`] we need to have an [`Atom`] first.
        // Verify that the atom exists, if not, create it. Note that in order
        // to create the atom, we need to have the creator and the wallet accounts
        // created first, so if they don't exist, we create them as part of this
        // process.
        let atom = self
            .get_or_create_vault_atom(decoded_consumer_context, event)
            .await?;

        // Update the respective vault with the correct share price
        let vault = Vault::builder()
            .id(atom.vault_id.clone())
            .atom_id(atom.vault_id.clone())
            .total_shares(U256Wrapper::from(U256::from(0)))
            .current_share_price(U256Wrapper::from_str(&current_share_price._0.to_string())?)
            .position_count(0)
            .build()
            .upsert(&decoded_consumer_context.pg_pool)
            .await?;

        Ok((vault, atom))
    }
}
