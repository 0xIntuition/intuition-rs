use crate::{
    EthMultiVault::AtomCreated,
    error::ConsumerError,
    mode::{
        metadata::get_supported_atom_metadata,
        resolver::types::ResolveAtom,
        types::DecodedConsumerContext,
        utils::{get_or_create_account, short_id, update_account_with_atom_id},
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
use sqlx::{Postgres, Transaction};
use std::str::FromStr;
use tracing::{info, warn};
impl AtomCreated {
    /// This function creates an `Event` for the `AtomCreated` event
    async fn create_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
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
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function decodes the atom data
    async fn decode_atom_data_and_update_atom(
        &self,
        atom: &mut Atom,
        backend_schema: &str,
        tx: &mut Transaction<'_, Postgres>,
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
        atom.upsert(backend_schema, tx.as_mut()).await?;
        Ok(decoded_atom_data)
    }

    /// This function verifies if the atom wallet account exists in our DB. If it does, it returns it.
    /// If it does not, it creates it.
    async fn get_or_create_atom_wallet_account(
        &self,
        backend_schema: &str,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Account, ConsumerError> {
        // First try to find existing account
        if let Some(mut account) =
            Account::find_by_id(self.atomWallet.to_string(), backend_schema, tx.as_mut()).await?
        {
            // We update the account type to `AtomWallet` if it is not already set
            if account.account_type != AccountType::AtomWallet {
                account.account_type = AccountType::AtomWallet;
                account.upsert(backend_schema, tx.as_mut()).await?;
            }
            return Ok(account);
        }

        // Only create new account if none exists
        Account::builder()
            .id(self.atomWallet.to_string())
            .label(short_id(&self.atomWallet.to_string()))
            .account_type(AccountType::AtomWallet)
            .build()
            .upsert(backend_schema, tx.as_mut())
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function verifies if the atom exists in our DB. If it does, it returns it.
    /// If it does not, it creates it.
    async fn get_or_create_vault_atom(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Atom, ConsumerError> {
        if let Some(atom) = Atom::find_by_id(
            self.vaultID.into(),
            &decoded_consumer_context.backend_schema,
            tx.as_mut(),
        )
        .await?
        {
            if atom.transaction_hash == "0x0000000000000000000000000000000000000000" {
                info!("Atom exists with zero transaction hash, updating it");
                let atom = self
                    .update_atom_with_zero_transaction_hash_or_create_atom(
                        decoded_consumer_context,
                        tx,
                        event,
                    )
                    .await?;
                return Ok(atom);
            }
            // If the atom exists, return it
            info!("Atom already exists, returning it");
            Ok(atom)
        } else {
            info!("Atom does not exist, creating it");
            let atom = self
                .update_atom_with_zero_transaction_hash_or_create_atom(
                    decoded_consumer_context,
                    tx,
                    event,
                )
                .await?;

            Ok(atom)
        }
    }

    /// This function updates an atom with a zero transaction hash
    async fn update_atom_with_zero_transaction_hash_or_create_atom(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        tx: &mut Transaction<'_, Postgres>,
        event: &DecodedMessage,
    ) -> Result<Atom, ConsumerError> {
        let mut atom_wallet_account = self
            .get_or_create_atom_wallet_account(&decoded_consumer_context.backend_schema, tx)
            .await?;
        let creator_account =
            get_or_create_account(self.creator.to_string(), decoded_consumer_context).await?;
        let atom = Atom::builder()
            .term_id(self.vaultID)
            .wallet_id(atom_wallet_account.id.clone())
            .creator_id(creator_account.id)
            .value_id(U256Wrapper::from_str(&self.vaultID.to_string())?)
            .raw_data(self.atomData.to_string())
            .atom_type(AtomType::Unknown)
            .block_number(U256Wrapper::from_str(&event.block_number.to_string())?)
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
            .resolving_status(AtomResolvingStatus::Pending)
            .build()
            .upsert(&decoded_consumer_context.backend_schema, tx.as_mut())
            .await?;
        update_account_with_atom_id(
            &mut atom_wallet_account,
            atom.term_id.clone(),
            decoded_consumer_context,
            tx,
        )
        .await?;
        Ok(atom)
    }

    /// This function handles an `AtomCreated` event. This is the most important function
    /// in the atom creation process.
    pub async fn handle_atom_creation(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        decoded_message: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Handling atom creation: {self:#?}");
        let mut tx = decoded_consumer_context.pg_pool.begin().await?;
        // Update the vault current share price
        let (_vault, mut atom) = self
            .update_vault_current_share_price(decoded_consumer_context, decoded_message, &mut tx)
            .await?;
        // We commit the first mini batch of transactions to release the locks
        tx.commit().await?;

        // We start a new transaction to decode the atom data and update the atom metadata
        let mut tx_2 = decoded_consumer_context.pg_pool.begin().await?;

        // decode the hex data from the atomData.
        let decoded_atom_data = self
            .decode_atom_data_and_update_atom(
                &mut atom,
                &decoded_consumer_context.backend_schema,
                &mut tx_2,
            )
            .await?;

        // get the supported atom metadata and update the atom metadata
        let supported_atom_metadata =
            get_supported_atom_metadata(&mut atom, &decoded_atom_data, decoded_consumer_context)
                .await?
                .update_atom_metadata(
                    &mut atom,
                    &decoded_consumer_context.backend_schema,
                    &mut tx_2,
                )
                .await?;

        // Handle the account or caip10 type
        let resolved_atom = ResolveAtom { atom: atom.clone() };
        supported_atom_metadata
            .handle_account_or_caip10_type(&resolved_atom, decoded_consumer_context, &mut tx_2)
            .await?;
        tx_2.commit().await?;

        // Create the event
        self.create_event(decoded_consumer_context, decoded_message)
            .await?;

        Ok(())
    }

    /// This function verifies if the vault exists in our DB. If it does, it returns it.
    /// If it does not, it creates it.
    async fn get_or_create_vault(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Vault, ConsumerError> {
        if let Some(vault) = Vault::find_by_term_id_and_curve_id(
            U256Wrapper::from(self.vaultID),
            1.try_into()?,
            tx.as_mut(),
            &decoded_consumer_context.backend_schema,
        )
        .await?
        {
            info!("Vault already exists, returning it");
            Ok(vault)
        } else {
            // create the vault
            Vault::builder()
                .term_id(self.vaultID)
                .curve_id(U256Wrapper::from_str("1")?)
                .total_assets(U256Wrapper::from_str("0")?)
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
                    Position::count_by_vault_and_curve(
                        self.vaultID.into(),
                        1.try_into()?,
                        &decoded_consumer_context.pg_pool,
                        &decoded_consumer_context.backend_schema,
                    )
                    .await? as i32,
                )
                .build()
                .upsert(&decoded_consumer_context.backend_schema, tx.as_mut())
                .await
                .map_err(ConsumerError::ModelError)
        }
    }

    /// This function updates the vault current share price and it returns the vault and atom
    async fn update_vault_current_share_price(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(Vault, Atom), ConsumerError> {
        // Get the share price of the atom
        let current_share_price = decoded_consumer_context
            .fetch_current_share_price(self.vaultID, event.block_number)
            .await?;

        // Get or create the vault
        self.get_or_create_vault(decoded_consumer_context, event, tx)
            .await?;

        // In order to upsert a [`Vault`] we need to have an [`Atom`] first.
        // Verify that the atom exists, if not, create it. Note that in order
        // to create the atom, we need to have the creator and the wallet accounts
        // created first, so if they don't exist, we create them as part of this
        // process.
        let atom = self
            .get_or_create_vault_atom(decoded_consumer_context, event, tx)
            .await?;
        // Update the respective vault with the correct share price
        let vault = Vault::update_current_share_price(
            self.vaultID.into(),
            current_share_price.into(),
            tx.as_mut(),
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        Ok((vault, atom))
    }
}
