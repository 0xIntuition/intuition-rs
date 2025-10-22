use crate::{
    error::ConsumerError,
    mode::{
        resolver::{
            atom_resolver::{try_to_parse_json_or_text, try_to_resolve_schema_org_url},
            types::{ResolveAtom, ResolverConsumerMessage},
        },
        types::DecodedConsumerContext,
        utils::{get_or_create_account, short_id, update_account_with_atom_id},
    },
};
use alloy::primitives::Address;
use models::{
    atom::{Atom, AtomResolvingStatus, AtomType},
    atom_value::AtomValue,
    caip10::Caip10,
    traits::SimpleCrud,
    types::FixedBytesWrapper,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::str::FromStr;
use tracing::debug;
/// Represents the metadata for an atom
#[derive(Debug, Serialize, Deserialize)]
pub struct AtomMetadata {
    pub label: String,
    pub emoji: String,
    pub atom_type: String,
    pub image: Option<String>,
}

impl AtomMetadata {
    /// Creates a new atom metadata for an address
    pub fn address(address: &str, image: Option<String>) -> Self {
        Self {
            label: short_id(address),
            emoji: "⛓️".to_string(),
            atom_type: "Account".to_string(),
            image,
        }
    }

    /// Creates a new atom metadata for a book
    pub fn book(name: String) -> Self {
        Self {
            label: name,
            emoji: "📚".to_string(),
            atom_type: "Book".to_string(),
            image: None,
        }
    }

    /// Creates a new atom metadata for a byte object
    pub fn byte_object(image: Option<String>) -> Self {
        Self {
            label: "byte object".to_string(),
            emoji: "🔢".to_string(),
            atom_type: "ByteObject".to_string(),
            image,
        }
    }

    /// Creates a new atom metadata for a caip10
    pub fn caip10(caip10: String) -> Self {
        Self {
            label: caip10,
            emoji: "🔗".to_string(),
            atom_type: "Caip10".to_string(),
            image: None,
        }
    }

    /// Creates a new caip10
    pub async fn create_caip10(
        atom_id: FixedBytesWrapper,
        caip10: String,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<Caip10, ConsumerError> {
        let caip10_parts = caip10.split(':').collect::<Vec<&str>>();
        if caip10_parts.len() != 4 {
            return Err(ConsumerError::InvalidCaip10);
        }

        let namespace = caip10_parts[1];
        let chain_id = caip10_parts[2].parse::<i32>()?;
        let account_address = caip10_parts[3];

        Caip10::builder()
            .id(atom_id)
            .namespace(namespace)
            .chain_id(chain_id)
            .account_address(account_address)
            .build()
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await
            .map_err(ConsumerError::ModelError)
    }

    /// Creates a new atom metadata for a follow action
    pub fn follow_action(image: Option<String>) -> Self {
        Self {
            label: "follow".to_string(),
            emoji: "🔔".to_string(),
            atom_type: "FollowAction".to_string(),
            image,
        }
    }

    /// Stores the atom data in the database based on the atom type
    pub async fn handle_account_or_caip10_type(
        &self,
        resolved_atom: &ResolveAtom,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        match AtomType::from_str(self.atom_type.as_str())? {
            AtomType::Account => {
                debug!(
                    "Updating account for: {}",
                    resolved_atom.atom.data.clone().unwrap()
                );
                self.update_account_and_atom_value(resolved_atom, decoded_consumer_context)
                    .await
            }
            AtomType::Caip10 => {
                debug!(
                    "Creating caip10 for: {}",
                    resolved_atom.atom.data.clone().unwrap()
                );
                Self::create_caip10(
                    resolved_atom.atom.term_id.clone(),
                    resolved_atom.atom.data.clone().unwrap(),
                    decoded_consumer_context,
                )
                .await?;
                Ok(())
            }
            _ => {
                debug!(
                    "This atom type is updated at the end of processing: {}",
                    self.atom_type
                );
                Ok(())
            }
        }
    }

    /// Creates a new atom metadata for a json object
    pub fn json_object(image: Option<String>) -> Self {
        Self {
            label: "json object".to_string(),
            emoji: "📦".to_string(),
            atom_type: "JsonObject".to_string(),
            image,
        }
    }

    /// Creates a new atom metadata for a keywords predicate
    pub fn keywords_predicate(image: Option<String>) -> Self {
        Self {
            label: "has tag".to_string(),
            emoji: "🏷️".to_string(),
            atom_type: "Keywords".to_string(),
            image,
        }
    }

    /// Creates a new atom metadata for a like action
    pub fn like_action(image: Option<String>) -> Self {
        Self {
            label: "like".to_string(),
            emoji: "👍".to_string(),
            atom_type: "LikeAction".to_string(),
            image,
        }
    }

    /// Creates a new atom metadata for an organization
    pub fn organization(name: String, image: Option<String>) -> Self {
        Self {
            label: name,
            emoji: "🏢".to_string(),
            atom_type: "Organization".to_string(),
            image,
        }
    }

    /// Creates a new atom metadata for an organization predicate
    pub fn organization_predicate(image: Option<String>) -> Self {
        Self {
            label: "is organization".to_string(),
            emoji: "🏢".to_string(),
            atom_type: "OrganizationPredicate".to_string(),
            image,
        }
    }

    /// Creates a new atom metadata for a person
    pub fn person(name: String, image: Option<String>) -> Self {
        Self {
            label: name,
            emoji: "👤".to_string(),
            atom_type: "Person".to_string(),
            image,
        }
    }

    /// Creates a new atom metadata for a person predicate
    pub fn person_predicate(image: Option<String>) -> Self {
        Self {
            label: "is person".to_string(),
            emoji: "👤".to_string(),
            atom_type: "PersonPredicate".to_string(),
            image,
        }
    }

    /// Creates a new atom metadata for a text object
    pub fn text_object(name: Option<String>) -> Self {
        Self {
            label: name
                .unwrap_or("text object".to_string())
                .chars()
                .take(256)
                .collect(),
            emoji: "📝".to_string(),
            atom_type: "TextObject".to_string(),
            image: None,
        }
    }

    /// Creates a new atom metadata for a thing
    pub fn thing(name: String, image: Option<String>) -> Self {
        Self {
            label: name,
            emoji: "🧩".to_string(),
            atom_type: "Thing".to_string(),
            image,
        }
    }

    /// Creates a new atom metadata for a thing predicate
    pub fn thing_predicate(image: Option<String>) -> Self {
        Self {
            label: "is thing".to_string(),
            emoji: "🧩".to_string(),
            atom_type: "ThingPredicate".to_string(),
            image,
        }
    }

    /// Returns an unknown atom metadata
    pub fn unknown() -> Self {
        Self {
            label: "Unknown".to_string(),
            emoji: "❓".to_string(),
            atom_type: "Unknown".to_string(),
            image: None,
        }
    }

    /// Creates an account and an atom value
    pub async fn update_account_and_atom_value(
        &self,
        resolved_atom: &ResolveAtom,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        if self.atom_type != "Account" {
            debug!("Skipping account creation for: {}", self.atom_type);
            return Ok(());
        }

        let mut account = get_or_create_account(
            resolved_atom
                .atom
                .data
                .clone()
                .ok_or(ConsumerError::AtomDataNotFound)?,
            decoded_consumer_context,
        )
        .await?;

        update_account_with_atom_id(
            &mut account,
            resolved_atom.atom.term_id.clone(),
            decoded_consumer_context,
        )
        .await?;

        // Skip if atom value already exists
        if AtomValue::find_by_id(
            resolved_atom.atom.term_id.clone(),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        .is_some()
        {
            debug!("Atom value already exists, skipping...");
            return Ok(());
        }

        AtomValue::builder()
            .id(resolved_atom.atom.term_id.clone())
            .account_id(account.id)
            .build()
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await?;

        Ok(())
    }

    /// Updates the atom metadata
    pub async fn update_atom_metadata(
        &self,
        atom: &mut Atom,
        backend_schema: &str,
        pg_pool: &PgPool,
    ) -> Result<AtomMetadata, ConsumerError> {
        atom.emoji = Some(self.emoji.clone());
        atom.atom_type = AtomType::from_str(&self.atom_type)?;
        atom.label = Some(self.label.clone());
        atom.image = self.image.clone();
        atom.upsert(backend_schema, pg_pool).await?;
        Ok(AtomMetadata {
            label: self.label.clone(),
            emoji: self.emoji.clone(),
            atom_type: self.atom_type.clone(),
            image: self.image.clone(),
        })
    }
}

/// Validates if a string is a valid Ethereum address with proper EIP-55 checksum
///
/// # Arguments
/// * `address` - The address string to validate
///
/// # Returns
/// * `bool` - True if valid address with proper checksum, false otherwise
pub fn is_valid_address(address: &str) -> Result<bool, ConsumerError> {
    // Enforce EIP-55 checksum validation for all addresses
    match Address::parse_checksummed(address, None) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Validates if a string is a valid address format (without EIP-55 checksum validation)
///
/// # Arguments
/// * `address` - The address string to validate
///
/// # Returns
/// * `bool` - True if valid address format, false otherwise
pub fn is_valid_address_format(address: &str) -> Result<bool, ConsumerError> {
    // Basic address format validation without checksum enforcement
    match address.parse::<Address>() {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Validates if a string is a valid CAIP10
///
/// # Arguments
/// * `caip10` - The CAIP10 string to validate
///
/// # Returns
/// * `bool` - True if valid CAIP10, false otherwise
///
/// A `caip10` looks like: `caip10:eip155:8453:0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb70`
/// where the first part is the `caip10` prefix, the second part is the `eip155` namespace,
/// the third part is the `chain_id` and the last part is the `address`.
pub fn is_valid_caip10(caip10: &str) -> Result<bool, ConsumerError> {
    // Check if the string starts with "caip10:"
    if !caip10.starts_with("caip10:") {
        return Ok(false);
    }

    // Check if the string has at least 4 parts
    let parts = caip10.split(':').collect::<Vec<&str>>();
    if parts.len() < 4 {
        return Ok(false);
    }

    // Check if the last part is a valid address format
    let address = parts.last().unwrap();
    if !is_valid_address_format(address)? {
        return Ok(false);
    }

    Ok(true)
}

/// Gets the metadata for a supported atom type based on the atom data.
/// So when we receive the the atom data, there are some situations
/// we need to handle:
/// 1. The atom data is a schema.org URL. This is one of the "happy paths", since
///    we can directly map it to an atom metadata and dont need to resolve
///    anything.
/// 2. The atom data is an address. This is also one of the "happy paths",
///    since we can directly map it to an account and dont need to resolve
///    anything.
/// 3. The atom data is a CAIP10. This is also one of the "happy paths",
///    since we can directly map it to an account and dont need to resolve
///    anything.
/// 4. The atom data is an IPFS URI. We need to fetch the data from IPFS
///    and then resolve it. Keep in mind that if we are parsing an IPFS URI,
///    we need to fetch the data from IPFS and then parse it as JSON.
/// 5. The atom data is a JSON object. We need to resolve the properties
///    of the JSON object and then map it to an atom.
pub async fn get_supported_atom_metadata(
    atom: &mut Atom,
    decoded_atom_data: &str,
    decoded_consumer_context: &DecodedConsumerContext,
) -> Result<AtomMetadata, ConsumerError> {
    // Trim whitespace from decoded atom data
    let decoded_atom_data = decoded_atom_data.trim();

    // 1. Handling the happy path (schema.org URL, predicate)
    if let Some(schema_org_url) = try_to_resolve_schema_org_url(decoded_atom_data).await? {
        debug!("Schema.org URL found, returning predicate metadata...");
        // As we dont need to resolve anything, we can mark the atom as resolved
        atom.resolving_status = AtomResolvingStatus::Resolved;
        return Ok(get_predicate_metadata(schema_org_url, atom.image.clone()));
    } else {
        debug!("No schema.org URL found, verifying if atom data is an address...");
    }

    // 2. Handling the happy path (address)
    if is_valid_address(decoded_atom_data)? {
        debug!("Atom data is an address, returning account metadata...");
        // As we dont need to resolve anything, we can mark the atom as resolved
        atom.resolving_status = AtomResolvingStatus::Resolved;
        Ok(AtomMetadata::address(decoded_atom_data, atom.image.clone()))
    // 3. Handling the happy path (CAIP10)
    } else if is_valid_caip10(decoded_atom_data)? {
        debug!("Atom data is a CAIP10, returning account metadata...");
        // As we dont need to resolve anything, we can mark the atom as resolved
        atom.resolving_status = AtomResolvingStatus::Resolved;
        Ok(AtomMetadata::caip10(decoded_atom_data.to_string()))
    } else {
        debug!("Atom data is not an address, verifying if it's an IPFS URI...");
        // 4. Now we need to enqueue the message to be processed by the resolver
        let message = ResolverConsumerMessage::new_atom(atom.term_id.0.to_string());
        decoded_consumer_context
            .client
            .send_message(serde_json::to_string(&message)?, None)
            .await?;

        // 5. Now we try to parse the JSON and return the metadata. At this point
        // the resolver will handle the rest of the cases, like text object,
        // byte object, etc.
        let metadata =
            try_to_parse_json_or_text(decoded_atom_data, atom, decoded_consumer_context).await?;

        Ok(metadata)
    }
}

/// Returns the metadata for a predicate based on the current atom data state
pub fn get_predicate_metadata(
    current_atom_data_state: String,
    image: Option<String>,
) -> AtomMetadata {
    match current_atom_data_state.as_str() {
        "Person" => AtomMetadata::person_predicate(image),
        "Thing" => AtomMetadata::thing_predicate(image),
        "Organization" => AtomMetadata::organization_predicate(image),
        "Keywords" | "keywords" => AtomMetadata::keywords_predicate(image),
        "LikeAction" => AtomMetadata::like_action(image),
        "FollowAction" => AtomMetadata::follow_action(image),
        _ => AtomMetadata::unknown(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_caip10() -> Result<(), ConsumerError> {
        // Valid CAIP10
        assert!(is_valid_caip10(
            "caip10:eip155:8453:0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb70"
        )?);

        // Invalid cases
        assert!(!is_valid_caip10("not_caip10:eip155:1:0x123")?);
        assert!(!is_valid_caip10("caip10:eip155:1")?); // Missing address
        assert!(!is_valid_caip10("caip10:eip155:1:not_an_address")?);
        assert!(!is_valid_caip10("")?);

        Ok(())
    }

    #[test]
    fn test_is_valid_address_eip55_checksum() -> Result<(), ConsumerError> {
        // Test with a known valid EIP-55 checksummed address
        let valid_checksummed = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"; // Vitalik's address
        assert!(
            is_valid_address(valid_checksummed)?,
            "Valid EIP-55 checksummed address should be valid"
        );

        // Test with another valid EIP-55 checksummed address (from actual atom raw_data)
        let valid_checksummed_2 = "0xB95ca3D3144e9d1DAFF0EE3d35a4488A4A5C9Fc5";
        let result = is_valid_address(valid_checksummed_2)?;
        println!(
            "Address '{}' validation result: {}",
            valid_checksummed_2, result
        );
        assert!(result, "Address {} should be valid", valid_checksummed_2);

        // Test with invalid checksum (wrong capitalization)
        let invalid_checksummed = "0xd8da6BF26964aF9D7eEd9e03E53415D37aA96045"; // Wrong checksum
        let invalid_result = is_valid_address(invalid_checksummed)?;
        println!(
            "Invalid checksum '{}' is valid: {}",
            invalid_checksummed, invalid_result
        );
        assert!(
            !invalid_result,
            "Invalid EIP-55 checksum should be rejected"
        );

        // Test with all uppercase (should be invalid unless it's the correct checksum)
        let all_uppercase = "0xD8DA6BF26964AF9D7EED9E03E53415D37AA96045"; // Wrong checksum
        let all_upper_result = is_valid_address(all_uppercase)?;
        println!(
            "All uppercase '{}' is valid: {}",
            all_uppercase, all_upper_result
        );
        assert!(
            !all_upper_result,
            "All uppercase with wrong checksum should be rejected"
        );

        // Test the original addresses from your question
        let uppercase_address = "0xD5b879093c35B6D9F99e63B1EDB3d8164F70c9CC";
        let lowercase_address = "0xd5b879093c35b6d9f99e63b1edb3d8164f70c9cc";

        // Check if the uppercase version has valid EIP-55 checksum
        let uppercase_result = is_valid_address(uppercase_address)?;
        println!(
            "Uppercase address '{}' is valid: {}",
            uppercase_address, uppercase_result
        );

        // Lowercase addresses should be rejected (no valid checksum)
        let lowercase_result = is_valid_address(lowercase_address)?;
        println!(
            "Lowercase address '{}' is valid: {}",
            lowercase_address, lowercase_result
        );
        assert!(
            !lowercase_result,
            "Lowercase address should be rejected (no valid EIP-55 checksum)"
        );

        // Test some invalid cases
        assert!(!is_valid_address("not_an_address")?);
        assert!(!is_valid_address("0x")?);
        assert!(!is_valid_address("")?);
        assert!(!is_valid_address("0x123")?); // Too short

        Ok(())
    }

    #[test]
    fn test_is_valid_address_with_whitespace() -> Result<(), ConsumerError> {
        // Test addresses with leading/trailing whitespace
        let address_with_spaces = "  0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045  ";
        assert!(
            is_valid_address(address_with_spaces.trim())?,
            "Address with whitespace should be valid after trimming"
        );

        let address_with_newline = "0xB95ca3D3144e9d1DAFF0EE3d35a4488A4A5C9Fc5\n";
        assert!(
            is_valid_address(address_with_newline.trim())?,
            "Address with newline should be valid after trimming"
        );

        Ok(())
    }
}
