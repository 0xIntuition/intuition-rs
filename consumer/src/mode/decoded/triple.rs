use crate::{
    error::ConsumerError,
    mode::types::DecodedConsumerContext,
    schemas::types::DecodedMessage,
    EthMultiVault::{EthMultiVaultInstance, TripleCreated},
};
use alloy::{eips::BlockId, primitives::U256, providers::RootProvider, transports::http::Http};
use models::{
    account::{Account, AccountType},
    atom::{Atom, AtomType},
    claim::Claim,
    event::{Event, EventType},
    position::Position,
    predicate_object::PredicateObject,
    traits::SimpleCrud,
    triple::Triple,
    types::U256Wrapper,
    vault::Vault,
};
use reqwest::Client;
use std::str::FromStr;
use tracing::info;

use super::utils::short_id;

impl TripleCreated {
    /// This function checks if the subject atom is an account and if the predicate and object atoms are a person or organization.
    /// If they are, it updates the account and atom with the label and image of the object atom.
    async fn check_and_update_account_predicate_object_claim_count(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        let (subject_atom, predicate_atom, object_atom) = self
            .get_subject_predicate_object_atoms(decoded_consumer_context)
            .await?;

        if self.is_account_with_person_or_org(&subject_atom, &predicate_atom, &object_atom) {
            self.update_account(&subject_atom, &object_atom, decoded_consumer_context)
                .await?;
            self.update_atom(&object_atom, decoded_consumer_context)
                .await?;
        }
        Ok(())
    }

