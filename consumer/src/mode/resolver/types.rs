#[cfg(feature = "v1_0_contract")]
use crate::mode::decoded::atom::atom_supported_types::AtomMetadata;
#[cfg(feature = "v1_5_contract")]
use crate::mode::decoded_v1_5::atom::atom_supported_types::AtomMetadata;
use crate::{
    error::ConsumerError,
    mode::{
        ipfs_upload::types::IpfsUploadMessage,
        resolver::{
            atom_resolver::{
                handle_binary_data, try_to_parse_json_or_text, try_to_resolve_ipfs_uri,
            },
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
    Atom(String),
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
        atom_id: &str,
    ) -> Result<(), ConsumerError> {
        let metadata = self
            .resolve_and_parse_atom_data(resolver_consumer_context, atom_id)
            .await?;

        // If the atom type is not unknown, we handle the new atom type that was resolved
        if AtomType::from_str(&metadata.atom_type)? != AtomType::Unknown {
            self.handle_known_atom_type(resolver_consumer_context, atom_id, metadata)
                .await?;
        } else {
            self.mark_atom_as_failed(resolver_consumer_context, &U256Wrapper::from_str(atom_id)?)
                .await?;
        }
        Ok(())
    }

    /// This function resolves and parses the atom data
    async fn resolve_and_parse_atom_data(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        atom_id: &str,
    ) -> Result<AtomMetadata, ConsumerError> {
        let atom = Atom::find_by_id(
            U256Wrapper::from_str(atom_id)?,
            &resolver_consumer_context.pg_pool,
            &resolver_consumer_context
                .server_initialize
                .env
                .backend_schema,
        )
        .await?
        .ok_or(ConsumerError::AtomDataNotFound)?;

        // We check if the atom data is an IPFS URI and if it is, we fetch the data from the IPFS node
        let data = try_to_resolve_ipfs_uri(
            &atom.clone().data.ok_or(ConsumerError::AtomDataNotFound)?,
            resolver_consumer_context,
        )
        .await?;
        // let text = data.text().await;
        // let bytes = data.bytes().await;

        // This is the case where we receive a response from the IPFS node, but we dont know yet
        // if the response is a JSON or a binary file.
        if let Some(data) = data {
            info!("Atom data is an IPFS URI and we have a response from the IPFS node");
            // First we try to decode the response as bytes
            match data.bytes().await {
                Ok(bytes) => {
                    // Try to convert bytes to text
                    match String::from_utf8(bytes.to_vec()) {
                        Ok(text) => {
                            info!("Trying to get text from {}", text);
                            let data = text.replace('\u{feff}', "");
                            try_to_parse_json_or_text(&data, &atom, resolver_consumer_context).await
                        }
                        Err(_) => {
                            info!("Failed to parse as text, trying to parse atom data as Binary");
                            handle_binary_data(resolver_consumer_context, &atom, bytes).await
                        }
                    }
                }
                Err(e) => {
                    info!("Failed to get bytes from IPFS response: {e}");
                    Err(ConsumerError::FailedToGetBytes)
                }
            }
        // This is the case where the atom data is not an IPFS URI, so we try to parse it as JSON
        } else {
            info!(
                "No IPFS URI found or IPFS URI is not valid, trying to parse atom data as JSON or text..."
            );
            try_to_parse_json_or_text(
                &atom.clone().data.ok_or(ConsumerError::AtomDataNotFound)?,
                &atom,
                resolver_consumer_context,
            )
            .await
        }
    }

    /// This function handles the known atom type
    async fn handle_known_atom_type(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        atom_id: &str,
        metadata: AtomMetadata,
    ) -> Result<(), ConsumerError> {
        let atom = self
            .find_and_update_atom(resolver_consumer_context, atom_id, metadata)
            .await?;

        if let Some(image) = atom.image.clone() {
            self.handle_atom_image(resolver_consumer_context, image)
                .await?;
        }

        atom.mark_as_resolved(
            &resolver_consumer_context.pg_pool,
            &resolver_consumer_context
                .server_initialize
                .env
                .backend_schema,
        )
        .await?;

        info!("Updated atom metadata: {atom:?}");
        Ok(())
    }

    /// This function finds the atom and updates its metadata
    async fn find_and_update_atom(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        atom_id: &str,
        metadata: AtomMetadata,
    ) -> Result<Atom, ConsumerError> {
        let atom = Atom::find_by_id(
            U256Wrapper::from_str(atom_id)?,
            &resolver_consumer_context.pg_pool,
            &resolver_consumer_context
                .server_initialize
                .env
                .backend_schema,
        )
        .await?
        .ok_or(ConsumerError::AtomNotFound)?;

        let mut atom = atom;
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

        Ok(atom)
    }

    /// This function handles the atom image
    async fn handle_atom_image(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        image: String,
    ) -> Result<(), ConsumerError> {
        info!("Sending image to IPFS upload consumer: {}", image);
        resolver_consumer_context
            .client
            .send_message(serde_json::to_string(&IpfsUploadMessage { image })?, None)
            .await
    }

    /// This function marks the atom as failed
    async fn mark_atom_as_failed(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        atom_id: &U256Wrapper,
    ) -> Result<(), ConsumerError> {
        let atom = Atom::find_by_id(
            atom_id.clone(),
            &resolver_consumer_context.pg_pool,
            &resolver_consumer_context
                .server_initialize
                .env
                .backend_schema,
        )
        .await?
        .ok_or(ConsumerError::AtomNotFound)?;
        atom.mark_as_failed(
            &resolver_consumer_context.pg_pool,
            &resolver_consumer_context
                .server_initialize
                .env
                .backend_schema,
        )
        .await
        .map_err(ConsumerError::from)
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
    pub fn new_atom(atom_id: String) -> Self {
        Self {
            message: ResolverMessageType::Atom(atom_id),
        }
    }

    /// This function creates a new account message
    pub fn new_account(account: Account) -> Self {
        Self {
            message: ResolverMessageType::Account(account),
        }
    }
}
