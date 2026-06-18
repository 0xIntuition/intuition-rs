use crate::{
    error::ConsumerError,
    mode::{
        ipfs_upload::types::IpfsUploadMessage,
        metadata::AtomMetadata,
        resolver::{
            atom_resolver::{
                handle_binary_data, try_to_parse_json_or_text, try_to_resolve_ipfs_uri,
            },
            ens_resolver::Ens,
            tns_resolver::Tns,
        },
        types::ResolverConsumerContext,
    },
    traits::AtomUpdater,
};
use alloy::primitives::Address;
use models::{
    account::Account,
    atom::{Atom, AtomType},
    traits::SimpleCrud,
    types::FixedBytesWrapper,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{debug, warn};

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
            ResolverMessageType::Atom(atom_id) => {
                debug!("Processing a resolved atom: {atom_id}");
                self.process_atom_message(resolver_consumer_context, atom_id)
                    .await
            }
            ResolverMessageType::Account(account) => {
                debug!("Processing a resolved account: {account:?}");
                self.process_account(resolver_consumer_context, &mut account.clone())
                    .await
            }
        }
    }

    /// Processes an atom message by determining if it's an account, CAIP-22, or regular atom
    async fn process_atom_message(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        atom_id: &str,
    ) -> Result<(), ConsumerError> {
        let atom = self
            .fetch_atom_by_id(resolver_consumer_context, atom_id)
            .await?;

        match atom.atom_type {
            AtomType::Account => {
                self.process_account_atom(resolver_consumer_context, &atom)
                    .await
            }
            AtomType::Caip22 => {
                debug!("Atom is a CAIP-22, resolving NFT metadata");
                self.process_caip22_atom(resolver_consumer_context, &atom)
                    .await
            }
            _ => {
                debug!("Atom is not an account or CAIP-22, processing as regular atom");
                self.process_atom(resolver_consumer_context, atom_id).await
            }
        }
    }

    /// Processes a CAIP-22 atom by resolving its tokenURI and metadata
    async fn process_caip22_atom(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        atom: &Atom,
    ) -> Result<(), ConsumerError> {
        use crate::mode::resolver::caip22_resolver::resolve_caip22;

        // Resolve the CAIP-22 (parses from atom.data, fetches tokenURI, stores as json_object)
        let metadata = resolve_caip22(atom, resolver_consumer_context).await?;

        // Update atom with resolved metadata (label, image, atom_type)
        let mut atom = atom.clone();
        metadata
            .update_atom_metadata(
                &mut atom,
                resolver_consumer_context.backend_schema(),
                resolver_consumer_context.pool(),
            )
            .await?;

        // Mark as resolved
        atom.mark_as_resolved(
            resolver_consumer_context.backend_schema(),
            resolver_consumer_context.pool(),
        )
        .await?;

        debug!("Successfully resolved CAIP-22 atom: {:?}", atom);
        Ok(())
    }

    /// Fetches an atom by its ID from the database
    async fn fetch_atom_by_id(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        atom_id: &str,
    ) -> Result<Atom, ConsumerError> {
        Atom::find_by_id(
            FixedBytesWrapper::from_str(atom_id)?,
            &resolver_consumer_context
                .server_initialize
                .env
                .backend_schema,
            &resolver_consumer_context.pg_pool,
        )
        .await?
        .ok_or(ConsumerError::AtomNotFound)
    }

    /// Processes an atom that represents an account (updates ENS)
    async fn process_account_atom(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        atom: &Atom,
    ) -> Result<(), ConsumerError> {
        debug!("Atom is an account, updating ENS for the account");

        let account_data = atom.data.clone().ok_or_else(|| {
            debug!("No data found for atom: {:?}", atom);
            ConsumerError::AtomDataNotFound
        })?;

        let mut account = self
            .fetch_account_by_atom_data(resolver_consumer_context, &account_data)
            .await?;
        debug!("Account found for atom: {:?}", account);

        self.process_account(resolver_consumer_context, &mut account)
            .await
    }

    /// Fetches an account by its atom data from the database
    async fn fetch_account_by_atom_data(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        account_data: &str,
    ) -> Result<Account, ConsumerError> {
        Account::find_by_id(
            account_data.to_string(),
            &resolver_consumer_context
                .server_initialize
                .env
                .backend_schema,
            &resolver_consumer_context.pg_pool,
        )
        .await?
        .ok_or(ConsumerError::AccountNotFound)
    }

    /// This function processes an account message type.
    /// TNS resolution is attempted first. If no TNS name is found,
    /// falls back to ENS resolution.
    async fn process_account(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        account: &mut Account,
    ) -> Result<(), ConsumerError> {
        let address = Address::from_str(&account.id)?;

        // Try TNS first (priority)
        let resolved = match Tns::reverse_resolve(address, &resolver_consumer_context.tns_client)
            .await
        {
            Ok(tns) if tns.name.is_some() => {
                debug!("TNS resolved for account: {:?}", tns);
                // Send avatar to IPFS upload consumer for image processing
                if let Some(ref avatar_url) = tns.avatar {
                    debug!("Sending TNS avatar to IPFS upload consumer: {}", avatar_url);
                    if let Err(e) = resolver_consumer_context
                        .client
                        .send_message(
                            serde_json::to_string(&IpfsUploadMessage {
                                image: avatar_url.clone(),
                            })?,
                            None,
                        )
                        .await
                    {
                        warn!("Failed to send TNS avatar to IPFS upload consumer: {e}");
                    }
                }
                Ens {
                    name: tns.name,
                    image: tns.avatar,
                }
            }
            Ok(_) => {
                debug!("No TNS name found, falling back to ENS");
                Ens::get_ens(address, resolver_consumer_context).await?
            }
            Err(e) => {
                debug!("TNS resolution failed: {e}, falling back to ENS");
                Ens::get_ens(address, resolver_consumer_context).await?
            }
        };

        if let Some(_name) = resolved.name.clone() {
            debug!("Updating account metadata for account: {:?}", account);
            self.update_account_metadata(
                resolver_consumer_context,
                account.id.clone(),
                resolved.clone(),
            )
            .await?;
            if let Some(atom_id) = account.atom_id.clone() {
                debug!("Updating atom metadata for account: {:?}", account);
                self.update_atom_metadata(resolver_consumer_context, &atom_id, resolved)
                    .await?;
            } else {
                debug!("No atom found for account: {:?}", account)
            }
        } else {
            debug!("No name resolved (TNS or ENS) for account: {:?}", account);
        }
        Ok(())
    }

    /// This function processes an atom message type.
    ///
    /// Network I/O (IPFS fetches, etc.) is performed before any DB writes
    /// to avoid holding database connections during slow external requests.
    async fn process_atom(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        atom_id: &str,
    ) -> Result<(), ConsumerError> {
        // Phase 1: Fetch atom and resolve data (includes network I/O like IPFS)
        // No transaction needed — this avoids holding a DB connection during slow fetches
        let metadata = self
            .resolve_and_parse_atom_data(resolver_consumer_context, atom_id)
            .await?;
        debug!("Metadata: {:?}", metadata);

        // Phase 2: DB writes only (fast path)
        if AtomType::from_str(&metadata.atom_type)? != AtomType::Unknown {
            debug!("Handling known atom type: {:?}", metadata);
            self.handle_known_atom_type(resolver_consumer_context, atom_id, metadata)
                .await?;
        } else {
            self.mark_atom_as_failed(
                &resolver_consumer_context
                    .server_initialize
                    .env
                    .backend_schema,
                &resolver_consumer_context.pg_pool,
                &FixedBytesWrapper::from_str(atom_id)?,
            )
            .await?;
        }
        Ok(())
    }

    /// This function resolves and parses the atom data.
    ///
    /// Uses the connection pool for the initial read and performs all network I/O
    /// (IPFS fetches) without holding a transaction open.
    async fn resolve_and_parse_atom_data(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        atom_id: &str,
    ) -> Result<AtomMetadata, ConsumerError> {
        let atom = Atom::find_by_id(
            FixedBytesWrapper::from_str(atom_id)?,
            &resolver_consumer_context
                .server_initialize
                .env
                .backend_schema,
            &resolver_consumer_context.pg_pool,
        )
        .await?
        .ok_or(ConsumerError::AtomDataNotFound)?;

        // We check if the atom data is an IPFS URI and if it is, we fetch the data from the IPFS node
        let data = try_to_resolve_ipfs_uri(
            &atom.clone().data.ok_or(ConsumerError::AtomDataNotFound)?,
            resolver_consumer_context,
        )
        .await?;

        // This is the case where we receive a response from the IPFS node, but we dont know yet
        // if the response is a JSON or a binary file.
        if let Some(data) = data {
            debug!("Atom data is an IPFS URI and we have a response from the IPFS node");
            // First we try to decode the response as bytes
            match data.bytes().await {
                Ok(bytes) => {
                    // Try to convert bytes to text
                    match String::from_utf8(bytes.to_vec()) {
                        Ok(text) => {
                            debug!("Trying to get text from {}", text);
                            let data = text.replace('\u{feff}', "");
                            try_to_parse_json_or_text(&data, &atom, resolver_consumer_context).await
                        }
                        Err(_) => {
                            debug!("Failed to parse as text, trying to parse atom data as Binary");
                            handle_binary_data(resolver_consumer_context, &atom, bytes).await
                        }
                    }
                }
                Err(e) => {
                    debug!("Failed to get bytes from IPFS response: {e}");
                    Err(ConsumerError::FailedToGetBytes)
                }
            }
        // This is the case where the atom data is not an IPFS URI, so we try to parse it as JSON
        } else {
            debug!(
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
            &resolver_consumer_context
                .server_initialize
                .env
                .backend_schema,
            &resolver_consumer_context.pg_pool,
        )
        .await?;

        debug!("Updated atom metadata: {atom:?}");
        Ok(())
    }

    /// This function finds the atom and updates its metadata
    async fn find_and_update_atom(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        atom_id: &str,
        metadata: AtomMetadata,
    ) -> Result<Atom, ConsumerError> {
        let mut atom = Atom::find_by_id(
            FixedBytesWrapper::from_str(atom_id)?,
            &resolver_consumer_context
                .server_initialize
                .env
                .backend_schema,
            &resolver_consumer_context.pg_pool,
        )
        .await?
        .ok_or(ConsumerError::AtomNotFound)?;
        metadata
            .update_atom_metadata(
                &mut atom,
                &resolver_consumer_context
                    .server_initialize
                    .env
                    .backend_schema,
                &resolver_consumer_context.pg_pool,
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
        debug!("Sending image to IPFS upload consumer: {}", image);
        resolver_consumer_context
            .client
            .send_message(serde_json::to_string(&IpfsUploadMessage { image })?, None)
            .await
    }

    /// This function marks the atom as failed
    async fn mark_atom_as_failed(
        &self,
        backend_schema: &str,
        pg_pool: &sqlx::PgPool,
        atom_id: &FixedBytesWrapper,
    ) -> Result<(), ConsumerError> {
        let atom = Atom::find_by_id(atom_id.clone(), backend_schema, pg_pool)
            .await?
            .ok_or(ConsumerError::AtomNotFound)?;
        atom.mark_as_failed(backend_schema, pg_pool)
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
        let backend_schema = &resolver_consumer_context
            .server_initialize
            .env
            .backend_schema;
        let mut account = Account::find_by_id(
            account_id,
            backend_schema,
            &resolver_consumer_context.pg_pool,
        )
        .await?
        .ok_or(ConsumerError::AccountNotFound)?;
        account.label = ens.name.ok_or(ConsumerError::LabelNotFound)?;
        account.image = ens.image;
        account
            .upsert(backend_schema, &resolver_consumer_context.pg_pool)
            .await?;
        Ok(())
    }

    /// This function updates the atom metadata
    async fn update_atom_metadata(
        &self,
        resolver_consumer_context: &ResolverConsumerContext,
        atom_id: &FixedBytesWrapper,
        ens: Ens,
    ) -> Result<(), ConsumerError> {
        let backend_schema = &resolver_consumer_context
            .server_initialize
            .env
            .backend_schema;
        let mut atom = Atom::find_by_id(
            atom_id.clone(),
            backend_schema,
            &resolver_consumer_context.pg_pool,
        )
        .await?
        .ok_or(ConsumerError::AtomNotFound)?;
        atom.label = ens.name;
        atom.image = ens.image;
        atom.upsert(backend_schema, &resolver_consumer_context.pg_pool)
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
