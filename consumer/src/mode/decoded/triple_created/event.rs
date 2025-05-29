use crate::{
    error::ConsumerError,
    mode::{
        resolver::types::ResolverConsumerMessage,
        types::DecodedConsumerContext,
        utils::{Origin, get_or_create_term, short_id},
    },
    schemas::types::DecodedMessage,
    traits::{SharePriceEvent, VaultManager},
};
use alloy::primitives::Uint;
use models::{
    account::{Account, AccountType},
    atom::{Atom, AtomResolvingStatus, AtomType},
    predicate_object::PredicateObject,
    term::TermType,
    traits::SimpleCrud,
    triple::Triple,
    types::U256Wrapper,
    vault::Vault,
};
use sqlx::{Postgres, Transaction};
use std::{fmt::Debug, str::FromStr};
use tracing::warn;

/// This trait represents a fee transferred event
pub trait TripleCreatedEvent: SharePriceEvent + VaultManager + Debug + Clone {
    /// This function returns the vault ID
    fn vault_id(&self) -> Result<Uint<256, 4>, ConsumerError>;
    /// This function returns the creator ID
    fn creator_id(&self) -> Result<String, ConsumerError>;
    /// This function returns the subject ID
    fn subject_id(&self) -> Result<Uint<256, 4>, ConsumerError>;
    /// This function returns the predicate ID
    fn predicate_id(&self) -> Result<Uint<256, 4>, ConsumerError>;
    /// This function returns the object ID
    fn object_id(&self) -> Result<Uint<256, 4>, ConsumerError>;
    /// This function updates the vault and counter vault current share prices
    async fn get_or_create_vaults(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        // Get the counter vault ID
        let counter_vault_id = decoded_consumer_context
            .get_counter_id_from_triple(self.vault_id()?)
            .await?;

        // Get or update the vault
        Origin::TripleCreated
            .get_or_create_vault(
                self.clone(),
                decoded_consumer_context,
                TermType::Triple,
                event,
            )
            .await?;

        // Get or update the counter vault
        self.get_or_create_counter_vault(
            U256Wrapper::from(counter_vault_id),
            decoded_consumer_context,
            event,
        )
        .await?;

        Ok(())
    }
    /// This function gets or creates a counter vault
    async fn get_or_create_counter_vault(
        &self,
        counter_vault_id: U256Wrapper,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<Vault, ConsumerError> {
        let vault = Vault::find_by_term_id_and_curve_id(
            counter_vault_id.clone(),
            U256Wrapper::from_str("1")?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;

        if let Some(vault) = vault {
            Ok(vault)
        } else {
            // Ensure that the term exists for the vault
            get_or_create_term(
                &self.clone(),
                Some(counter_vault_id.clone()),
                decoded_consumer_context,
                TermType::Triple,
            )
            .await?;

            let new_vault = Vault::builder()
                .term_id(counter_vault_id)
                .curve_id(U256Wrapper::from_str("1")?)
                .current_share_price(
                    self.current_share_price(decoded_consumer_context, event.block_number)
                        .await?,
                )
                .position_count(0)
                .block_number(event.block_number)
                .log_index(event.log_index)
                .transaction_hash(event.transaction_hash.clone())
                .total_assets(self.total_assets()?)
                .market_cap(self.market_cap()?)
                .total_shares(
                    self.total_shares(decoded_consumer_context, event.block_number)
                        .await?,
                )
                .build()
                .upsert(
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool,
                )
                .await
                .map_err(ConsumerError::ModelError)?;

            Ok(new_vault)
        }
    }
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
        id: &U256Wrapper,
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
        vault: Vault,
        event: &DecodedMessage,
    ) -> Result<Atom, ConsumerError> {
        Atom::builder()
            .wallet_id(account.id.clone())
            .creator_id(account.id)
            .term_id(vault.term_id.clone())
            .value_id(vault.term_id.clone())
            .data(Atom::decode_data(atom_data.to_string())?)
            .raw_data(atom_data.to_string())
            .atom_type(AtomType::Unknown)
            .block_number(U256Wrapper::from_str("0")?)
            .block_timestamp(0)
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
        id: U256Wrapper,
        event: &DecodedMessage,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Atom, ConsumerError> {
        if let Some(atom) = self
            .find_atom(&decoded_consumer_context.backend_schema, &id, tx)
            .await?
        {
            return Ok(atom);
        }

        let atom_data = decoded_consumer_context
            .fetch_atom_data(self.subject_id()?)
            .await?;

        let account = self
            .get_or_create_temporary_account(&decoded_consumer_context.backend_schema, tx)
            .await?;

        let vault = match Origin::TripleCreated
            .get_or_create_vault(
                self.clone(),
                decoded_consumer_context,
                TermType::Triple,
                event,
            )
            .await
        {
            Ok(vault) => vault,
            Err(e) => {
                warn!("Error inserting vault: {:?}, returning existing vault", e);
                Vault::find_by_term_id_and_curve_id(
                    self.vault_id()?.into(),
                    self.curve_id()?,
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?
                .ok_or(ConsumerError::VaultNotFound)?
            }
        };

        let atom = self
            .create_atom(
                &decoded_consumer_context.backend_schema,
                tx,
                atom_data.to_string(),
                account,
                vault,
                event,
            )
            .await?;

        // Enqueue the atom for resolution
        let message = ResolverConsumerMessage::new_atom(atom.term_id.to_string());
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
            .fetch_or_create_temporary_atom(
                decoded_consumer_context,
                U256Wrapper::from(self.subject_id()?),
                event,
                tx,
            )
            .await?;
        let predicate_atom = self
            .fetch_or_create_temporary_atom(
                decoded_consumer_context,
                U256Wrapper::from(self.predicate_id()?),
                event,
                tx,
            )
            .await?;
        let object_atom = self
            .fetch_or_create_temporary_atom(
                decoded_consumer_context,
                U256Wrapper::from(self.object_id()?),
                event,
                tx,
            )
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
        let counter_vault_id = decoded_consumer_context
            .get_counter_id_from_triple(self.vault_id()?)
            .await?;

        let creator_account = self
            .get_or_create_creator_account(&decoded_consumer_context.backend_schema, tx)
            .await?;

        let (subject_atom, predicate_atom, object_atom) = self
            .get_subject_predicate_object_atoms(decoded_consumer_context, event, tx)
            .await?;

        let term_id = U256Wrapper::from(self.vault_id()?);
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
                .counter_term_id(U256Wrapper::from(counter_vault_id))
                .block_number(U256Wrapper::try_from(event.block_number).unwrap_or_default())
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .build()
        })
        .upsert(&decoded_consumer_context.backend_schema, tx.as_mut())
        .await
        .map_err(ConsumerError::ModelError)
    }
    /// This function updates the predicate object triple count
    async fn update_predicate_object_triple_count(
        &self,
        backend_schema: &str,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), ConsumerError> {
        let id = format!("{}-{}", self.predicate_id()?, self.object_id()?);
        if let Some(mut predicate_object) =
            PredicateObject::find_by_id(id.clone(), backend_schema, tx.as_mut()).await?
        {
            predicate_object.triple_count += 1;
            predicate_object.upsert(backend_schema, tx.as_mut()).await?;
        } else {
            PredicateObject::builder()
                .id(id)
                .predicate_id(self.predicate_id()?)
                .object_id(self.object_id()?)
                .triple_count(1)
                .build()
                .upsert(backend_schema, tx.as_mut())
                .await?;
        }
        Ok(())
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
        if let Some(mut atom) = Atom::find_by_id(
            U256Wrapper::from(self.subject_id()?),
            backend_schema,
            tx.as_mut(),
        )
        .await?
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
