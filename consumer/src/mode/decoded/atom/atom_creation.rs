use crate::{
    EthMultiVault::AtomCreated,
    error::ConsumerError,
    mode::{
        decoded::{
            atom::atom_supported_types::get_supported_atom_metadata,
            utils::{get_or_create_account, short_id, update_account_with_atom_id},
        },
        resolver::types::ResolveAtom,
        types::DecodedConsumerContext,
    },
    schemas::types::DecodedMessage,
};
use models::{
    account::{Account, AccountType},
    atom::{Atom, AtomResolvingStatus, AtomType},
    event::{Event, EventType},
    position::Position,
    traits::SimpleCrud,
    types::U256Wrapper,
    vault::Vault,
};
use std::str::FromStr;
use tracing::{info, warn};
impl AtomCreated {
    /// This function creates an `Event` for the `AtomCreated` event
    async fn create_event(
        &self,
        event: &DecodedMessage,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<Event, ConsumerError> {
        // Create the event
        Event::builder()
            .id(DecodedMessage::event_id(event))
            .event_type(EventType::AtomCreated)
            .atom_id(self.vaultID)
            .block_number(U256Wrapper::from_str(&event.block_number.to_string())?)
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

    /// This function decodes the atom data
    async fn decode_atom_data_and_update_atom(
        &self,
        atom: &mut Atom,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<String, ConsumerError> {
        // decode the hex data from the atomData.
        let decoded_atom_data = if let Ok(decoded_atom_data) =
            Atom::decode_data(self.atomData.to_string())
        {
            decoded_atom_data
        } else {
            warn!(
                "Failed to decode atom data. This is not a critical error, but this atom will be created with empty data and `Unknown` type.",
            );
            // return an empty string
            String::new()
        };

        // Update the atom with the decoded data
        atom.data = Some(decoded_atom_data.clone());
        atom.upsert(
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;
        Ok(decoded_atom_data)
    }

    /// This function verifies if the atom wallet account exists in our DB. If it does, it returns it.
    /// If it does not, it creates it.
    async fn get_or_create_atom_wallet_account(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<Account, ConsumerError> {
        // First try to find existing account
        if let Some(mut account) = Account::find_by_id(
            self.atomWallet.to_string(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        {
            // We update the account type to `AtomWallet` if it is not already set
            if account.account_type != AccountType::AtomWallet {
                account.account_type = AccountType::AtomWallet;
                account
                    .upsert(
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.backend_schema,
                    )
                    .await?;
            }
            return Ok(account);
        }

        // Only create new account if none exists
        Account::builder()
            .id(self.atomWallet.to_string())
            .label(short_id(&self.atomWallet.to_string()))
            .account_type(AccountType::AtomWallet)
            .build()
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
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
            &decoded_consumer_context.backend_schema,
        )
        .await?
        {
            // If the atom exists, return it
            info!("Atom already exists, returning it");
            Ok(atom)
        } else {
            info!("Atom does not exist, creating it");
            let atom_wallet_account = self
                .get_or_create_atom_wallet_account(decoded_consumer_context)
                .await?;
            let creator_account =
                get_or_create_account(self.creator.to_string(), decoded_consumer_context).await?;
            // Create the `Atom` and upsert it. Note that we are using the raw_data as the data
            // for now, this will be updated later with the resolver consumer.
            let atom = Atom::builder()
                .id(U256Wrapper::from_str(
                    &self.vaultID.to_string().to_lowercase(),
                )?)
                .wallet_id(atom_wallet_account.id.clone())
                .creator_id(creator_account.id)
                .vault_id(U256Wrapper::from_str(&self.vaultID.to_string())?)
                .value_id(U256Wrapper::from_str(&self.vaultID.to_string())?)
                .raw_data(self.atomData.to_string())
                .atom_type(AtomType::Unknown)
                .block_number(U256Wrapper::from_str(&event.block_number.to_string())?)
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .resolving_status(AtomResolvingStatus::Pending)
                .build()
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;
            //updating the account with the atom id
            update_account_with_atom_id(
                atom_wallet_account.id,
                atom.id.clone(),
                decoded_consumer_context,
            )
            .await?;

            Ok(atom)
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

        // decode the hex data from the atomData.
        let decoded_atom_data = self
            .decode_atom_data_and_update_atom(&mut atom, decoded_consumer_context)
            .await?;

        // get the supported atom metadata and update the atom metadata
        let supported_atom_metadata =
            get_supported_atom_metadata(&mut atom, &decoded_atom_data, decoded_consumer_context)
                .await?
                .update_atom_metadata(
                    &mut atom,
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;

        // Handle the account or caip10 type
        let resolved_atom = ResolveAtom { atom: atom.clone() };
        supported_atom_metadata
            .handle_account_or_caip10_type(&resolved_atom, decoded_consumer_context)
            .await?;

        // Create the event
        self.create_event(decoded_message, decoded_consumer_context)
            .await?;

        Ok(())
    }

    /// This function verifies if the vault exists in our DB. If it does, it returns it.
    /// If it does not, it creates it.
    async fn get_or_create_vault(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<Vault, ConsumerError> {
        if let Some(vault) = Vault::find_by_id(
            U256Wrapper::from_str(&self.vaultID.to_string())?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        {
            info!("Vault already exists, returning it");
            Ok(vault)
        } else {
            // create the vault
            Vault::builder()
                .id(Vault::format_vault_id(self.vaultID.to_string(), None))
                .total_shares(
                    decoded_consumer_context
                        .fetch_total_shares_in_vault(self.vaultID, event.block_number)
                        .await?,
                )
                .current_share_price(
                    decoded_consumer_context
                        .fetch_current_share_price(self.vaultID, event.block_number)
                        .await?,
                )
                .position_count(
                    Position::count_by_vault(
                        U256Wrapper::from_str(&self.vaultID.to_string())?,
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.backend_schema,
                    )
                    .await? as i32,
                )
                .build()
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await
                .map_err(ConsumerError::ModelError)
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
            .fetch_current_share_price(self.vaultID, event.block_number)
            .await?;

        // Get or create the vault
        self.get_or_create_vault(decoded_consumer_context, event)
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
        let vault = Vault::update_current_share_price(
            U256Wrapper::from_str(&self.vaultID.to_string())?,
            U256Wrapper::from_str(&current_share_price.to_string())?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        Ok((vault, atom))
    }
}
