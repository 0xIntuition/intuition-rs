use crate::{
    error::ConsumerError,
    schemas::types::DecodedMessage,
    EthMultiVault::{EthMultiVaultInstance, TripleCreated},
};
use alloy::{eips::BlockId, primitives::U256, providers::RootProvider, transports::http::Http};
use log::info;
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
use sqlx::PgPool;
use std::str::FromStr;

use super::utils::short_id;

impl TripleCreated {
    /// This function checks if the subject atom is an account and if the predicate and object atoms are a person or organization.
    /// If they are, it updates the account and atom with the label and image of the object atom.
    async fn check_and_update_account_predicate_object_claim_count(
        &self,
        pg_pool: &PgPool,
    ) -> Result<(), ConsumerError> {
        let (subject_atom, predicate_atom, object_atom) =
            self.get_subject_predicate_object_atoms(pg_pool).await?;

        if self.is_account_with_person_or_org(&subject_atom, &predicate_atom, &object_atom) {
            self.update_account(&subject_atom, &object_atom, pg_pool)
                .await?;
            self.update_atom(&object_atom, pg_pool).await?;
        }
        Ok(())
    }

    /// This function creates an `Event` for the `FeesTransferred` event
    async fn create_event(
        &self,
        event: &DecodedMessage,
        pg_pool: &PgPool,
    ) -> Result<Event, ConsumerError> {
        // Create the event
        Event::builder()
            .id(event.transaction_hash.clone())
            .event_type(EventType::TripleCreated)
            .triple_id(self.vaultID)
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
            .build()
            .upsert(pg_pool)
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function verifies if the creator account exists in our DB. If it does, it returns it.
    /// If it does not, it creates it.
    async fn get_or_create_creator_account(
        &self,
        pg_pool: &PgPool,
    ) -> Result<Account, ConsumerError> {
        Account::find_by_id(self.creator.to_string().to_lowercase(), pg_pool)
            .await?
            .unwrap_or_else(|| {
                Account::builder()
                    .id(self.creator.to_string().to_lowercase())
                    .label(short_id(&self.creator.to_string()))
                    .account_type(AccountType::Default)
                    .build()
            })
            .upsert(pg_pool)
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function gets or creates a triple
    async fn get_or_create_triple(
        &self,
        pg_pool: &PgPool,
        event: &DecodedMessage,
        counter_vault_id: U256,
    ) -> Result<Triple, ConsumerError> {
        let creator_account = self.get_or_create_creator_account(pg_pool).await?;

        let (subject_atom, predicate_atom, object_atom) =
            self.get_subject_predicate_object_atoms(pg_pool).await?;

        Triple::find_by_id(U256Wrapper::from(self.vaultID), pg_pool)
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
            .upsert(pg_pool)
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// This function verifies if the vault exists in our DB. If it does, it returns it.
    /// If it does not, it creates it.
    async fn get_or_create_vault(
        &self,
        pg_pool: &PgPool,
        id: U256,
        current_share_price: U256,
    ) -> Result<Vault, ConsumerError> {
        Vault::find_by_id(U256Wrapper::from_str(&id.to_string())?, pg_pool)
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
            .upsert(pg_pool)
            .await
            .map_err(ConsumerError::ModelError)
    }
    /// This function gets the subject, predicate and object atoms from the DB
    /// and returns them as a tuple of atoms. If any of the atoms are not found,
    /// it returns an error.
    async fn get_subject_predicate_object_atoms(
        &self,
        pg_pool: &PgPool,
    ) -> Result<(Atom, Atom, Atom), ConsumerError> {
        Ok((
            Atom::find_by_id(U256Wrapper::from(self.subjectId), pg_pool)
                .await?
                .ok_or(ConsumerError::SubjectAtomNotFound)?,
            Atom::find_by_id(U256Wrapper::from(self.predicateId), pg_pool)
                .await?
                .ok_or(ConsumerError::PredicateAtomNotFound)?,
            Atom::find_by_id(U256Wrapper::from(self.objectId), pg_pool)
                .await?
                .ok_or(ConsumerError::ObjectAtomNotFound)?,
        ))
    }

    /// This function handles an `TripleCreated` event.
    pub async fn handle_triple_creation(
        &self,
        pg_pool: &PgPool,
        web3: &EthMultiVaultInstance<Http<Client>, RootProvider<Http<Client>>>,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Handling triple creation: {self:#?}");

        // Update the counter vault current share price and get the triple
        let triple = self
            .update_vaults_current_share_price_and_get_triple(pg_pool, web3, event)
            .await?;

        // Update the predicate object
        self.update_predicate_object_triple_count(pg_pool).await?;

        // Update the positions
        self.update_positions(pg_pool, &triple).await?;

        // Create the event
        self.create_event(event, pg_pool).await?;
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
        pg_pool: &PgPool,
    ) -> Result<(), ConsumerError> {
        if let Some(mut account) =
            Account::find_by_id(subject_atom.data.to_lowercase(), pg_pool).await?
        {
            account.label = object_atom.label.clone().unwrap_or_default();
            account.image = object_atom.image.clone();
            account.upsert(pg_pool).await?;
            Ok(())
        } else {
            Err(ConsumerError::AccountNotFound)
        }
    }

    /// This function updates the atom with the label and image of the object atom.
    async fn update_atom(&self, object_atom: &Atom, pg_pool: &PgPool) -> Result<(), ConsumerError> {
        if let Some(mut atom) = Atom::find_by_id(U256Wrapper::from(self.subjectId), pg_pool).await?
        {
            atom.label = object_atom.label.clone();
            atom.image = object_atom.image.clone();
            atom.upsert(pg_pool).await?;
            Ok(())
        } else {
            Err(ConsumerError::AtomNotFound)
        }
    }

    /// This function updates the positions
    async fn update_positions(
        &self,
        pg_pool: &PgPool,
        triple: &Triple,
    ) -> Result<(), ConsumerError> {
        let positions =
            Position::find_by_vault_id(U256Wrapper::from(self.vaultID), pg_pool).await?;
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
                .upsert(pg_pool)
                .await?;

            // Update the predicate object claim count
            self.update_predicate_object_claim_count(pg_pool).await?;
        }

        self.check_and_update_account_predicate_object_claim_count(pg_pool)
            .await?;

        Ok(())
    }

    /// This function updates the predicate object claim count
    async fn update_predicate_object_claim_count(
        &self,
        pg_pool: &PgPool,
    ) -> Result<(), ConsumerError> {
        if let Some(mut predicate_object) =
            PredicateObject::find_by_id(format!("{}-{}", self.predicateId, self.objectId), pg_pool)
                .await?
        {
            predicate_object.claim_count += 1;
            predicate_object.upsert(pg_pool).await?;
        } else {
            PredicateObject::builder()
                .id(format!("{}-{}", self.predicateId, self.objectId))
                .predicate_id(self.predicateId)
                .object_id(self.objectId)
                .claim_count(1)
                .triple_count(1)
                .build()
                .upsert(pg_pool)
                .await?;
        }
        Ok(())
    }

    /// This function updates the predicate object triple count
    async fn update_predicate_object_triple_count(
        &self,
        pg_pool: &PgPool,
    ) -> Result<(), ConsumerError> {
        if let Some(mut predicate_object) =
            PredicateObject::find_by_id(format!("{}-{}", self.predicateId, self.objectId), pg_pool)
                .await?
        {
            predicate_object.triple_count += 1;
            predicate_object.upsert(pg_pool).await?;
        } else {
            PredicateObject::builder()
                .id(format!("{}-{}", self.predicateId, self.objectId))
                .predicate_id(self.predicateId)
                .object_id(self.objectId)
                .claim_count(0)
                .triple_count(1)
                .build()
                .upsert(pg_pool)
                .await?;
        }
        Ok(())
    }

    /// This function updates the vault and counter vault current share prices
    async fn update_vaults_current_share_price_and_get_triple(
        &self,
        pg_pool: &PgPool,
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

        println!(
            "vault_current_share_price: {:?}",
            vault_current_share_price._0
        );
        // Get or create the triple
        let triple = self
            .get_or_create_triple(pg_pool, event, counter_vault_id._0)
            .await?;

        println!("triple: {:?}", triple);
        // Get or update the vault
        self.get_or_create_vault(pg_pool, self.vaultID, vault_current_share_price._0)
            .await?;
        // Get or update the counter vault
        self.get_or_create_vault(
            pg_pool,
            counter_vault_id._0,
            counter_vault_current_share_price._0,
        )
        .await?;

        Ok(triple)
    }
}
