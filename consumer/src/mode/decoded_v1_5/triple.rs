use crate::{
    EthMultiVault::TripleCreated,
    error::ConsumerError,
    mode::{resolver::types::ResolverConsumerMessage, types::DecodedConsumerContext},
    schemas::types::DecodedMessage,
    traits::{SharePriceEvent, VaultManager},
};
use async_trait::async_trait;
use models::{
    account::{Account, AccountType},
    atom::{Atom, AtomResolvingStatus, AtomType},
    claim::Claim,
    event::{Event, EventType},
    position::Position,
    predicate_object::PredicateObject,
    share_price_changed_curve::SharePriceChangedCurve,
    term::TermType,
    traits::SimpleCrud,
    triple::Triple,
    types::U256Wrapper,
    vault::Vault,
};
use std::str::FromStr;
use tracing::info;

use super::utils::{get_or_create_term, get_or_create_vault, short_id};

/// This impl is used to convert the `TripleCreated` event into a `SharePriceEvent`
/// and we can use the general share price change logic for this. We need this because
/// we may need to create new vaults while handling the `TripleCreated` event.
#[async_trait]
impl SharePriceEvent for &TripleCreated {
    fn total_assets(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from_str("0")?)
    }

    fn new_share_price(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from_str("0")?)
    }
}

