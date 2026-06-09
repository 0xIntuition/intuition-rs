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
use std::{str::FromStr, time::Duration};
use tracing::{debug, warn};

/// Maximum response size for metadata fetches (1 MB)
const MAX_RESPONSE_SIZE: usize = 1024 * 1024;

/// Maximum base64 input length (slightly more than MAX_RESPONSE_SIZE to account for encoding overhead)
const MAX_BASE64_INPUT_LENGTH: usize = 1024 * 1024 + 100_000;

/// HTTP request timeout in seconds
const HTTP_TIMEOUT_SECS: u64 = 30;

/// Gets the RPC URL for a given chain ID from environment configuration.
/// Returns `ChainRpcNotConfigured` if chain is supported but RPC URL is missing,
/// or `UnsupportedChain` if the chain ID is not supported at all.
fn get_rpc_url_for_chain(
    chain_id: i64,
    resolver_consumer_context: &ResolverConsumerContext,
) -> Result<String, ConsumerError> {
    let env = &resolver_consumer_context.server_initialize.env;

    match chain_id {
        1 => env
            .ethereum_mainnet_rpc_url
            .clone()
            .ok_or(ConsumerError::ChainRpcNotConfigured(chain_id)),
        11155111 => env
            .ethereum_sepolia_rpc_url
            .clone()
            .ok_or(ConsumerError::ChainRpcNotConfigured(chain_id)),
        8453 => env
            .base_mainnet_rpc_url
            .clone()
            .ok_or(ConsumerError::ChainRpcNotConfigured(chain_id)),
        84532 => env
            .base_sepolia_rpc_url
            .clone()
            .ok_or(ConsumerError::ChainRpcNotConfigured(chain_id)),
        59144 => env
            .linea_mainnet_rpc_url
            .clone()
            .ok_or(ConsumerError::ChainRpcNotConfigured(chain_id)),
        59141 => env
            .linea_sepolia_rpc_url
            .clone()
            .ok_or(ConsumerError::ChainRpcNotConfigured(chain_id)),
        1155 => env
            .trust_mainnet_rpc_url
            .clone()
            .ok_or(ConsumerError::ChainRpcNotConfigured(chain_id)),
        13579 => env
            .trust_testnet_rpc_url
            .clone()
            .ok_or(ConsumerError::ChainRpcNotConfigured(chain_id)),
        31337 => env
            .local_intuition_rpc_url
            .clone()
            .ok_or(ConsumerError::ChainRpcNotConfigured(chain_id)),
        80002 => env
            .polygon_amoy_rpc_url
            .clone()
            .ok_or(ConsumerError::ChainRpcNotConfigured(chain_id)),
        _ => Err(ConsumerError::UnsupportedChain(chain_id)),
    }
}

/// Creates a configured HTTP client with timeout
fn create_http_client() -> Result<reqwest::Client, ConsumerError> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
        .map_err(ConsumerError::from)
}

