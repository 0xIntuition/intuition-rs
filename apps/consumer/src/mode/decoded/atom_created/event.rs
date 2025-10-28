use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::get_block_timestamp,
        types::DecodedConsumerContext,
        utils::{get_or_create_account, get_or_create_account_from_event},
    },
    schemas::types::DecodedMessage,
    traits::AccountManager,
};
use alloy::primitives::FixedBytes;
use models::{
    account::{Account, AccountType},
    atom::{Atom, AtomResolvingStatus, AtomType},
    traits::SimpleCrud,
    types::{FixedBytesWrapper, U256Wrapper},
};
use sqlx::PgPool;
use std::fmt::Debug;

/// This trait represents a fee transferred event
pub trait AtomCreatedEvent: AccountManager + Debug + Clone {
    fn term_id(&self) -> Result<FixedBytes<32>, ConsumerError>;
    fn creator_id(&self) -> Result<String, ConsumerError>;
    fn atom_data(&self) -> Result<String, ConsumerError>;
    /// This function creates an atom
    async fn create_atom_wallet_account_and_atom(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<Atom, ConsumerError> {
        let atom_wallet_account = self
            .get_or_create_atom_wallet_account(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await?;
        let creator_account =
            get_or_create_account(self.creator_id()?, decoded_consumer_context, None).await?;
        let atom = Atom::builder()
            .term_id(FixedBytesWrapper::from(self.term_id()?))
            .wallet_id(atom_wallet_account.id.clone())
            .creator_id(creator_account.id)
            .value_id(FixedBytesWrapper::from(self.term_id()?))
            .raw_data(self.atom_data()?)
            .atom_type(AtomType::Unknown)
            .data(Atom::decode_data(self.atom_data()?)?)
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
        Ok(atom)
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

            account.atom_id = Some(self.term_id()?.into());
            account.upsert(backend_schema, pg_pool).await?;
        }
        Ok(account)
    }
}
