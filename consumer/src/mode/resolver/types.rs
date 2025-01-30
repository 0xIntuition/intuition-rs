use crate::{
    error::ConsumerError,
    mode::{
        ipfs_upload::types::IpfsUploadMessage,
        resolver::{
            atom_resolver::{try_to_parse_json, try_to_resolve_ipfs_uri},
            ens_resolver::Ens,
        },
        types::ResolverConsumerContext,
    },
};
use alloy::primitives::Address;
use models::{
    account::Account,
    atom::{Atom, AtomType},
    traits::SimpleCrud,
    types::U256Wrapper,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::info;
/// This struct represents a message that is sent to the resolver
/// consumer to be processed.
#[derive(Debug, Serialize, Deserialize)]
pub struct ResolverConsumerMessage {
    pub message: ResolverMessageType,
}

/// This struct represents an atom message that is sent to the resolver
/// consumer
#[derive(Debug, Serialize, Deserialize)]
pub struct ResolveAtom {
    pub atom: Atom,
}

/// This enum represents the possible types of messages that can be sent to the
/// resolver consumer
#[derive(Debug, Serialize, Deserialize)]
pub enum ResolverMessageType {
    Atom(Box<ResolveAtom>),
    Account(Account),
}

impl ResolverMessageType {
    /// This function processes a resolver message according to its type
    pub async fn process(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
    ) -> Result<(), ConsumerError> {
        match self {
            ResolverMessageType::Atom(resolver_message) => {
                info!("Processing a resolved message: {resolver_message:?}");
                self.process_atom(resolver_consumer_context, resolver_message)
                    .await
            }
            ResolverMessageType::Account(account) => {
                info!("Processing a resolved account: {account:?}");
                self.process_account(resolver_consumer_context, &mut account.clone())
                    .await
            }
        }
    }

    /// This function processes an account message type
    async fn process_account(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        account: &mut Account,
    ) -> Result<(), ConsumerError> {
        let ens = Ens::get_ens(Address::from_str(&account.id)?, resolver_consumer_context).await?;
        if let Some(_name) = ens.name.clone() {
            info!("ENS for account: {:?}", ens);
            // We need to update the account metadata
            self.update_account_metadata(
                resolver_consumer_context,
                account.id.clone(),
                ens.clone(),
            )
            .await?;
            // We also need to update the atom
            if let Some(atom_id) = account.atom_id.clone() {
                self.update_atom_metadata(resolver_consumer_context, &atom_id, ens)
                    .await?;
            } else {
                // We deal with the case where the account atom_id was not set
                // when the account was created. In this case, we need to query the DB
                // to find the atom_id, as this update happens in another consumer
                info!(
                    "No atom found for account: {:?}, querying the DB...",
                    account
                );
                let account = Account::find_by_id(
                    account.id.clone(),
                    &resolver_consumer_context.pg_pool,
                    &resolver_consumer_context
                        .server_initialize
                        .env
                        .backend_schema,
                )
                .await?
                .ok_or(ConsumerError::AccountNotFound)?;
                if let Some(atom_id) = account.atom_id {
                    self.update_atom_metadata(resolver_consumer_context, &atom_id, ens)
                        .await?;
                } else {
                    info!("No atom found for account: {:?}", account);
                }
            }
        } else {
            info!("No ENS found for account: {:?}", account);
        }
        Ok(())
    }

    /// This function processes an atom message type
    async fn process_atom(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        resolver_message: &ResolveAtom,
    ) -> Result<(), ConsumerError> {
        let data = try_to_resolve_ipfs_uri(
            &resolver_message
                .atom
                .data
                .clone()
                .ok_or(ConsumerError::AtomDataNotFound)?,
            resolver_consumer_context,
        )
        .await?;
        // If we resolved an IPFS URI, we need to try to parse the JSON
        let metadata = if let Some(data) = data {
            // At this point we know that the data is a valid response
            // so we can try to parse the JSON. We also need to remove the UTF-8 BOM
            // if present, as it can cause issues with the JSON parsing.
            let data = data.text().await?.replace('\u{feff}', "");
            let _bytes = data.bytes();
            info!("Atom data is an IPFS URI: {data}");
            try_to_parse_json(&data, &resolver_message.atom, resolver_consumer_context).await?
        } else {
            info!(
                "No IPFS URI found or IPFS URI is not valid, trying to parse atom data as JSON..."
            );
            // This is the fallback case, where we try to parse the atom data as JSON
            // even if it's not a valid IPFS URI. This is useful for cases where the
            // atom data is a JSON object that is not a schema.org URL.
            try_to_parse_json(
                &resolver_message
                    .atom
                    .data
                    .clone()
                    .ok_or(ConsumerError::AtomDataNotFound)?,
                &resolver_message.atom,
                resolver_consumer_context,
            )
            .await?
        };

        // If at this point we have an atom type that is not unknown (it means it changes it state),
        // we need to update the atom metadata
        if AtomType::from_str(&metadata.atom_type)? != AtomType::Unknown {
            let atom = Atom::find_by_id(
                resolver_message.atom.id.clone(),
                &resolver_consumer_context.pg_pool,
                &resolver_consumer_context
                    .server_initialize
                    .env
                    .backend_schema,
            )
            .await?;

            if let Some(mut atom) = atom {
                metadata
                    .update_atom_metadata(
                        &mut atom,
                        &resolver_consumer_context.pg_pool,
                        &resolver_consumer_context
                            .server_initialize
                            .env
                            .backend_schema,
                    )
                    .await?;

                // If the atom has an image, we need to download it and classify it
                if let Some(image) = metadata.image {
                    // If we receive an image, we send it to the IPFS upload consumer
                    // to be classified and stored
                    info!("Sending image to IPFS upload consumer: {}", image);
                    resolver_consumer_context
                        .client
                        .send_message(serde_json::to_string(&IpfsUploadMessage { image })?, None)
                        .await?;
                }

                // Mark the atom as resolved
                resolver_message
                    .atom
                    .mark_as_resolved(
                        &resolver_consumer_context.pg_pool,
                        &resolver_consumer_context
                            .server_initialize
                            .env
                            .backend_schema,
                    )
                    .await?;
                info!("Updated atom metadata: {atom:?}");
            }
        } else {
            // Mark the atom as failed
            resolver_message
                .atom
                .mark_as_failed(
                    &resolver_consumer_context.pg_pool,
                    &resolver_consumer_context
                        .server_initialize
                        .env
                        .backend_schema,
                )
                .await?;
        }
        Ok(())
    }

    /// This function updates the account metadata
    async fn update_account_metadata(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        account_id: String,
        ens: Ens,
    ) -> Result<(), ConsumerError> {
        let mut account = Account::find_by_id(
            account_id,
            &resolver_consumer_context.pg_pool,
            &resolver_consumer_context
                .server_initialize
                .env
                .backend_schema,
        )
        .await?
        .ok_or(ConsumerError::AccountNotFound)?;
        account.label = ens.name.ok_or(ConsumerError::LabelNotFound)?;
        account.image = ens.image;
        account
            .upsert(
                &resolver_consumer_context.pg_pool,
                &resolver_consumer_context
                    .server_initialize
                    .env
                    .backend_schema,
            )
            .await?;
        Ok(())
    }

    /// This function updates the atom metadata
    async fn update_atom_metadata(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        atom_id: &U256Wrapper,
        ens: Ens,
    ) -> Result<(), ConsumerError> {
        let mut atom = Atom::find_by_id(
            atom_id.clone(),
            &resolver_consumer_context.pg_pool,
            &resolver_consumer_context
                .server_initialize
                .env
                .backend_schema,
        )
        .await?
        .ok_or(ConsumerError::AtomNotFound)?;
        atom.label = ens.name;
        atom.image = ens.image;
        atom.upsert(
            &resolver_consumer_context.pg_pool,
            &resolver_consumer_context
                .server_initialize
                .env
                .backend_schema,
        )
        .await?;
        Ok(())
    }
}

impl ResolverConsumerMessage {
    /// This function creates a new atom message
    pub fn new_atom(atom: Atom) -> Self {
        Self {
            message: ResolverMessageType::Atom(Box::new(ResolveAtom { atom })),
        }
    }

    /// This function creates a new account message
    pub fn new_account(account: Account) -> Self {
        Self {
            message: ResolverMessageType::Account(account),
        }
    }
}