/// Validates that a URL does not point to internal/private networks (SSRF protection).
///
/// Delegates to the canonical implementation in `shared_utils::ssrf` and maps
/// its errors onto the consumer's error type so existing call sites and error
/// handling are unchanged.
fn validate_url_not_internal(url_str: &str) -> Result<(), ConsumerError> {
    shared_utils::ssrf::validate_url_not_internal(url_str).map_err(|err| match err {
        shared_utils::error::LibError::SsrfBlocked(url) => ConsumerError::SsrfBlocked(url),
        // Unparseable URL or any other validation failure: surface as an
        // unsupported token URI, matching prior behaviour.
        _ => ConsumerError::UnsupportedTokenUri(url_str.to_string()),
    })
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

    Ok(AtomMetadata::caip22(name, image, Some(parsed.token_id)))
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
/// Includes SSRF protection, request timeouts, and response size limits.
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
        let bytes = response.bytes().await?;
        if bytes.len() > MAX_RESPONSE_SIZE {
            return Err(ConsumerError::ResponseTooLarge(bytes.len(), MAX_RESPONSE_SIZE));
        }
        let text = String::from_utf8(bytes.to_vec())?;
        return serde_json::from_str(&text).map_err(ConsumerError::from);
    }

    // Handle data URIs (base64 encoded JSON)
    if let Some(base64_data) = token_uri.strip_prefix("data:application/json;base64,") {
        debug!("Decoding CAIP-22 metadata from base64 data URI");
        let decoded = base64_decode_with_limit(base64_data)?;
        let json_str = String::from_utf8(decoded)?;
        return serde_json::from_str(&json_str).map_err(ConsumerError::from);
    }

    // Handle raw JSON data URIs
    if let Some(json_data) = token_uri.strip_prefix("data:application/json,") {
        debug!("Parsing CAIP-22 metadata from raw JSON data URI");
        if json_data.len() > MAX_RESPONSE_SIZE {
            return Err(ConsumerError::ResponseTooLarge(json_data.len(), MAX_RESPONSE_SIZE));
        }
        return serde_json::from_str(json_data).map_err(ConsumerError::from);
    }

    // Handle HTTP(S) URLs with SSRF protection
    if token_uri.starts_with("http://") || token_uri.starts_with("https://") {
        debug!("Fetching CAIP-22 metadata from HTTP: {}", token_uri);

        // SSRF protection: validate URL before fetching
        validate_url_not_internal(token_uri)?;

        let client = create_http_client()?;
        let response = client.get(token_uri).send().await?;

        // Check content-length header if available
        if let Some(content_length) = response.content_length() {
            if content_length as usize > MAX_RESPONSE_SIZE {
                return Err(ConsumerError::ResponseTooLarge(content_length as usize, MAX_RESPONSE_SIZE));
            }
        }

        let bytes = response.bytes().await?;
        if bytes.len() > MAX_RESPONSE_SIZE {
            return Err(ConsumerError::ResponseTooLarge(bytes.len(), MAX_RESPONSE_SIZE));
        }

        let text = String::from_utf8(bytes.to_vec())?;
        return serde_json::from_str(&text).map_err(ConsumerError::from);
    }

    warn!("Unsupported token URI format: {}", token_uri);
    Err(ConsumerError::UnsupportedTokenUri(token_uri.to_string()))
}

/// Base64 decoding helper with size limit validation
fn base64_decode_with_limit(input: &str) -> Result<Vec<u8>, ConsumerError> {
    // Check input length before attempting to decode
    if input.len() > MAX_BASE64_INPUT_LENGTH {
        return Err(ConsumerError::ResponseTooLarge(input.len(), MAX_BASE64_INPUT_LENGTH));
    }

    use base64::{Engine, engine::general_purpose::STANDARD};
    let decoded = STANDARD
        .decode(input)
        .map_err(|e| ConsumerError::DecodingError(e.to_string()))?;

    // Also check decoded size
    if decoded.len() > MAX_RESPONSE_SIZE {
        return Err(ConsumerError::ResponseTooLarge(decoded.len(), MAX_RESPONSE_SIZE));
    }

    Ok(decoded)
}

/// Simple base64 decoding helper (for tests)
#[cfg(test)]
fn base64_decode(input: &str) -> Result<Vec<u8>, ConsumerError> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD
        .decode(input)
        .map_err(|e| ConsumerError::DecodingError(e.to_string()))
}

/// Fetches tokenURI directly from a contract given RPC URL and parsed CAIP-22 data.
/// This is useful for testing without the full ResolverConsumerContext.
pub async fn fetch_token_uri_direct(
    rpc_url: &str,
    contract_address: &str,
    token_id: &str,
) -> Result<String, ConsumerError> {
    let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);
    let dyn_provider = DynProvider::new(provider);

    let address =
        Address::from_str(contract_address).map_err(|e| ConsumerError::AddressParse(e.to_string()))?;

    let contract = IERC721MetadataInstance::new(address, dyn_provider);
    let token_id_u256 = U256::from_str(token_id).map_err(ConsumerError::UintParse)?;

    contract.tokenURI(token_id_u256).call().await.map_err(|e| {
        warn!(
            "Failed to fetch tokenURI for contract={}, token_id={}: {}",
            contract_address, token_id, e
        );
        ConsumerError::Alloy(e)
    })
}

