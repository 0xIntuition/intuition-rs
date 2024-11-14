use super::atom_resolver::{try_to_parse_json, try_to_resolve_ipfs_uri, SCHEMA_ORG_CONTEXTS};
use crate::{error::ConsumerError, mode::decoded::utils::short_id};
use alloy::primitives::Address;
use log::info;
use models::{
    account::{Account, AccountType},
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
    pub async fn create_account(
        &self,
        pg_pool: &PgPool,
        atom_data: &Atom,
        decoded_atom_data: &str,
    ) -> Result<(), ConsumerError> {
        if self.atom_type == "Account" {
            // Create the account first
            let account = Account::builder()
                .id(decoded_atom_data)
                .atom_id(atom_data.id.clone())
                .label(short_id(decoded_atom_data))
                .account_type(AccountType::Default)
                .build()
                .upsert(pg_pool)
                .await?;
            // create an AtomValue
            AtomValue::builder()
                .id(atom_data.vault_id.clone())
                .account_id(account.id.clone())
                .build()
                .upsert(pg_pool)
                .await?;
        }

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
        pg_pool: &PgPool,
        atom: &Atom,
        decoded_atom_data: &str,
    ) -> Result<(), ConsumerError> {
        match AtomType::from_str(self.atom_type.as_str())? {
            AtomType::Account => self.create_account(pg_pool, atom, decoded_atom_data).await,

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

/// This function tries to resolve a schema.org URL from the atom data
pub async fn try_to_resolve_schema_org_url(
    atom_data: &str,
) -> Result<Option<String>, ConsumerError> {
    // check if the atom data contains a predefine string (schema.org/something)
    if let Some(schema_org_url) = SCHEMA_ORG_CONTEXTS
        .iter()
        .find(|ctx| atom_data.starts_with(**ctx))
        .map(|ctx| atom_data[ctx.len()..].trim_start_matches('/').to_string())
    {
        Ok(Some(schema_org_url))
    } else {
        Ok(None)
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
    pg_pool: &PgPool,
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
        // 3. Handling IPFS URIs
        let data = try_to_resolve_ipfs_uri(decoded_atom_data).await?;
        // If we resolved an IPFS URI, we need to try to parse the JSON
        let metadata = if let Some(data) = data {
            info!("Atom data is an IPFS URI: {data}");
            try_to_parse_json(&data, atom, pg_pool).await?
        } else {
            info!("No IPFS URI found, trying to parse atom data as JSON...");
            // 4. This is the fallback case, where we try to parse the atom data as JSON
            // even if it's not a valid IPFS URI. This is useful for cases where the
            // atom data is a JSON object that is not a schema.org URL.
            try_to_parse_json(decoded_atom_data, atom, pg_pool).await?
        };

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
