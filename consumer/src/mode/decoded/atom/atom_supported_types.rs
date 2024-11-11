use alloy::primitives::Address;
use log::{info, warn};
use models::{
    account::{Account, AccountType},
    atom::{Atom, AtomType},
    atom_value::AtomValue,
    traits::SimpleCrud,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::str::FromStr;

use crate::{error::ConsumerError, mode::decoded::utils::short_id};

use super::atom_resolver::{try_to_parse_json, try_to_resolve_ipfs_uri};

/// Represents the metadata for an atom
#[derive(Debug, Serialize, Deserialize)]
pub struct AtomMetadata {
    pub label: String,
    pub emoji: String,
    pub atom_type: String,
}

impl AtomMetadata {
    pub fn new(label: String, emoji: String, atom_type: String) -> Self {
        Self {
            label,
            emoji,
            atom_type,
        }
    }

    /// Stores the atom data in the database based on the atom type
    pub async fn handle_account_type(
        &self,
        pg_pool: &PgPool,
        atom: &mut Atom,
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

    /// Creates an account and an atom value
    pub async fn create_account(
        &self,
        pg_pool: &PgPool,
        atom_data: &mut Atom,
        decoded_atom_data: &str,
    ) -> Result<(), ConsumerError> {
        if self.atom_type == "Account" {
            // Create the account first
            let account = Account::builder()
                .id(decoded_atom_data)
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

        atom_data.atom_type = AtomType::from_str(self.atom_type.as_str())?;

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

/// Gets the metadata for a supported atom type based on the atom data
pub async fn get_supported_atom_metadata(
    atom: &Atom,
    decoded_atom_data: &str,
    pg_pool: &PgPool,
) -> Result<AtomMetadata, ConsumerError> {
    // So when we receive the the atom data, there are some situations
    // we need to handle:
    // 1. The atom data is an address. This is the "happy path", since
    //    we can directly map it to an account and dont need to resolve
    //    anything.
    // 2. The atom data is an IPFS URI. We need to fetch the data from IPFS
    //    and then resolve it.
    // 3. The atom data is a JSON object. We need to resolve the properties
    //    of the JSON object and then map it to an atom.
    // Keep in mind that if we are parsing an IPFS URI, we need to fetch
    // the data from IPFS and then parse it as JSON.

    // We need to keep track of the current state of the atom data
    // because we might need to update it if we are resolving an IPFS URI
    let mut current_atom_data_state: String = decoded_atom_data.to_string();

    warn!("Current atom data state: {}", current_atom_data_state);
    // Handling the happy path
    if is_valid_address(&current_atom_data_state)? {
        Ok(AtomMetadata::new(
            short_id(&current_atom_data_state),
            "⛓️".to_string(),
            "Account".to_string(),
        ))
    } else {
        // Handle IPFS URIs
        let data = try_to_resolve_ipfs_uri(&current_atom_data_state).await?;
        // If we resolved an IPFS URI, we need to update the current state of the atom data
        if let Some(data) = data {
            current_atom_data_state = data;
        }

        // Try to parse the JSON
        match try_to_parse_json(&current_atom_data_state, atom, pg_pool).await {
            Ok(json_data) => {
                // If we resolved a JSON, we need to update the current state of the atom data
                if let Some(json_data) = json_data {
                    current_atom_data_state = json_data;
                }
            }
            Err(e) => {
                warn!(
                    "Not able to parse {} into a JSON: {}",
                    current_atom_data_state, e
                );
            }
        }

        let metadata = get_atom_metadata_from_str(current_atom_data_state);

        Ok(metadata)
    }
}

pub fn get_atom_metadata_from_str(current_atom_data_state: String) -> AtomMetadata {
    match current_atom_data_state.as_str() {
        "https://schema.org/Person" => AtomMetadata {
            label: "is person".to_string(),
            emoji: "👤".to_string(),
            atom_type: "PersonPredicate".to_string(),
        },
        "https://schema.org/Thing" => AtomMetadata {
            label: "is thing".to_string(),
            emoji: "🧩".to_string(),
            atom_type: "ThingPredicate".to_string(),
        },
        "https://schema.org/Organization" => AtomMetadata {
            label: "is organization".to_string(),
            emoji: "🏢".to_string(),
            atom_type: "OrganizationPredicate".to_string(),
        },
        "https://schema.org/keywords" => AtomMetadata {
            label: "has tag".to_string(),
            emoji: "🏷️".to_string(),
            atom_type: "Keywords".to_string(),
        },
        "https://schema.org/LikeAction" => AtomMetadata {
            label: "like".to_string(),
            emoji: "👍".to_string(),
            atom_type: "LikeAction".to_string(),
        },
        "https://schema.org/FollowAction" => AtomMetadata {
            label: "follow".to_string(),
            emoji: "🔔".to_string(),
            atom_type: "FollowAction".to_string(),
        },
        _ => AtomMetadata {
            label: "Unknown".to_string(),
            emoji: "❓".to_string(),
            atom_type: "Unknown".to_string(),
        },
    }
}