    /// This function creates an `Event` for the `FeesTransferred` event
    async fn create_event(
        &self,
        event: &DecodedMessage,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<Event, ConsumerError> {
        // Create the event
        Event::builder()
            .id(DecodedMessage::event_id(event))
            .event_type(EventType::TripleCreated)
            .triple_id(self.vaultID)
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

    /// This function verifies if the creator account exists in our DB. If it does, it returns it.
    /// If it does not, it creates it.
    async fn get_or_create_creator_account(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<Account, ConsumerError> {
        // First try to find existing account
        if let Some(account) = Account::find_by_id(
            self.creator.to_string(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        {
            return Ok(account);
        }

        // Only create new account if none exists
        Account::builder()
            .id(self.creator.to_string())
            .label(short_id(&self.creator.to_string()))
            .account_type(AccountType::Default)
            .build()
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function gets or creates a triple
    async fn get_or_create_triple(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        counter_vault_id: U256,
    ) -> Result<Triple, ConsumerError> {
        let creator_account = self
            .get_or_create_creator_account(decoded_consumer_context)
            .await?;

        let (subject_atom, predicate_atom, object_atom) = self
            .get_subject_predicate_object_atoms(decoded_consumer_context)
            .await?;

        Triple::find_by_id(
            U256Wrapper::from(self.vaultID),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .unwrap_or_else(|| {
            Triple::builder()
                .id(self.vaultID)
                .creator_id(creator_account.id)
                .subject_id(subject_atom.id.clone())
                .predicate_id(predicate_atom.id.clone())
                .object_id(object_atom.id.clone())
                .vault_id(U256Wrapper::from(self.vaultID))
                .counter_vault_id(U256Wrapper::from(counter_vault_id))
                .block_number(U256Wrapper::try_from(event.block_number).unwrap_or_default())
                .block_timestamp(event.block_timestamp)
                .transaction_hash(event.transaction_hash.clone())
                .build()
        })
        .upsert(
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await
        .map_err(ConsumerError::ModelError)
    }

    /// This function verifies if the vault exists in our DB. If it does, it returns it.
    /// If it does not, it creates it.
    async fn get_or_create_vault(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        id: U256,
        current_share_price: U256,
    ) -> Result<Vault, ConsumerError> {
        Vault::find_by_id(
            U256Wrapper::from_str(&id.to_string())?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .unwrap_or_else(|| {
            Vault::builder()
                .id(id)
                .triple_id(self.vaultID)
                .total_shares(U256Wrapper::from(U256::from(0)))
                .current_share_price(U256Wrapper::from(current_share_price))
                .position_count(0)
                .build()
        })
        .upsert(
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
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
        Ok((
            Atom::find_by_id(
                U256Wrapper::from(self.subjectId),
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await?
            .ok_or(ConsumerError::SubjectAtomNotFound)?,
            Atom::find_by_id(
                U256Wrapper::from(self.predicateId),
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await?
            .ok_or(ConsumerError::PredicateAtomNotFound)?,
            Atom::find_by_id(
                U256Wrapper::from(self.objectId),
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await?
            .ok_or(ConsumerError::ObjectAtomNotFound)?,
        ))
    }

    /// This function handles an `TripleCreated` event.
    pub async fn handle_triple_creation(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        web3: &EthMultiVaultInstance<Http<Client>, RootProvider<Http<Client>>>,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Handling triple creation: {self:#?}");

        // Update the counter vault current share price and get the triple
        let triple = self
            .update_vaults_current_share_price_and_get_triple(
                &decoded_consumer_context,
                web3,
                event,
            )
            .await?;

        // Update the predicate object
        self.update_predicate_object_triple_count(&decoded_consumer_context)
            .await?;

        // Update the positions
        self.update_positions(&decoded_consumer_context, &triple)
            .await?;

        // Create the event
        self.create_event(event, decoded_consumer_context).await?;
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
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        if let Some(mut account) = Account::find_by_id(
            subject_atom
                .data
                .clone()
                .ok_or(ConsumerError::AtomDataNotFound)?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        {
            account.label = object_atom.label.clone().unwrap_or_default();
            account.image = object_atom.image.clone();
            account
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;
            Ok(())
        } else {
            Err(ConsumerError::AccountNotFound)
        }
    }

    /// This function updates the atom with the label and image of the object atom.
    async fn update_atom(
        &self,
        object_atom: &Atom,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        if let Some(mut atom) = Atom::find_by_id(
            U256Wrapper::from(self.subjectId),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        {
            atom.label = object_atom.label.clone();
            atom.image = object_atom.image.clone();
            atom.upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await?;
            Ok(())
        } else {
            Err(ConsumerError::AtomNotFound)
        }
    }

    /// This function updates the positions
    async fn update_positions(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        triple: &Triple,
    ) -> Result<(), ConsumerError> {
        let positions = Position::find_by_vault_id(
            U256Wrapper::from(self.vaultID),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;
        for position in positions {
            Claim::builder()
                .id(format!("{}-{}", self.vaultID, position.account_id))
                .account_id(position.account_id.clone())
                .triple_id(self.vaultID)
                .subject_id(self.subjectId)
                .predicate_id(self.predicateId)
                .object_id(self.objectId)
                .vault_id(U256Wrapper::from(self.vaultID))
                .counter_vault_id(triple.counter_vault_id.clone())
                .shares(position.shares.clone())
                .counter_shares(position.shares)
                .build()
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;

            // Update the predicate object claim count
            self.update_predicate_object_claim_count(&decoded_consumer_context)
                .await?;
        }

        self.check_and_update_account_predicate_object_claim_count(&decoded_consumer_context)
            .await?;

        Ok(())
    }

    /// This function updates the predicate object claim count
    async fn update_predicate_object_claim_count(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        if let Some(mut predicate_object) = PredicateObject::find_by_id(
            format!("{}-{}", self.predicateId, self.objectId),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        {
            predicate_object.claim_count += 1;
            predicate_object
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;
        } else {
            PredicateObject::builder()
                .id(format!("{}-{}", self.predicateId, self.objectId))
                .predicate_id(self.predicateId)
                .object_id(self.objectId)
                .claim_count(1)
                .triple_count(1)
                .build()
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;
        }
        Ok(())
    }

    /// This function updates the predicate object triple count
    async fn update_predicate_object_triple_count(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        if let Some(mut predicate_object) = PredicateObject::find_by_id(
            format!("{}-{}", self.predicateId, self.objectId),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        {
            predicate_object.triple_count += 1;
            predicate_object
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;
        } else {
            PredicateObject::builder()
                .id(format!("{}-{}", self.predicateId, self.objectId))
                .predicate_id(self.predicateId)
                .object_id(self.objectId)
                .claim_count(0)
                .triple_count(1)
                .build()
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;
        }
        Ok(())
    }

    /// This function updates the vault and counter vault current share prices
    async fn update_vaults_current_share_price_and_get_triple(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        web3: &EthMultiVaultInstance<Http<Client>, RootProvider<Http<Client>>>,
        event: &DecodedMessage,
    ) -> Result<Triple, ConsumerError> {
        // Get the counter vault ID
        let counter_vault_id = web3.getCounterIdFromTriple(self.vaultID).call().await?;
        // Get the share price of the atom
        // Get the current share price of the counter vault
        let counter_vault_current_share_price = web3
            .currentSharePrice(counter_vault_id._0)
            .block(BlockId::from_str(&event.block_number.to_string())?)
            .call()
            .await?;

        // Get the current share price of the vault
        let vault_current_share_price = web3
            .currentSharePrice(self.vaultID)
            .block(BlockId::from_str(&event.block_number.to_string())?)
            .call()
            .await?;

        // Get or create the triple
        let triple = self
            .get_or_create_triple(decoded_consumer_context, event, counter_vault_id._0)
            .await?;

        // Get or update the vault
        self.get_or_create_vault(
            decoded_consumer_context,
            self.vaultID,
            vault_current_share_price._0,
        )
        .await?;
        // Get or update the counter vault
        self.get_or_create_vault(
            decoded_consumer_context,
            counter_vault_id._0,
            counter_vault_current_share_price._0,
        )
        .await?;

        Ok(triple)
    }
}