/// This impl is used to convert the `AtomCreated` event into a `VaultManager`
/// and we can use the general vault creation logic for this.
#[async_trait]
impl VaultManager for &TripleCreated {
    fn term_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.vaultId))
    }

    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError> {
        U256Wrapper::from_str("1").map_err(ConsumerError::ModelError)
    }

    async fn total_shares(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(SharePriceChangedCurve::fetch_current_share_price(
            U256Wrapper::from(self.vaultId),
            U256Wrapper::from_str("1")?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .total_shares)
    }

    async fn current_share_price(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(SharePriceChangedCurve::fetch_current_share_price(
            U256Wrapper::from(self.vaultId),
            U256Wrapper::from_str("1")?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .share_price)
    }

    async fn position_count(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<i32, ConsumerError> {
        Ok(Position::count_by_vault_and_curve(
            self.vaultId.to_string(),
            "1".to_string(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await? as i32)
    }
}

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
            .triple_id(self.vaultId)
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
    ) -> Result<Triple, ConsumerError> {
        // Get the counter vault ID
        let counter_vault_id = decoded_consumer_context
            .get_counter_id_from_triple(self.vaultId)
            .await?;

        let creator_account = self
            .get_or_create_creator_account(decoded_consumer_context)
            .await?;

        let (subject_atom, predicate_atom, object_atom) = self
            .get_subject_predicate_object_atoms(decoded_consumer_context)
            .await?;

        Triple::find_by_id(
            U256Wrapper::from(self.vaultId),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .unwrap_or_else(|| {
            Triple::builder()
                .creator_id(creator_account.id)
                .subject_id(subject_atom.term_id.clone())
                .predicate_id(predicate_atom.term_id.clone())
                .object_id(object_atom.term_id.clone())
                .term_id(U256Wrapper::from(self.vaultId))
                .counter_term_id(U256Wrapper::from(counter_vault_id))
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

    /// This function fetches an atom or creates it
    async fn fetch_or_create_temporary_atom(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        id: U256Wrapper,
    ) -> Result<Atom, ConsumerError> {
        if let Some(atom) = self.find_atom(decoded_consumer_context, &id).await? {
            return Ok(atom);
        }

        let atom_data = decoded_consumer_context
            .fetch_atom_data(self.subjectId)
            .await?;

        let account = self
            .get_or_create_temporary_account(decoded_consumer_context)
            .await?;
        let vault = get_or_create_vault(self, decoded_consumer_context, TermType::Triple).await?;

        let atom = self
            .create_atom(
                decoded_consumer_context,
                atom_data.to_string(),
                account,
                vault,
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

    /// This function finds an atom
    async fn find_atom(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        id: &U256Wrapper,
    ) -> Result<Option<Atom>, ConsumerError> {
        Atom::find_by_id(
            id.clone(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await
        .map_err(ConsumerError::ModelError)
    }

    /// This function gets or creates an account
    async fn get_or_create_temporary_account(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<Account, ConsumerError> {
        if let Some(account) = Account::find_by_id(
            "0x0000000000000000000000000000000000000000".to_string(),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
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
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await
                .map_err(ConsumerError::ModelError)
        }
    }

    /// This function creates an atom
    async fn create_atom(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        atom_data: String,
        account: Account,
        vault: Vault,
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
            .build()
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
        let subject_atom = self
            .fetch_or_create_temporary_atom(
                decoded_consumer_context,
                U256Wrapper::from(self.subjectId),
            )
            .await?;
        let predicate_atom = self
            .fetch_or_create_temporary_atom(
                decoded_consumer_context,
                U256Wrapper::from(self.predicateId),
            )
            .await?;
        let object_atom = self
            .fetch_or_create_temporary_atom(
                decoded_consumer_context,
                U256Wrapper::from(self.objectId),
            )
            .await?;
        Ok((subject_atom, predicate_atom, object_atom))
    }

    /// This function updates the vault and counter vault current share prices
    async fn get_or_create_vaults(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        // Get the counter vault ID
        let counter_vault_id = decoded_consumer_context
            .get_counter_id_from_triple(self.vaultId)
            .await?;

        // Get or update the vault
        get_or_create_vault(self, decoded_consumer_context, TermType::Triple).await?;
        // Get or update the counter vault
        self.get_or_create_counter_vault(
            U256Wrapper::from(counter_vault_id),
            decoded_consumer_context,
        )
        .await?;

        Ok(())
    }

    /// This function gets or creates a counter vault
    pub async fn get_or_create_counter_vault(
        &self,
        counter_vault_id: U256Wrapper,
        decoded_consumer_context: &DecodedConsumerContext,
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
                &self,
                Some(counter_vault_id),
                decoded_consumer_context,
                TermType::Triple,
            )
            .await?;

            let new_vault = Vault::builder()
                .term_id(U256Wrapper::from(self.vaultId))
                .curve_id(U256Wrapper::from_str("1")?)
                .current_share_price(self.current_share_price(decoded_consumer_context).await?)
                .total_shares(self.total_shares(decoded_consumer_context).await?)
                .position_count(0)
                .build()
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await
                .map_err(ConsumerError::ModelError)?;

            Ok(new_vault)
        }
    }

    /// This function handles an `TripleCreated` event.
    pub async fn handle_triple_creation(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Handling triple creation: {self:#?}");

        // Ensure that the vault and counter vault exist
        self.get_or_create_vaults(decoded_consumer_context).await?;

        // Get or create the triple
        let triple = self
            .get_or_create_triple(decoded_consumer_context, event)
            .await?;

        info!("Triple created: {triple:#?}");
        // Update the predicate object
        self.update_predicate_object_triple_count(decoded_consumer_context)
            .await?;
        info!("Predicate object triple count updated");

        // Update the positions
        self.update_positions(decoded_consumer_context).await?;
        info!("Positions updated");
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
    ) -> Result<(), ConsumerError> {
        let positions = Position::find_by_vault_id(
            format!("{}-1-{}", self.vaultId, self.subjectId),
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;
        for position in positions {
            Claim::builder()
                .id(format!("{}-{}", self.vaultId, position.account_id))
                .account_id(position.account_id.clone())
                .position_id(position.id.clone())
                .build()
                .upsert(
                    &decoded_consumer_context.pg_pool,
                    &decoded_consumer_context.backend_schema,
                )
                .await?;

            // Update the predicate object claim count
            self.update_predicate_object_claim_count(decoded_consumer_context)
                .await?;
        }

        self.check_and_update_account_predicate_object_claim_count(decoded_consumer_context)
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
}
