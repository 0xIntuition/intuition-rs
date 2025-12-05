use crate::{
    error::ConsumerError,
    mode::{
        resolver::{
            atom_resolver::{try_to_parse_json_or_text, try_to_resolve_schema_org_url},
            types::ResolverConsumerMessage,
        },
        types::DecodedConsumerContext,
        utils::{get_or_create_account, short_id},
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

    /// Creates a new atom metadata for a CAIP-22 NFT
    pub fn caip22(name: Option<String>, image: Option<String>) -> Self {
        Self {
            label: name.unwrap_or_else(|| "NFT".to_string()),
            emoji: "🖼️".to_string(),
            atom_type: "Caip22".to_string(),
            image,
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
        atom: &mut Atom,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        match AtomType::from_str(self.atom_type.as_str())? {
            AtomType::Account => {
                debug!("Updating account for: {}", atom.data.clone().unwrap());
                self.update_account_and_atom_value(atom, decoded_consumer_context)
                    .await
            }
            AtomType::Caip10 => {
                debug!("Creating caip10 for: {}", atom.data.clone().unwrap());
                Self::create_caip10(
                    atom.term_id.clone(),
                    atom.data.clone().unwrap(),
                    decoded_consumer_context,
                )
                .await?;
                Ok(())
            }
            _ => {
                debug!("No need to update atom type");
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
        atom: &mut Atom,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        if self.atom_type != "Account" {
            debug!("Skipping account creation for: {}", self.atom_type);
            return Ok(());
        }

        let account = get_or_create_account(
            atom.data.clone().unwrap(),
            decoded_consumer_context,
            Some(atom.term_id.clone()),
        )
        .await?;

        // now we enqueue the message to be processed by the resolver
        let message = ResolverConsumerMessage::new_account(account.clone());
        decoded_consumer_context
            .client
            .send_message(serde_json::to_string(&message)?, None)
            .await?;

        // Skip if atom value already exists
        if AtomValue::find_by_id(
            atom.term_id.clone(),
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
            .id(atom.term_id.clone())
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

/// Validates if a string is a valid EIP-155 address format without enforcing EIP-55 checksum
///
/// # Arguments
/// * `address` - The address string to validate
///
/// # Returns
/// * `bool` - True if valid EIP-155 address format, false otherwise
///
/// This function validates the hex address format (0x followed by 40 hex characters)
/// but does not enforce EIP-55 checksum validation. Use this for CAIP-10 addresses
/// where checksumming may not be enforced.
pub fn is_valid_eip155_address(address: &str) -> bool {
    // Must start with 0x
    if !address.starts_with("0x") {
        return false;
    }

    // Must be exactly 42 characters (0x + 40 hex chars)
    if address.len() != 42 {
        return false;
    }

    // All characters after 0x must be valid hex (0-9, a-f, A-F)
    address[2..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Validates if a string is a valid account format for non-Ethereum chains
///
/// # Arguments
/// * `account` - The account string to validate
///
/// # Returns
/// * `bool` - True if valid account format, false otherwise
///
/// This function performs basic format validation for different blockchain account formats:
/// - Bitcoin: 26-35 characters, base58 encoded
/// - Solana: 32-44 characters, base58 encoded
/// - Cosmos: bech32 encoded with specific prefixes
/// - General: non-empty, reasonable length, no invalid characters
pub fn is_valid_account_format(account: &str) -> bool {
    if account.is_empty() {
        return false;
    }

    // Check for reasonable length (most blockchain addresses are 20-100 chars)
    if account.len() < 10 || account.len() > 100 {
        return false;
    }

    // Check for obviously invalid characters
    if account.contains('\0') || account.contains('\n') || account.contains('\r') {
        return false;
    }

    // Basic format checks for common blockchain address patterns
    if account.starts_with("bc1") || account.starts_with("tb1") {
        // Bitcoin bech32 addresses
        return account.len() >= 42 && account.len() <= 62;
    } else if account.starts_with("1") || account.starts_with("3") {
        // Bitcoin legacy addresses
        return account.len() >= 26 && account.len() <= 35;
    } else if account.starts_with("cosmos1") || account.starts_with("osmo1") {
        // Cosmos bech32 addresses
        return account.len() >= 39 && account.len() <= 59;
    } else if account.len() >= 32 && account.len() <= 44 {
        // Solana addresses (base58, 32-44 chars)
        return true;
    }

    // For other formats, just ensure it's not obviously malformed
    // Allow alphanumeric, hyphens, underscores, dots
    account
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// Parsed CAIP-22 components
#[derive(Debug, Clone)]
pub struct ParsedCaip22 {
    pub namespace: String,
    pub chain_id: i64,
    pub asset_namespace: String,
    pub contract_address: String,
    pub token_id: String,
}

/// Validates if a string is a valid CAIP-22 NFT asset identifier
///
/// # Arguments
/// * `caip22` - The CAIP-22 string to validate
///
/// # Returns
/// * `bool` - True if valid CAIP-22, false otherwise
///
/// Format: `caip22:eip155:{chain_id}/erc721:{contract_address}/{token_id}`
///
/// # Examples
/// - `caip22:eip155:84532/erc721:0x8004AA63c570c570eBF15376c0dB199918BFe9Fb/1563`
/// - `caip22:eip155:11155111/erc721:0x8004a6090Cd10A7288092483047B097295Fb8847/3265`
pub fn is_valid_caip22(caip22: &str) -> Result<bool, ConsumerError> {
    Ok(parse_caip22(caip22).is_ok())
}

/// Parses a CAIP-22 string into its components
///
/// # Arguments
/// * `caip22` - The CAIP-22 string to parse
///
/// # Returns
/// * `ParsedCaip22` - The parsed components
///
/// # Errors
/// Returns `ConsumerError::InvalidCaip22` if the format is invalid
pub fn parse_caip22(caip22: &str) -> Result<ParsedCaip22, ConsumerError> {
    // Must start with "caip22:"
    let without_prefix = caip22
        .strip_prefix("caip22:")
        .ok_or(ConsumerError::InvalidCaip22)?;

    // Split by '/' to get chain reference and asset reference
    // Format: eip155:84532/erc721:0x.../1563
    let parts: Vec<&str> = without_prefix.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(ConsumerError::InvalidCaip22);
    }

    let chain_ref = parts[0]; // eip155:84532
    let asset_ref = parts[1]; // erc721:0x.../1563

    // Parse chain reference (namespace:chain_id)
    let chain_parts: Vec<&str> = chain_ref.split(':').collect();
    if chain_parts.len() != 2 {
        return Err(ConsumerError::InvalidCaip22);
    }

    let namespace = chain_parts[0];
    let chain_id_str = chain_parts[1];

    // Validate namespace is eip155 (for now)
    if namespace != "eip155" {
        return Err(ConsumerError::InvalidCaip22);
    }

    // Parse chain_id
    let chain_id: i64 = chain_id_str
        .parse()
        .map_err(|_| ConsumerError::InvalidCaip22)?;

    // Parse asset reference (asset_namespace:contract/token_id)
    let asset_parts: Vec<&str> = asset_ref.splitn(2, ':').collect();
    if asset_parts.len() != 2 {
        return Err(ConsumerError::InvalidCaip22);
    }

    let asset_namespace = asset_parts[0]; // erc721
    let contract_and_token = asset_parts[1]; // 0x.../1563

    // Validate asset namespace
    if asset_namespace != "erc721" && asset_namespace != "erc1155" {
        return Err(ConsumerError::InvalidCaip22);
    }

    // Split contract address and token ID (token_id is after the last '/')
    let last_slash_pos = contract_and_token
        .rfind('/')
        .ok_or(ConsumerError::InvalidCaip22)?;

    let contract_address = &contract_and_token[..last_slash_pos];
    let token_id = &contract_and_token[last_slash_pos + 1..];

    // Validate contract address
    if !is_valid_eip155_address(contract_address) {
        return Err(ConsumerError::InvalidCaip22);
    }

    // Validate token_id is a valid positive integer
    if token_id.is_empty() || !token_id.chars().all(|c| c.is_ascii_digit()) {
        return Err(ConsumerError::InvalidCaip22);
    }

    Ok(ParsedCaip22 {
        namespace: namespace.to_string(),
        chain_id,
        asset_namespace: asset_namespace.to_string(),
        contract_address: contract_address.to_string(),
        token_id: token_id.to_string(),
    })
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

    let namespace = parts[1];
    let address = parts.last().unwrap();

    // For eip155 chains, validate address format without enforcing EIP-55 checksum
    if namespace == "eip155" {
        if !is_valid_eip155_address(address) {
            return Ok(false);
        }
    } else {
        // For other chains, perform basic format validation
        if !is_valid_account_format(address) {
            return Ok(false);
        }
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
    decoded_consumer_context: &DecodedConsumerContext,
) -> Result<AtomMetadata, ConsumerError> {
    // 1. Handling the happy path (schema.org URL, predicate)
    if let Some(schema_org_url) =
        try_to_resolve_schema_org_url(&atom.data.clone().ok_or(ConsumerError::AtomDataNotFound)?)
            .await?
    {
        debug!("Schema.org URL found, returning predicate metadata...");
        // As we dont need to resolve anything, we can mark the atom as resolved
        atom.resolving_status = AtomResolvingStatus::Resolved;
        return Ok(get_predicate_metadata(schema_org_url, atom.image.clone()));
    } else {
        debug!("No schema.org URL found, verifying if atom data is an address...");
    }

    // 2. Handling the happy path (address)
    if is_valid_address(&atom.data.clone().ok_or(ConsumerError::AtomDataNotFound)?)? {
        debug!("Atom data is an address, returning account metadata...");
        // As we dont need to resolve anything, we can mark the atom as resolved
        atom.resolving_status = AtomResolvingStatus::Resolved;
        Ok(AtomMetadata::address(
            &atom.data.clone().ok_or(ConsumerError::AtomDataNotFound)?,
            atom.image.clone(),
        ))
    // 3. Handling the happy path (CAIP10)
    } else if is_valid_caip10(&atom.data.clone().ok_or(ConsumerError::AtomDataNotFound)?)? {
        debug!("Atom data is a CAIP10, returning account metadata...");
        // As we dont need to resolve anything, we can mark the atom as resolved
        atom.resolving_status = AtomResolvingStatus::Resolved;
        Ok(AtomMetadata::caip10(
            atom.data.clone().ok_or(ConsumerError::AtomDataNotFound)?,
        ))
    // 4. Handling CAIP-22 (NFT asset identifier - requires resolution)
    } else if is_valid_caip22(&atom.data.clone().ok_or(ConsumerError::AtomDataNotFound)?)? {
        debug!("Atom data is a CAIP-22, enqueuing for resolution...");
        // Mark as pending since we need to resolve the tokenURI
        atom.resolving_status = AtomResolvingStatus::Pending;

        // Enqueue for resolution in the resolver consumer
        let message = ResolverConsumerMessage::new_atom(atom.term_id.0.to_string());
        decoded_consumer_context
            .client
            .send_message(serde_json::to_string(&message)?, None)
            .await?;

        Ok(AtomMetadata::caip22(None, None))
    } else {
        debug!("Atom data is not an address or CAIP, verifying if it's an IPFS URI...");
        // 5. Now we need to enqueue the message to be processed by the resolver
        let message = ResolverConsumerMessage::new_atom(atom.term_id.0.to_string());
        decoded_consumer_context
            .client
            .send_message(serde_json::to_string(&message)?, None)
            .await?;

        // 5. Now we try to parse the JSON and return the metadata. At this point
        // the resolver will handle the rest of the cases, like text object,
        // byte object, etc.
        let metadata = try_to_parse_json_or_text(
            &atom.data.clone().ok_or(ConsumerError::AtomDataNotFound)?,
            atom,
            decoded_consumer_context,
        )
        .await?;

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
    fn test_is_valid_caip22() -> Result<(), ConsumerError> {
        // Valid CAIP-22 examples
        assert!(is_valid_caip22(
            "caip22:eip155:84532/erc721:0x8004AA63c570c570eBF15376c0dB199918BFe9Fb/1563"
        )?);
        assert!(is_valid_caip22(
            "caip22:eip155:11155111/erc721:0x8004a6090Cd10A7288092483047B097295Fb8847/3265"
        )?);
        assert!(is_valid_caip22(
            "caip22:eip155:1/erc721:0xBC4CA0EdA7647A8aB7C2061c2E118A18a936f13D/1234"
        )?);
        // ERC-1155 should also be valid
        assert!(is_valid_caip22(
            "caip22:eip155:1/erc1155:0x76BE3b62873462d2142405439777e971754E8E77/100"
        )?);

        // Invalid cases
        assert!(!is_valid_caip22("caip10:eip155:1:0x123")?); // Wrong prefix
        assert!(!is_valid_caip22("caip22:eip155:84532")?); // Missing asset ref
        assert!(!is_valid_caip22("caip22:eip155:84532/erc721:0x123")?); // Missing token ID
        assert!(!is_valid_caip22(
            "caip22:eip155:abc/erc721:0x8004AA63c570c570eBF15376c0dB199918BFe9Fb/1"
        )?); // Invalid chain ID
        assert!(!is_valid_caip22("caip22:eip155:1/erc721:not_an_address/1")?); // Invalid address
        assert!(!is_valid_caip22(
            "caip22:eip155:1/erc20:0x8004AA63c570c570eBF15376c0dB199918BFe9Fb/1"
        )?); // Invalid asset namespace
        assert!(!is_valid_caip22("")?);

        Ok(())
    }

    #[test]
    fn test_parse_caip22() -> Result<(), ConsumerError> {
        let parsed = parse_caip22(
            "caip22:eip155:84532/erc721:0x8004AA63c570c570eBF15376c0dB199918BFe9Fb/1563",
        )?;

        assert_eq!(parsed.namespace, "eip155");
        assert_eq!(parsed.chain_id, 84532);
        assert_eq!(parsed.asset_namespace, "erc721");
        assert_eq!(
            parsed.contract_address,
            "0x8004AA63c570c570eBF15376c0dB199918BFe9Fb"
        );
        assert_eq!(parsed.token_id, "1563");

        // Test with large chain ID
        let parsed2 = parse_caip22(
            "caip22:eip155:11155111/erc721:0x8004a6090Cd10A7288092483047B097295Fb8847/3265",
        )?;
        assert_eq!(parsed2.chain_id, 11155111);
        assert_eq!(parsed2.token_id, "3265");

        // Test with ERC-1155
        let parsed3 =
            parse_caip22("caip22:eip155:1/erc1155:0x76BE3b62873462d2142405439777e971754E8E77/100")?;
        assert_eq!(parsed3.asset_namespace, "erc1155");

        Ok(())
    }

    #[test]
    fn test_is_valid_caip10() -> Result<(), ConsumerError> {
        // Valid CAIP10
        assert!(is_valid_caip10(
            "caip10:eip155:8453:0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb70"
        )?);

        // Valid non-eip155 CAIP10
        assert!(is_valid_caip10(
            "caip10:cosmos:cosmoshub-4:cosmos1abc123def456ghi789jkl012mno345pqr678stu901"
        )?);
        assert!(is_valid_caip10(
            "caip10:bitcoin:mainnet:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"
        )?);

        // Invalid cases
        assert!(!is_valid_caip10("not_caip10:eip155:1:0x123")?);
        assert!(!is_valid_caip10("caip10:eip155:1")?); // Missing address
        assert!(!is_valid_caip10("caip10:eip155:1:not_an_address")?);
        assert!(!is_valid_caip10("caip10:cosmos:cosmoshub-4:")?); // Empty address
        assert!(!is_valid_caip10("")?);

        Ok(())
    }

    #[test]
    fn test_is_valid_account_format() {
        // Valid Bitcoin addresses
        assert!(is_valid_account_format(
            "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"
        ));
        assert!(is_valid_account_format(
            "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy"
        ));
        assert!(is_valid_account_format(
            "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4"
        ));

        // Valid Cosmos addresses
        assert!(is_valid_account_format(
            "cosmos1abc123def456ghi789jkl012mno345pqr678stu901"
        ));
        assert!(is_valid_account_format(
            "osmo1abc123def456ghi789jkl012mno345pqr678stu901"
        ));

        // Valid Solana addresses (32-44 chars)
        assert!(is_valid_account_format("11111111111111111111111111111112"));
        assert!(is_valid_account_format(
            "So11111111111111111111111111111111111111112"
        ));

        // Valid general format
        assert!(is_valid_account_format("user123456"));
        assert!(is_valid_account_format("account-name"));
        assert!(is_valid_account_format("user.name123"));

        // Invalid cases
        assert!(!is_valid_account_format("")); // Empty
        assert!(!is_valid_account_format("a")); // Too short
        assert!(!is_valid_account_format(&"a".repeat(101))); // Too long
        assert!(!is_valid_account_format("invalid\nchar")); // Contains newline
        assert!(!is_valid_account_format("invalid\0char")); // Contains null
        assert!(!is_valid_account_format("invalid@char")); // Invalid character
    }

    #[test]
    fn test_is_valid_address_eip55_checksum() -> Result<(), ConsumerError> {
        // Test with a known valid EIP-55 checksummed address
        let valid_checksummed = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"; // Vitalik's address
        assert!(
            is_valid_address(valid_checksummed)?,
            "Valid EIP-55 checksummed address should be valid"
        );

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
}
