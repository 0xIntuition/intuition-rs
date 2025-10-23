use std::str::FromStr;

use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::{get_block_timestamp, get_counter_id_from_triple_id},
        resolver::types::ResolverConsumerMessage,
        types::DecodedConsumerContext,
        utils::short_id,
    },
    schemas::types::DecodedMessage,
};
use models::{
    account::{Account, AccountType},
    atom::{Atom, AtomResolvingStatus, AtomType},
    traits::SimpleCrud,
    triple::Triple,
    types::{FixedBytesWrapper, U256Wrapper},
};
use sqlx::{Postgres, Transaction};

/// This trait represents a fee transferred event
pub trait TripleCreatedEvent: Clone {
    /// This function returns the term ID
    fn term_id(&self) -> Result<FixedBytesWrapper, ConsumerError>;
    /// This function returns the creator ID
    fn creator_id(&self) -> Result<String, ConsumerError>;
    /// This function returns the subject ID
    fn subject_id(&self) -> Result<FixedBytesWrapper, ConsumerError>;
    /// This function returns the predicate ID
    fn predicate_id(&self) -> Result<FixedBytesWrapper, ConsumerError>;
    /// This function returns the object ID
    fn object_id(&self) -> Result<FixedBytesWrapper, ConsumerError>;

    /// This function verifies if the creator account exists in our DB. If it does, it returns it.
    /// If it does not, it creates it.
    async fn get_or_create_creator_account(
        &self,
        backend_schema: &str,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Account, ConsumerError> {
        // First try to find existing account
        if let Some(account) =
            Account::find_by_id(self.creator_id()?, backend_schema, tx.as_mut()).await?
        {
            return Ok(account);
        }

        // Only create new account if none exists
        Account::builder()
            .id(self.creator_id()?)
            .label(short_id(&self.creator_id()?))
            .account_type(AccountType::Default)
            .build()
            .upsert(backend_schema, tx.as_mut())
            .await
            .map_err(ConsumerError::ModelError)
    }
    /// This function finds an atom
    async fn find_atom(
        &self,
        backend_schema: &str,
        id: &FixedBytesWrapper,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Option<Atom>, ConsumerError> {
        Atom::find_by_id(id.clone(), backend_schema, tx.as_mut())
            .await
            .map_err(ConsumerError::ModelError)
    }
    /// This function gets or creates an account
    async fn get_or_create_temporary_account(
        &self,
        backend_schema: &str,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Account, ConsumerError> {
        if let Some(account) = Account::find_by_id(
            "0x0000000000000000000000000000000000000000".to_string(),
            backend_schema,
            tx.as_mut(),
        )
        .await?
        {
            Ok(account)
        } else {
            Account::builder()
                .id("0x0000000000000000000000000000000000000000".to_string())
                .label("Unknown".to_string())
                .account_type(AccountType::Default)
                .build()
                .upsert(backend_schema, tx.as_mut())
                .await
                .map_err(ConsumerError::ModelError)
        }
    }
    /// This function creates an atom
    async fn create_atom(
        &self,
        backend_schema: &str,
        tx: &mut Transaction<'_, Postgres>,
        atom_data: String,
        account: Account,
        term_id: FixedBytesWrapper,
        event: &DecodedMessage,
    ) -> Result<Atom, ConsumerError> {
        Atom::builder()
            .wallet_id(account.id.clone())
            .creator_id(account.id)
            .term_id(term_id.clone())
            .value_id(term_id.clone())
            .data(Atom::decode_data(atom_data.to_string())?)
            .raw_data(atom_data.to_string())
            .atom_type(AtomType::Unknown)
            .block_number(U256Wrapper::from_str("0")?)
            .created_at(get_block_timestamp(event.block_timestamp)?)
            .transaction_hash("0x0000000000000000000000000000000000000000".to_string())
            .resolving_status(AtomResolvingStatus::Pending)
            .log_index(event.log_index)
            .build()
            .upsert(backend_schema, tx.as_mut())
            .await
            .map_err(ConsumerError::ModelError)
    }
    /// This function fetches an atom or creates it
    async fn fetch_or_create_temporary_atom(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        id: FixedBytesWrapper,
        event: &DecodedMessage,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Atom, ConsumerError> {
        if let Some(atom) = self
            .find_atom(&decoded_consumer_context.backend_schema, &id, tx)
            .await?
        {
            return Ok(atom);
        }

        let atom_data = decoded_consumer_context.fetch_atom_data(id.clone()).await?;

        let account = self
            .get_or_create_temporary_account(&decoded_consumer_context.backend_schema, tx)
            .await?;

        let atom = self
            .create_atom(
                &decoded_consumer_context.backend_schema,
                tx,
                atom_data.to_string(),
                account,
                id,
                event,
            )
            .await?;

        // Enqueue the atom for resolution
        let message = ResolverConsumerMessage::new_atom(atom.term_id.0.to_string());
        decoded_consumer_context
            .client
            .send_message(serde_json::to_string(&message)?, None)
            .await?;
        Ok(atom)
    }
    /// This function gets the subject, predicate and object atoms from the DB
    /// and returns them as a tuple of atoms. If any of the atoms are not found,
    /// it returns an error.
    async fn get_subject_predicate_object_atoms(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(Atom, Atom, Atom), ConsumerError> {
        let subject_atom = self
            .fetch_or_create_temporary_atom(decoded_consumer_context, self.subject_id()?, event, tx)
            .await?;
        let predicate_atom = self
            .fetch_or_create_temporary_atom(
                decoded_consumer_context,
                self.predicate_id()?,
                event,
                tx,
            )
            .await?;
        let object_atom = self
            .fetch_or_create_temporary_atom(decoded_consumer_context, self.object_id()?, event, tx)
            .await?;
        Ok((subject_atom, predicate_atom, object_atom))
    }
    /// This function gets or creates a triple
    async fn get_or_create_triple(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Triple, ConsumerError> {
        // Get the counter vault ID
        let counter_vault_id = get_counter_id_from_triple_id(self.term_id()?)?;

        let creator_account = self
            .get_or_create_creator_account(&decoded_consumer_context.backend_schema, tx)
            .await?;

        let (subject_atom, predicate_atom, object_atom) = self
            .get_subject_predicate_object_atoms(decoded_consumer_context, event, tx)
            .await?;

        let term_id = self.term_id()?;
        let created_at = get_block_timestamp(event.block_timestamp)?;
        Triple::find_by_id(
            term_id.clone(),
            &decoded_consumer_context.backend_schema,
            tx.as_mut(),
        )
        .await?
        .unwrap_or_else(|| {
            Triple::builder()
                .creator_id(creator_account.id)
                .subject_id(subject_atom.term_id.clone())
                .predicate_id(predicate_atom.term_id.clone())
                .object_id(object_atom.term_id.clone())
                .term_id(term_id)
                .counter_term_id(counter_vault_id)
                .block_number(U256Wrapper::try_from(event.block_number).unwrap_or_default())
                .created_at(created_at)
                .transaction_hash(event.transaction_hash.clone())
                .build()
        })
        .upsert(&decoded_consumer_context.backend_schema, tx.as_mut())
        .await
        .map_err(ConsumerError::ModelError)
    }
    /// This function checks if the subject atom is an account and if the predicate and object atoms are a person or organization.
    fn is_account_with_person_or_org(
        &self,
        subject_atom: &Atom,
        predicate_atom: &Atom,
        object_atom: &Atom,
    ) -> bool {
        subject_atom.atom_type == AtomType::Account
            && ((predicate_atom.atom_type == AtomType::PersonPredicate
                && object_atom.atom_type == AtomType::Person)
                || (predicate_atom.atom_type == AtomType::OrganizationPredicate
                    && object_atom.atom_type == AtomType::Organization))
    }
    /// This function updates the account with the label and image of the object atom.
    async fn update_account(
        &self,
        subject_atom: &Atom,
        object_atom: &Atom,
        backend_schema: &str,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), ConsumerError> {
        if let Some(mut account) = Account::find_by_id(
            subject_atom
                .data
                .clone()
                .ok_or(ConsumerError::AtomDataNotFound)?,
            backend_schema,
            tx.as_mut(),
        )
        .await?
        {
            account.label = object_atom.label.clone().unwrap_or_default();
            account.image = object_atom.image.clone();
            account.upsert(backend_schema, tx.as_mut()).await?;
            Ok(())
        } else {
            Err(ConsumerError::AccountNotFound)
        }
    }

    /// This function updates the atom with the label and image of the object atom.
    async fn update_atom(
        &self,
        object_atom: &Atom,
        backend_schema: &str,
        tx: &mut Transaction<'_, Postgres>,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        if let Some(mut atom) =
            Atom::find_by_id(self.subject_id()?, backend_schema, tx.as_mut()).await?
        {
            atom.label = object_atom.label.clone();
            atom.image = object_atom.image.clone();
            atom.log_index = event.log_index;
            atom.block_number = U256Wrapper::try_from(event.block_number)?;
            atom.upsert(backend_schema, tx.as_mut()).await?;
            Ok(())
        } else {
            Err(ConsumerError::AtomNotFound)
        }
    }
    /// This function checks if the subject atom is an account and if the predicate and object atoms are a person or organization.
    /// If they are, it updates the account and atom with the label and image of the object atom.
    async fn check_and_update_account_predicate_object(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), ConsumerError> {
        let (subject_atom, predicate_atom, object_atom) = self
            .get_subject_predicate_object_atoms(decoded_consumer_context, event, tx)
            .await?;

        if self.is_account_with_person_or_org(&subject_atom, &predicate_atom, &object_atom) {
            self.update_account(
                &subject_atom,
                &object_atom,
                &decoded_consumer_context.backend_schema,
                tx,
            )
            .await?;
            self.update_atom(
                &object_atom,
                &decoded_consumer_context.backend_schema,
                tx,
                event,
            )
            .await?;
        }
        Ok(())
    }
}
