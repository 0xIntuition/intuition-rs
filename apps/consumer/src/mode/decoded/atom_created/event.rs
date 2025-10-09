use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::get_block_timestamp,
        resolver::types::ResolverConsumerMessage,
        types::DecodedConsumerContext,
        utils::{VaultOrigin, get_or_create_account, get_or_create_account_from_event},
    },
    schemas::types::DecodedMessage,
    traits::{AccountManager, SharePriceEvent, VaultManager},
};
use models::{
    account::{Account, AccountType},
    atom::{Atom, AtomResolvingStatus, AtomType},
    term::{Term, TermType},
    traits::SimpleCrud,
    types::{FixedBytesWrapper, U256Wrapper},
    vault::Vault,
};
use sqlx::PgPool;
use std::fmt::Debug;
use tracing::{debug, error, warn};

/// This trait represents a fee transferred event
pub trait AtomCreatedEvent:
    SharePriceEvent + VaultManager + AccountManager + Debug + Clone
{
    fn creator_id(&self) -> Result<String, ConsumerError>;
    fn atom_data(&self) -> Result<String, ConsumerError>;
    /// This function updates the vault current share price and it returns the vault and atom
    async fn get_or_create_vault_and_atom(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(Vault, Atom), ConsumerError> {
        debug!("Creating vault for atom {}", self.term_id()?);
        // Get or create the vault
        let vault = match VaultOrigin::AtomCreated
            .get_or_create_vault(
                self.clone(),
                decoded_consumer_context,
                TermType::Atom,
                event,
                None,
            )
            .await
        {
            Ok(vault) => vault,
            Err(e) => {
                VaultOrigin::handle_vault_insert_error(
                    e,
                    self.term_id()?.into(),
                    self.curve_id()?,
                    decoded_consumer_context,
                )
                .await?
            }
        };

        // In order to upsert a [`Vault`] we need to have an [`Atom`] first.
        // Verify that the atom exists, if not, create it. Note that in order
        // to create the atom, we need to have the creator and the wallet accounts
        // created first, so if they don't exist, we create them as part of this
        // process.
        let atom = self
            .get_or_create_vault_atom(decoded_consumer_context, event)
            .await?;

        Ok((vault, atom))
    }
    /// This function verifies if the atom exists in our DB. If it does, it returns it.
    /// If it does not, it creates it.
    async fn get_or_create_vault_atom(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<Atom, ConsumerError> {
        if let Some(atom) = Atom::find_by_id(
            self.term_id()?.into(),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        {
            if atom.transaction_hash == "0x0000000000000000000000000000000000000000" {
                debug!("Atom exists with zero transaction hash, updating it");
                let atom = self
                    .update_atom_with_zero_transaction_hash_or_create_atom(
                        decoded_consumer_context,
                        event,
                    )
                    .await?;
                return Ok(atom);
            }
            // If the atom exists, return it
            debug!("Atom already exists, returning it");
            Ok(atom)
        } else {
            debug!("Atom does not exist, creating it");
            let atom = self
                .update_atom_with_zero_transaction_hash_or_create_atom(
                    decoded_consumer_context,
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
        event: &DecodedMessage,
    ) -> Result<Atom, ConsumerError> {
        let mut atom_wallet_account = self
            .get_or_create_atom_wallet_account(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await?;
        let creator_account =
            get_or_create_account(self.creator_id()?, decoded_consumer_context).await?;
        let atom = Atom::builder()
            .term_id(FixedBytesWrapper::from(self.term_id()?))
            .wallet_id(atom_wallet_account.id.clone())
            .creator_id(creator_account.id)
            .value_id(FixedBytesWrapper::from(self.term_id()?))
            .raw_data(self.atom_data()?)
            .atom_type(AtomType::Unknown)
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .created_at(get_block_timestamp(event.block_timestamp)?)
            .transaction_hash(event.transaction_hash.clone())
            .resolving_status(AtomResolvingStatus::Pending)
            .log_index(event.log_index)
            .build()
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await?;
        Self::update_account_with_atom_id(
            &mut atom_wallet_account,
            atom.term_id.clone(),
            decoded_consumer_context,
        )
        .await?;
        Self::update_term_created_at(self, decoded_consumer_context, event).await?;
        Ok(atom)
    }

    /// This function updates the term created at for atoms with a zero transaction hash
    async fn update_term_created_at(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        if let Some(mut term) = Term::find_by_id(
            self.term_id()?.into(),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        {
            term.created_at = get_block_timestamp(event.block_timestamp)?;
            term.upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await?;
        } else {
            error!("Term does not exist, skipping update");
        }
        Ok(())
    }
    /// This function verifies if the atom wallet account exists in our DB. If it does, it returns it.
    /// If it does not, it creates it.
    async fn get_or_create_atom_wallet_account(
        &self,
        backend_schema: &str,
        pg_pool: &PgPool,
    ) -> Result<Account, ConsumerError> {
        // First try to find existing account
        let mut account =
            get_or_create_account_from_event(self.clone(), backend_schema, pg_pool).await?;

        // We update the account type to `AtomWallet` if it is not already set
        if account.account_type != AccountType::AtomWallet {
            account.account_type = AccountType::AtomWallet;
            account.upsert(backend_schema, pg_pool).await?;
        }
        Ok(account)
    }
    /// This function updates an account with an atom ID and enqueues a resolver message
    async fn update_account_with_atom_id(
        account: &mut Account,
        atom_id: FixedBytesWrapper,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        account.atom_id = Some(atom_id);
        account
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await?;
        debug!("Updated account: {:?}", account);

        // Now we need to enqueue the message to be processed by the resolver. In this
        // process we check if the account has ENS data associated, and if it does, we
        // update the account with the ENS data (name [label] and image)
        let message = ResolverConsumerMessage::new_account(account.clone());
        decoded_consumer_context
            .client
            .send_message(serde_json::to_string(&message)?, None)
            .await?;
        Ok(())
    }
    /// This function decodes the atom data
    async fn decode_atom_data_and_update_atom(
        &self,
        atom: &mut Atom,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<String, ConsumerError> {
        // decode the hex data from the atomData.
        let decoded_atom_data = if let Ok(decoded_atom_data) = Atom::decode_data(self.atom_data()?)
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
        atom.block_number = U256Wrapper::try_from(event.block_number)?;
        atom.log_index = event.log_index;
        atom.upsert(
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?;
        Ok(decoded_atom_data)
    }
}
