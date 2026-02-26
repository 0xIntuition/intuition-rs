use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::{get_block_timestamp, get_counter_id_from_triple_id},
        types::DecodedConsumerContext,
        utils::short_id,
    },
    schemas::types::DecodedMessage,
};
use models::{
    account::{Account, AccountType},
    atom::{Atom, AtomType},
    term::{Term, TermType},
    traits::SimpleCrud,
    triple::Triple,
    types::{FixedBytesWrapper, U256Wrapper},
};
use sqlx::PgPool;

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
        pg_pool: &PgPool,
    ) -> Result<Account, ConsumerError> {
        // First try to find existing account
        if let Some(account) =
            Account::find_by_id(self.creator_id()?, backend_schema, pg_pool).await?
        {
            return Ok(account);
        }

        // Only create new account if none exists
        Account::builder()
            .id(self.creator_id()?)
            .label(short_id(&self.creator_id()?))
            .account_type(AccountType::Default)
            .build()
            .upsert(backend_schema, pg_pool)
            .await
            .map_err(ConsumerError::ModelError)
    }
    /// This function gets the subject, predicate and object atoms from the DB
    /// and returns them as a tuple of atoms. If any of the atoms are not found,
    /// it returns an error.
    async fn get_subject_predicate_object_atoms(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(Atom, Atom, Atom), ConsumerError> {
        decoded_consumer_context
            .retry_with_backoff(|| async {
                Atom::find_subject_predicate_object(
                    self.subject_id()?,
                    self.predicate_id()?,
                    self.object_id()?,
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool,
                )
                .await
                .map_err(ConsumerError::ModelError)
            })
            .await
    }
    /// This function gets or creates a triple
    async fn get_or_create_triple(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<Triple, ConsumerError> {
        // Get the counter vault ID
        let counter_vault_id = get_counter_id_from_triple_id(self.term_id()?)?;

        let creator_account = self
            .get_or_create_creator_account(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await?;

        let term_id = self.term_id()?;
        let created_at = get_block_timestamp(event.block_timestamp)?;
        let subject_id = self.subject_id()?;
        let predicate_id = self.predicate_id()?;
        let object_id = self.object_id()?;
        Triple::find_by_id(
            term_id.clone(),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        .unwrap_or_else(|| {
            Triple::builder()
                .creator_id(creator_account.id)
                .subject_id(subject_id)
                .predicate_id(predicate_id)
                .object_id(object_id)
                .term_id(term_id)
                .counter_term_id(counter_vault_id)
                .block_number(U256Wrapper::try_from(event.block_number).unwrap_or_default())
                .created_at(created_at)
                .transaction_hash(event.transaction_hash.clone())
                .build()
        })
        .upsert(
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
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
        decoded_consumer_context: &DecodedConsumerContext,
        subject_atom: &Atom,
        object_atom: &Atom,
    ) -> Result<(), ConsumerError> {
        if let Some(mut account) = Account::find_by_id(
            subject_atom
                .data
                .clone()
                .ok_or(ConsumerError::AtomDataNotFound)?,
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        {
            account.label = object_atom.label.clone().unwrap_or_default();
            account.image = object_atom.image.clone();
            account
                .upsert(
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool,
                )
                .await?;
            Ok(())
        } else {
            Err(ConsumerError::AccountNotFound)
        }
    }
    /// Attempts to get subject, predicate, and object as atoms. Returns Ok(None)
    /// if any component is a triple (nested triple) rather than an atom.
    async fn try_get_subject_predicate_object_atoms(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<Option<(Atom, Atom, Atom)>, ConsumerError> {
        let subject_term = Term::find_by_id(
            self.subject_id()?,
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?;
        let predicate_term = Term::find_by_id(
            self.predicate_id()?,
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?;
        let object_term = Term::find_by_id(
            self.object_id()?,
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?;

        // If any term is missing or not an atom, skip account update logic
        match (subject_term, predicate_term, object_term) {
            (Some(s), Some(p), Some(o))
                if s.term_type == TermType::Atom
                    && p.term_type == TermType::Atom
                    && o.term_type == TermType::Atom =>
            {
                let atoms = Atom::find_subject_predicate_object(
                    self.subject_id()?,
                    self.predicate_id()?,
                    self.object_id()?,
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool,
                )
                .await
                .map_err(ConsumerError::ModelError)?;
                Ok(Some(atoms))
            }
            _ => Ok(None),
        }
    }
    /// This function checks if the subject atom is an account and if the predicate and object atoms are a person or organization.
    /// If they are, it updates the account and atom with the label and image of the object atom.
    async fn check_and_update_account_predicate_object(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        if let Some((subject_atom, predicate_atom, object_atom)) = self
            .try_get_subject_predicate_object_atoms(decoded_consumer_context)
            .await?
        {
            if self.is_account_with_person_or_org(&subject_atom, &predicate_atom, &object_atom) {
                self.update_account(decoded_consumer_context, &subject_atom, &object_atom)
                    .await?;
            }
        }
        Ok(())
    }
}
