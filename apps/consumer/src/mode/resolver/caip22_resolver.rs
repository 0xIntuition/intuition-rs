use crate::{
    IERC721Metadata::IERC721MetadataInstance,
    error::ConsumerError,
    mode::{
        ipfs_upload::types::IpfsUploadMessage,
        metadata::{parse_caip22, AtomMetadata},
        resolver::atom_resolver::{create_json_object_atom_value, create_json_object_from_obj},
        types::ResolverConsumerContext,
    },
    traits::AtomUpdater,
};
use alloy::{
    primitives::{Address, U256},
    providers::{DynProvider, ProviderBuilder},
};
use models::{atom::Atom, traits::SimpleCrud};
use serde_json::Value;
use std::str::FromStr;
use tracing::{debug, warn};

/// Gets the RPC URL for a given chain ID from environment configuration
fn get_rpc_url_for_chain(
    chain_id: i64,
    resolver_consumer_context: &ResolverConsumerContext,
) -> Result<String, ConsumerError> {
    let env = &resolver_consumer_context.server_initialize.env;

    match chain_id {
        1 => env
            .ethereum_mainnet_rpc_url
            .clone()
            .ok_or(ConsumerError::UnsupportedChain(chain_id)),
        11155111 => env
            .ethereum_sepolia_rpc_url
            .clone()
            .ok_or(ConsumerError::UnsupportedChain(chain_id)),
        8453 => env
            .base_mainnet_rpc_url
            .clone()
            .ok_or(ConsumerError::UnsupportedChain(chain_id)),
        84532 => env
            .base_sepolia_rpc_url
            .clone()
            .ok_or(ConsumerError::UnsupportedChain(chain_id)),
        59144 => env
            .linea_mainnet_rpc_url
            .clone()
            .ok_or(ConsumerError::UnsupportedChain(chain_id)),
        59141 => env
            .linea_sepolia_rpc_url
            .clone()
            .ok_or(ConsumerError::UnsupportedChain(chain_id)),
        1155 => env
            .trust_mainnet_rpc_url
            .clone()
            .ok_or(ConsumerError::UnsupportedChain(chain_id)),
        13579 => env
            .trust_testnet_rpc_url
            .clone()
            .ok_or(ConsumerError::UnsupportedChain(chain_id)),
        31337 => env
            .local_intuition_rpc_url
            .clone()
            .ok_or(ConsumerError::UnsupportedChain(chain_id)),
        _ => Err(ConsumerError::UnsupportedChain(chain_id)),
    }
}

/// Resolves a CAIP-22 atom by fetching tokenURI and parsing metadata
/// Stores the resolved NFT metadata as a json_object
pub async fn resolve_caip22(
    atom: &Atom,
    resolver_consumer_context: &ResolverConsumerContext,
) -> Result<AtomMetadata, ConsumerError> {
    // Parse the CAIP-22 string from atom data
    let atom_data = atom.data.clone().ok_or(ConsumerError::AtomDataNotFound)?;
    let parsed = parse_caip22(&atom_data)?;

    debug!(
        "Resolving CAIP-22: chain_id={}, contract={}, token_id={}",
        parsed.chain_id, parsed.contract_address, parsed.token_id
    );

    // Step 1: Get RPC URL for the chain from environment config
    let rpc_url = get_rpc_url_for_chain(parsed.chain_id, resolver_consumer_context)?;

    // Step 2: Build provider and contract instance
    let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);
    let dyn_provider = DynProvider::new(provider);

    let contract_address = Address::from_str(&parsed.contract_address)
        .map_err(|e| ConsumerError::AddressParse(e.to_string()))?;

    let contract = IERC721MetadataInstance::new(contract_address, dyn_provider);

    // Step 3: Call tokenURI
    let token_id =
        U256::from_str(&parsed.token_id).map_err(|e| ConsumerError::UintParse(e.into()))?;

    let token_uri = contract.tokenURI(token_id).call().await.map_err(|e| {
        warn!(
            "Failed to fetch tokenURI for CAIP-22 {}: {}",
            atom.term_id, e
        );
        ConsumerError::Alloy(e)
    })?;

    debug!("Resolved tokenURI for CAIP-22: {}", token_uri);

    // Step 4: Fetch the metadata from tokenURI
    let metadata_json = fetch_token_metadata(&token_uri, resolver_consumer_context).await?;

    // Step 5: Store the JSON object and link to atom via atom_value.json_object_id
    let json_object = create_json_object_from_obj(atom, &metadata_json)
        .upsert(
            resolver_consumer_context.backend_schema(),
            resolver_consumer_context.pool(),
        )
        .await?;

    create_json_object_atom_value(atom, &json_object, resolver_consumer_context).await?;

    // Step 6: Extract name and image from metadata
    let name = metadata_json
        .get("name")
        .and_then(|v| v.as_str())
        .map(String::from);

    let image = metadata_json
        .get("image")
        .and_then(|v| v.as_str())
        .map(String::from);

    // Step 7: Send image to IPFS upload consumer for caching
    if let Some(ref img_url) = image {
        debug!("Sending CAIP-22 image to IPFS upload consumer: {}", img_url);
        resolver_consumer_context
            .client
            .send_message(
                serde_json::to_string(&IpfsUploadMessage {
                    image: img_url.clone(),
                })?,
                None,
            )
            .await?;
    }

    Ok(AtomMetadata::caip22(name, image))
}

/// Fetches token metadata from a URI (handles IPFS, HTTP, data URIs)
async fn fetch_token_metadata(
    token_uri: &str,
    resolver_consumer_context: &ResolverConsumerContext,
) -> Result<Value, ConsumerError> {
    // Handle IPFS URIs
    if let Some(ipfs_hash) = token_uri.strip_prefix("ipfs://") {
        debug!("Fetching CAIP-22 metadata from IPFS: {}", ipfs_hash);
        let response = resolver_consumer_context
            .ipfs_resolver
            .fetch_from_ipfs(ipfs_hash)
            .await
            .map_err(|e| {
                warn!("Failed to fetch from IPFS: {}", e);
                ConsumerError::UnsupportedTokenUri(token_uri.to_string())
            })?;
        let text = response.text().await?;
        return serde_json::from_str(&text).map_err(ConsumerError::from);
    }

    // Handle data URIs (base64 encoded JSON)
    if let Some(base64_data) = token_uri.strip_prefix("data:application/json;base64,") {
        debug!("Decoding CAIP-22 metadata from base64 data URI");
        let decoded = base64_decode(base64_data)?;
        let json_str = String::from_utf8(decoded)?;
        return serde_json::from_str(&json_str).map_err(ConsumerError::from);
    }

    // Handle raw JSON data URIs
    if let Some(json_data) = token_uri.strip_prefix("data:application/json,") {
        debug!("Parsing CAIP-22 metadata from raw JSON data URI");
        return serde_json::from_str(json_data).map_err(ConsumerError::from);
    }

    // Handle HTTP(S) URLs
    if token_uri.starts_with("http://") || token_uri.starts_with("https://") {
        debug!("Fetching CAIP-22 metadata from HTTP: {}", token_uri);
        let response = reqwest::get(token_uri).await?;
        let text = response.text().await?;
        return serde_json::from_str(&text).map_err(ConsumerError::from);
    }

    warn!("Unsupported token URI format: {}", token_uri);
    Err(ConsumerError::UnsupportedTokenUri(token_uri.to_string()))
}

/// Simple base64 decoding helper
fn base64_decode(input: &str) -> Result<Vec<u8>, ConsumerError> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD
        .decode(input)
        .map_err(|e| ConsumerError::DecodingError(e.to_string()))
}
