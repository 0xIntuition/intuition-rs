use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::{get_or_create_account, short_id},
        resolver::{
            atom_resolver::{try_to_parse_json, try_to_resolve_schema_org_url},
            types::{ResolveAtom, ResolverConsumerMessage},
        },
        types::DecodedConsumerContext,
    },
};
use alloy::primitives::Address;
use log::info;
use models::{
    atom::{Atom, AtomType},
    atom_value::AtomValue,
    traits::SimpleCrud,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::str::FromStr;

/// Represents the metadata for an atom
#[derive(Debug, Serialize, Deserialize)]
pub struct AtomMetadata {
    pub label: String,
    pub emoji: String,
    pub atom_type: String,
}

impl AtomMetadata {
    /// Creates a new atom metadata for an address
    pub fn address(address: &str) -> Self {
        Self {
            label: short_id(address),
            emoji: "⛓️".to_string(),
            atom_type: "Account".to_string(),
        }
    }

    /// Creates a new atom metadata for a book
    pub fn book(name: String) -> Self {
        Self {
            label: name,
            emoji: "📚".to_string(),
            atom_type: "Book".to_string(),
        }
    }

    /// Creates an account and an atom value
    pub async fn create_account_and_atom_value(
        &self,
        resolved_atom: &ResolveAtom,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        if self.atom_type != "Account" {
            return Ok(());
        }

        let account = get_or_create_account(
            resolved_atom.decoded_atom_data.to_string(),
            decoded_consumer_context,
        )
        .await?;

        // Skip if atom value already exists
        if AtomValue::find_by_id(
            resolved_atom.atom.vault_id.clone(),
            &decoded_consumer_context.pg_pool,
        )
        .await?
        .is_some()
        {
            info!("Atom value already exists, skipping...");
            return Ok(());
        }

        AtomValue::builder()
            .id(resolved_atom.atom.vault_id.clone())
            .account_id(account.id)
            .build()
            .upsert(&decoded_consumer_context.pg_pool)
            .await?;

        Ok(())
    }

    /// Creates a new atom metadata for a follow action
    pub fn follow_action() -> Self {
        Self {
            label: "follow".to_string(),
            emoji: "🔔".to_string(),
            atom_type: "FollowAction".to_string(),
        }
    }

    /// Stores the atom data in the database based on the atom type
    pub async fn handle_account_type(
        &self,
        resolved_atom: &ResolveAtom,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        match AtomType::from_str(self.atom_type.as_str())? {
            AtomType::Account => {
                self.create_account_and_atom_value(resolved_atom, decoded_consumer_context)
                    .await
            }

            _ => {
                info!(
                    "This atom type is updated at the end of processing: {}",
                    self.atom_type
                );
                Ok(())
            }
        }
    }
    /// Creates a new atom metadata for a keywords predicate
    pub fn keywords_predicate() -> Self {
        Self {
            label: "has tag".to_string(),
            emoji: "🏷️".to_string(),
            atom_type: "Keywords".to_string(),
        }
    }

    /// Creates a new atom metadata for a like action
    pub fn like_action() -> Self {
        Self {
            label: "like".to_string(),
            emoji: "👍".to_string(),
            atom_type: "LikeAction".to_string(),
        }
    }

    /// Creates a new atom metadata for an organization
    pub fn organization(name: String) -> Self {
        Self {
            label: name,
            emoji: "🏢".to_string(),
            atom_type: "Organization".to_string(),
        }
    }

    /// Creates a new atom metadata for an organization predicate
    pub fn organization_predicate() -> Self {
        Self {
            label: "is organization".to_string(),
            emoji: "🏢".to_string(),
            atom_type: "OrganizationPredicate".to_string(),
        }
    }

    /// Creates a new atom metadata for a person
    pub fn person(name: String) -> Self {
        Self {
            label: name,
            emoji: "👤".to_string(),
            atom_type: "Person".to_string(),
        }
    }

    /// Creates a new atom metadata for a person predicate
    pub fn person_predicate() -> Self {
        Self {
            label: "is person".to_string(),
            emoji: "👤".to_string(),
            atom_type: "PersonPredicate".to_string(),
        }
    }

    /// Creates a new atom metadata for a thing
    pub fn thing(name: String) -> Self {
        Self {
            label: name,
            emoji: "🧩".to_string(),
            atom_type: "Thing".to_string(),
        }
    }

    /// Creates a new atom metadata for a thing predicate
    pub fn thing_predicate() -> Self {
        Self {
            label: "is thing".to_string(),
            emoji: "🧩".to_string(),
            atom_type: "ThingPredicate".to_string(),
        }
    }

    /// Returns an unknown atom metadata
    pub fn unknown() -> Self {
        Self {
            label: "Unknown".to_string(),
            emoji: "❓".to_string(),
            atom_type: "Unknown".to_string(),
        }
    }

    /// Updates the atom metadata
    pub async fn update_atom_metadata(
        &self,
        atom: &mut Atom,
        pg_pool: &PgPool,
    ) -> Result<(), ConsumerError> {
        atom.emoji = Some(self.emoji.clone());
        atom.atom_type = AtomType::from_str(&self.atom_type)?;
        atom.label = Some(self.label.clone());
        atom.upsert(pg_pool).await?;
        Ok(())
    }
}

/// Validates if a string is a valid Ethereum address
///
/// # Arguments
/// * `address` - The address string to validate
///
/// # Returns
/// * `bool` - True if valid address, false otherwise
pub fn is_valid_address(address: &str) -> Result<bool, ConsumerError> {
    // Try to parse the address as an alloy Address type
    match Address::from_str(address) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Gets the metadata for a supported atom type based on the atom data.
/// So when we receive the the atom data, there are some situations
/// we need to handle:
/// 1. The atom data is a schema.org URL. This is the "happy path", since
///    we can directly map it to an atom metadata and dont need to resolve
///    anything.
/// 2. The atom data is an address. This is also some sort of "happy path",
///    since we can directly map it to an account and dont need to resolve
///    anything.
/// 3. The atom data is an IPFS URI. We need to fetch the data from IPFS
///    and then resolve it. Keep in mind that if we are parsing an IPFS URI,
///    we need to fetch the data from IPFS and then parse it as JSON.
/// 4. The atom data is a JSON object. We need to resolve the properties
///    of the JSON object and then map it to an atom.
pub async fn get_supported_atom_metadata(
    atom: &mut Atom,
    decoded_atom_data: &str,
    decoded_consumer_context: &DecodedConsumerContext,
) -> Result<AtomMetadata, ConsumerError> {
    // 1. Handling the happy path (schema.org URL, predicate)
    if let Some(schema_org_url) = try_to_resolve_schema_org_url(decoded_atom_data).await? {
        return Ok(get_predicate_metadata(schema_org_url));
    } else {
        info!("No schema.org URL found, verifying if atom data is an address...");
    }

    // 2. Handling the happy path (address)
    if is_valid_address(decoded_atom_data)? {
        info!("Atom data is an address, returning account metadata...");
        Ok(AtomMetadata::address(decoded_atom_data))
    } else {
        info!("Atom data is not an address, verifying if it's an IPFS URI...");
        // 3. Now we need to enqueue the message to be processed by the resolver
        let message =
            ResolverConsumerMessage::new_atom(atom.clone(), decoded_atom_data.to_string());
        decoded_consumer_context
            .client
            .send_message(serde_json::to_string(&message)?)
            .await?;

        // Now we try to parse the JSON and return the metadata. At this point
        // the resolver will handle the rest of the cases.
        let metadata =
            try_to_parse_json(decoded_atom_data, atom, &decoded_consumer_context.pg_pool).await?;

        Ok(metadata)
    }
}

/// Returns the metadata for a predicate based on the current atom data state
pub fn get_predicate_metadata(current_atom_data_state: String) -> AtomMetadata {
    match current_atom_data_state.as_str() {
        "Person" => AtomMetadata::person_predicate(),
        "Thing" => AtomMetadata::thing_predicate(),
        "Organization" => AtomMetadata::organization_predicate(),
        "Keywords" => AtomMetadata::keywords_predicate(),
        "LikeAction" => AtomMetadata::like_action(),
        "FollowAction" => AtomMetadata::follow_action(),
        _ => AtomMetadata::unknown(),
    }
}
