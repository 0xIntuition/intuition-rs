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
use log::info;
use models::{
    account::{Account, AccountType},
    atom::{Atom, AtomType},
    traits::SimpleCrud,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

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
    pub decoded_atom_data: String,
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
                self.process_account(resolver_consumer_context, account)
                    .await
            }
        }
    }

    /// This function processes an account message type
    async fn process_account(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        account: &Account,
    ) -> Result<(), ConsumerError> {
        let ens = Ens::get_ens(Address::from_str(&account.id)?, resolver_consumer_context).await?;
        if let Some(name) = ens.name.clone() {
            info!("ENS for account: {:?}", ens);
            Account::builder()
                .id(account.id.clone())
                .label(name.clone())
                .image(ens.image.unwrap_or_default())
                .account_type(AccountType::Default)
                .build()
                .upsert(&resolver_consumer_context.pg_pool)
                .await
                .map_err(ConsumerError::ModelError)?;
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
            &resolver_message.decoded_atom_data,
            resolver_consumer_context,
        )
        .await?;
        // If we resolved an IPFS URI, we need to try to parse the JSON
        let metadata = if let Some(data) = data {
            info!("Atom data is an IPFS URI: {data}");
            try_to_parse_json(
                &data,
                &resolver_message.atom,
                &resolver_consumer_context.pg_pool,
            )
            .await?
        } else {
            info!("No IPFS URI found, trying to parse atom data as JSON...");
            // This is the fallback case, where we try to parse the atom data as JSON
            // even if it's not a valid IPFS URI. This is useful for cases where the
            // atom data is a JSON object that is not a schema.org URL.
            try_to_parse_json(
                &resolver_message.decoded_atom_data,
                &resolver_message.atom,
                &resolver_consumer_context.pg_pool,
            )
            .await?
        };

        // If at this point we have an atom type that is not unknown (it means it changes it state),
        // we need to update the atom metadata
        if AtomType::from_str(&metadata.atom_type)? != AtomType::Unknown {
            let atom = Atom::find_by_id(
                resolver_message.atom.id.clone(),
                &resolver_consumer_context.pg_pool,
            )
            .await?;

            if let Some(mut atom) = atom {
                metadata
                    .update_atom_metadata(&mut atom, &resolver_consumer_context.pg_pool)
                    .await?;

                // If the atom has an image, we need to download it and classify it
                if let Some(image) = metadata.image {
                    // If we receive an image, we send it to the IPFS upload consumer
                    // to be classified and stored
                    info!("Sending image to IPFS upload consumer: {}", image);
                    resolver_consumer_context
                        .client
                        .send_message(serde_json::to_string(&IpfsUploadMessage { image })?)
                        .await?;
                }

                // Mark the atom as resolved
                resolver_message
                    .atom
                    .mark_as_resolved(&resolver_consumer_context.pg_pool)
                    .await?;
                info!("Updated atom metadata: {atom:?}");
            }
        } else {
            // Mark the atom as failed
            resolver_message
                .atom
                .mark_as_failed(&resolver_consumer_context.pg_pool)
                .await?;
        }
        Ok(())
    }
}

impl ResolverConsumerMessage {
    /// This function creates a new atom message
    pub fn new_atom(atom: Atom, decoded_atom_data: String) -> Self {
        Self {
            message: ResolverMessageType::Atom(Box::new(ResolveAtom {
                atom,
                decoded_atom_data,
            })),
        }
    }

    /// This function creates a new account message
    pub fn new_account(account: Account) -> Self {
        Self {
            message: ResolverMessageType::Account(account),
        }
    }
}
