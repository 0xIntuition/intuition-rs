use crate::{
    IERC721Metadata::IERC721MetadataInstance,
    error::ConsumerError,
    mode::{
        ipfs_upload::types::IpfsUploadMessage,
        metadata::{AtomMetadata, ParsedCaip22, parse_caip22},
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
    let parsed = parse_caip22_data(atom)?;

    debug!(
        "Resolving CAIP-22: chain_id={}, contract={}, token_id={}",
        parsed.chain_id, parsed.contract_address, parsed.token_id
    );

    let token_uri = fetch_token_uri_from_contract(&parsed, resolver_consumer_context).await?;
    debug!("Resolved tokenURI for CAIP-22: {}", token_uri);

    let metadata_json = fetch_token_metadata(&token_uri, resolver_consumer_context).await?;
    store_metadata_json(atom, &metadata_json, resolver_consumer_context).await?;

    let (name, image) = extract_metadata_fields(&metadata_json);
    queue_image_for_ipfs(&image, resolver_consumer_context).await?;

    Ok(AtomMetadata::caip22(name, image))
}

/// Parses the CAIP-22 string from atom data
fn parse_caip22_data(atom: &Atom) -> Result<ParsedCaip22, ConsumerError> {
    let atom_data = atom.data.clone().ok_or(ConsumerError::AtomDataNotFound)?;
    parse_caip22(&atom_data)
}

/// Builds an ERC721 contract instance for the given CAIP-22 parsed data
fn build_erc721_contract(
    parsed: &ParsedCaip22,
    resolver_consumer_context: &ResolverConsumerContext,
) -> Result<IERC721MetadataInstance<DynProvider>, ConsumerError> {
    let rpc_url = get_rpc_url_for_chain(parsed.chain_id, resolver_consumer_context)?;
    let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);
    let dyn_provider = DynProvider::new(provider);

    let contract_address = Address::from_str(&parsed.contract_address)
        .map_err(|e| ConsumerError::AddressParse(e.to_string()))?;

    Ok(IERC721MetadataInstance::new(contract_address, dyn_provider))
}

/// Fetches the tokenURI from an ERC721 contract
async fn fetch_token_uri_from_contract(
    parsed: &ParsedCaip22,
    resolver_consumer_context: &ResolverConsumerContext,
) -> Result<String, ConsumerError> {
    let contract = build_erc721_contract(parsed, resolver_consumer_context)?;
    let token_id = U256::from_str(&parsed.token_id).map_err(ConsumerError::UintParse)?;

    contract.tokenURI(token_id).call().await.map_err(|e| {
        warn!(
            "Failed to fetch tokenURI for CAIP-22 chain_id={}, contract={}, token_id={}: {}",
            parsed.chain_id, parsed.contract_address, parsed.token_id, e
        );
        ConsumerError::Alloy(e)
    })
}

/// Stores the metadata JSON object and links it to the atom
async fn store_metadata_json(
    atom: &Atom,
    metadata_json: &Value,
    resolver_consumer_context: &ResolverConsumerContext,
) -> Result<(), ConsumerError> {
    let json_object = create_json_object_from_obj(atom, metadata_json)
        .upsert(
            resolver_consumer_context.backend_schema(),
            resolver_consumer_context.pool(),
        )
        .await?;

    create_json_object_atom_value(atom, &json_object, resolver_consumer_context).await?;
    Ok(())
}

/// Extracts name and image fields from metadata JSON
fn extract_metadata_fields(metadata_json: &Value) -> (Option<String>, Option<String>) {
    let name = metadata_json
        .get("name")
        .and_then(|v| v.as_str())
        .map(String::from);

    let image = metadata_json
        .get("image")
        .and_then(|v| v.as_str())
        .map(String::from);

    (name, image)
}

/// Queues an image URL for IPFS upload if provided
async fn queue_image_for_ipfs(
    image: &Option<String>,
    resolver_consumer_context: &ResolverConsumerContext,
) -> Result<(), ConsumerError> {
    if let Some(img_url) = image {
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
    Ok(())
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
    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD
        .decode(input)
        .map_err(|e| ConsumerError::DecodingError(e.to_string()))
}