/// Fetches metadata JSON from a token URI (HTTP/HTTPS only, for testing).
/// Includes SSRF protection, request timeouts, and response size limits.
pub async fn fetch_metadata_from_http(token_uri: &str) -> Result<Value, ConsumerError> {
    if !token_uri.starts_with("http://") && !token_uri.starts_with("https://") {
        return Err(ConsumerError::UnsupportedTokenUri(token_uri.to_string()));
    }

    // SSRF protection
    validate_url_not_internal(token_uri)?;

    let client = create_http_client()?;
    let response = client.get(token_uri).send().await?;

    // Check content-length header if available
    if let Some(content_length) = response.content_length() {
        if content_length as usize > MAX_RESPONSE_SIZE {
            return Err(ConsumerError::ResponseTooLarge(content_length as usize, MAX_RESPONSE_SIZE));
        }
    }

    let bytes = response.bytes().await?;
    if bytes.len() > MAX_RESPONSE_SIZE {
        return Err(ConsumerError::ResponseTooLarge(bytes.len(), MAX_RESPONSE_SIZE));
    }

    let text = String::from_utf8(bytes.to_vec())?;
    serde_json::from_str(&text).map_err(ConsumerError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::metadata::parse_caip22;

    /// Integration test for Base Sepolia NFT
    /// Contract: 0x8004AA63c570c570eBF15376c0dB199918BFe9Fb
    /// Token ID: 1563
    #[tokio::test]
    async fn test_fetch_real_nft_base_sepolia() {
        let caip22_str = "caip22:eip155:84532/erc721:0x8004AA63c570c570eBF15376c0dB199918BFe9Fb/1563";
        let parsed = parse_caip22(caip22_str).expect("Should parse CAIP-22");

        assert_eq!(parsed.chain_id, 84532);
        assert_eq!(
            parsed.contract_address,
            "0x8004AA63c570c570eBF15376c0dB199918BFe9Fb"
        );
        assert_eq!(parsed.token_id, "1563");

        // Use public Base Sepolia RPC
        let rpc_url = "https://sepolia.base.org";

        let token_uri = fetch_token_uri_direct(rpc_url, &parsed.contract_address, &parsed.token_id)
            .await
            .expect("Should fetch tokenURI from Base Sepolia");

        println!("Base Sepolia tokenURI: {}", token_uri);
        assert!(!token_uri.is_empty(), "tokenURI should not be empty");

        // Fetch metadata if it's an HTTP URL
        if token_uri.starts_with("http") {
            let metadata = fetch_metadata_from_http(&token_uri)
                .await
                .expect("Should fetch metadata from HTTP");

            println!("Base Sepolia metadata: {}", metadata);

            // Check that we got valid JSON with expected fields
            assert!(
                metadata.is_object(),
                "Metadata should be a JSON object"
            );

            if let Some(name) = metadata.get("name") {
                println!("NFT Name: {}", name);
            }
            if let Some(image) = metadata.get("image") {
                println!("NFT Image: {}", image);
            }
        }
    }

    /// Integration test for Ethereum Sepolia NFT
    /// Contract: 0x8004a6090Cd10A7288092483047B097295Fb8847
    /// Token ID: 3265
    ///
    /// Note: This test is ignored by default because public Sepolia RPCs can be unreliable.
    /// Run with: cargo test -p consumer -- test_fetch_real_nft_ethereum_sepolia --ignored --nocapture
    /// Or set ETHEREUM_SEPOLIA_RPC_URL env var to use a reliable RPC.
    #[tokio::test]
    #[ignore = "Requires reliable Ethereum Sepolia RPC - run manually with --ignored"]
    async fn test_fetch_real_nft_ethereum_sepolia() {
        let caip22_str =
            "caip22:eip155:11155111/erc721:0x8004a6090Cd10A7288092483047B097295Fb8847/3265";
        let parsed = parse_caip22(caip22_str).expect("Should parse CAIP-22");

        assert_eq!(parsed.chain_id, 11155111);
        assert_eq!(
            parsed.contract_address,
            "0x8004a6090Cd10A7288092483047B097295Fb8847"
        );
        assert_eq!(parsed.token_id, "3265");

        // Try to use env var first, fallback to public RPC
        let rpc_url = std::env::var("ETHEREUM_SEPOLIA_RPC_URL")
            .unwrap_or_else(|_| "https://ethereum-sepolia-rpc.publicnode.com".to_string());

        let token_uri = fetch_token_uri_direct(rpc_url.as_str(), &parsed.contract_address, &parsed.token_id)
            .await
            .expect("Should fetch tokenURI from Ethereum Sepolia");

        println!("Ethereum Sepolia tokenURI: {}", token_uri);
        assert!(!token_uri.is_empty(), "tokenURI should not be empty");

        // Fetch metadata if it's an HTTP URL
        if token_uri.starts_with("http") {
            let metadata = fetch_metadata_from_http(&token_uri)
                .await
                .expect("Should fetch metadata from HTTP");

            println!("Ethereum Sepolia metadata: {}", metadata);

            // Check that we got valid JSON with expected fields
            assert!(
                metadata.is_object(),
                "Metadata should be a JSON object"
            );

            if let Some(name) = metadata.get("name") {
                println!("NFT Name: {}", name);
            }
            if let Some(image) = metadata.get("image") {
                println!("NFT Image: {}", image);
            }
        }
    }

    /// Test metadata field extraction
    #[test]
    fn test_extract_metadata_fields() {
        let metadata: Value = serde_json::json!({
            "name": "Test NFT",
            "image": "https://example.com/image.png",
            "description": "A test NFT"
        });

        let (name, image) = extract_metadata_fields(&metadata);
        assert_eq!(name, Some("Test NFT".to_string()));
        assert_eq!(image, Some("https://example.com/image.png".to_string()));

        // Test with missing fields
        let empty_metadata: Value = serde_json::json!({});
        let (name2, image2) = extract_metadata_fields(&empty_metadata);
        assert_eq!(name2, None);
        assert_eq!(image2, None);
    }

    /// Test base64 decoding
    #[test]
    fn test_base64_decode() {
        // "Hello World" in base64
        let encoded = "SGVsbG8gV29ybGQ=";
        let decoded = base64_decode(encoded).expect("Should decode base64");
        assert_eq!(String::from_utf8(decoded).unwrap(), "Hello World");
    }

    /// Test SSRF protection - blocks localhost
    #[test]
    fn test_ssrf_blocks_localhost() {
        assert!(validate_url_not_internal("http://localhost/metadata.json").is_err());
        assert!(validate_url_not_internal("http://127.0.0.1/metadata.json").is_err());
        assert!(validate_url_not_internal("http://[::1]/metadata.json").is_err());
    }

    /// Test SSRF protection - blocks private IPs
    #[test]
    fn test_ssrf_blocks_private_ips() {
        // 10.0.0.0/8
        assert!(validate_url_not_internal("http://10.0.0.1/metadata.json").is_err());
        assert!(validate_url_not_internal("http://10.255.255.255/metadata.json").is_err());

        // 172.16.0.0/12
        assert!(validate_url_not_internal("http://172.16.0.1/metadata.json").is_err());
        assert!(validate_url_not_internal("http://172.31.255.255/metadata.json").is_err());

        // 192.168.0.0/16
        assert!(validate_url_not_internal("http://192.168.0.1/metadata.json").is_err());
        assert!(validate_url_not_internal("http://192.168.255.255/metadata.json").is_err());

        // Link-local
        assert!(validate_url_not_internal("http://169.254.169.254/metadata.json").is_err());
    }

    /// Test SSRF protection - blocks internal hostnames
    #[test]
    fn test_ssrf_blocks_internal_hostnames() {
        assert!(validate_url_not_internal("http://myservice.local/metadata.json").is_err());
        assert!(validate_url_not_internal("http://internal.internal/metadata.json").is_err());
        assert!(validate_url_not_internal("http://test.localhost/metadata.json").is_err());
        assert!(validate_url_not_internal("http://metadata.google.internal/metadata.json").is_err());
    }

    /// Test SSRF protection - allows public URLs
    #[test]
    fn test_ssrf_allows_public_urls() {
        assert!(validate_url_not_internal("https://example.com/metadata.json").is_ok());
        assert!(validate_url_not_internal("https://ipfs.io/ipfs/QmTest").is_ok());
        assert!(validate_url_not_internal("https://arweave.net/test").is_ok());
        assert!(validate_url_not_internal("http://8.8.8.8/test").is_ok());
    }

    /// Test private IP detection
    #[test]
    fn test_is_private_ip() {
        use shared_utils::ssrf::is_private_ip;
        use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

        // Private IPv4
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1))));

        // Public IPv4
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));

        // Private IPv6
        assert!(is_private_ip(&IpAddr::V6(Ipv6Addr::LOCALHOST)));
        assert!(is_private_ip(&IpAddr::V6(Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 1))));
        assert!(is_private_ip(&IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1))));
    }

    /// Test base64 size limit
    #[test]
    fn test_base64_decode_size_limit() {
        // Small input should work
        let small_input = "SGVsbG8="; // "Hello"
        assert!(base64_decode_with_limit(small_input).is_ok());

        // Very large input should fail
        let large_input = "A".repeat(MAX_BASE64_INPUT_LENGTH + 1);
        let result = base64_decode_with_limit(&large_input);
        assert!(matches!(result, Err(ConsumerError::ResponseTooLarge(_, _))));
    }
}
