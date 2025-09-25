use crate::{
    ENSName::ENSNameInstance,
    ENSRegistry::ENSRegistryInstance,
    error::ConsumerError,
    mode::{ipfs_upload::types::IpfsUploadMessage, types::ResolverConsumerContext},
};
use alloy::{
    primitives::{Address, FixedBytes, keccak256},
    providers::DynProvider,
};
use alloy_network::Ethereum;
use tracing::debug;

/// This struct represents the ENS name and avatar for an address.
#[derive(Clone, Debug)]
pub struct Ens {
    pub name: Option<String>,
    pub image: Option<String>,
    pub twitter: Option<String>,
    pub discord: Option<String>,
    pub github: Option<String>,
    pub telegram: Option<String>,
    pub email: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub location: Option<String>,
    pub real_name: Option<String>,
}

impl Ens {
    /// This function hashes the ENS name.
    fn namehash(name: &str) -> Vec<u8> {
        if name.is_empty() {
            return vec![0u8; 32];
        }
        let mut hash = vec![0u8; 32];
        for label in name.rsplit('.') {
            hash.append(&mut keccak256(label.as_bytes()).to_vec());
            hash = keccak256(hash.as_slice()).to_vec();
        }
        hash
    }

    /// This function gets the ENS name and avatar for an address.
    pub async fn get_ens(
        address: Address,
        consumer_context: &ResolverConsumerContext,
    ) -> Result<Ens, ConsumerError> {
        let name = Self::get_ens_name(address, &consumer_context.mainnet_client).await?;
        let mut image = None;
        if let Some(name_str) = &name {
            image = Self::get_ens_avatar(name_str, consumer_context).await?;
        }

        // Get social fields if we have an ENS name
        let mut twitter = None;
        let mut discord = None;
        let mut github = None;
        let mut telegram = None;
        let mut email = None;
        let mut description = None;
        let mut url = None;
        let mut location = None;
        let mut real_name = None;

        if name.is_some() {
            // Fetch all social fields in parallel for better performance
            let (
                twitter_result,
                discord_result,
                github_result,
                telegram_result,
                email_result,
                description_result,
                url_result,
                location_result,
                name_result,
            ) = tokio::try_join!(
                Self::get_ens_description(address, "com.twitter", &consumer_context.mainnet_client),
                Self::get_ens_description(address, "com.discord", &consumer_context.mainnet_client),
                Self::get_ens_description(address, "com.github", &consumer_context.mainnet_client),
                Self::get_ens_description(
                    address,
                    "org.telegram",
                    &consumer_context.mainnet_client
                ),
                Self::get_ens_description(address, "email", &consumer_context.mainnet_client),
                Self::get_ens_description(address, "description", &consumer_context.mainnet_client),
                Self::get_ens_description(address, "url", &consumer_context.mainnet_client),
                Self::get_ens_description(address, "location", &consumer_context.mainnet_client),
                Self::get_ens_description(address, "name", &consumer_context.mainnet_client)
            )?;

            twitter = twitter_result;
            discord = discord_result;
            github = github_result;
            telegram = telegram_result;
            email = email_result;
            description = description_result;
            url = url_result;
            location = location_result;
            real_name = name_result;
        }

        Ok(Ens {
            name,
            image,
            twitter,
            discord,
            github,
            telegram,
            email,
            description,
            url,
            location,
            real_name,
        })
    }

    /// Gets the ENS avatar URL for a given name
    async fn get_ens_avatar(
        name: &str,
        consumer_context: &ResolverConsumerContext,
    ) -> Result<Option<String>, ConsumerError> {
        let url = format!("https://metadata.ens.domains/mainnet/avatar/{}", name);
        match reqwest::get(&url).await {
            Ok(response) => {
                if response.status() == 200 {
                    debug!("Sending image to IPFS upload consumer: {}", url);
                    consumer_context
                        .client
                        .send_message(
                            serde_json::to_string(&IpfsUploadMessage { image: url.clone() })?,
                            None,
                        )
                        .await?;
                    Ok(Some(url))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }

    /// This function gets the ENS name for an address.
    pub async fn get_ens_name(
        address: Address,
        mainnet_client: &ENSRegistryInstance<DynProvider, Ethereum>,
    ) -> Result<Option<String>, ConsumerError> {
        debug!("Getting ENS name for {}", address);
        let address_hash = Self::namehash(&Self::prepare_name(address));
        let resolver_address =
            Self::get_resolver_address(address, &address_hash, mainnet_client).await?;

        if resolver_address != Address::ZERO {
            let alloy_contract = ENSNameInstance::new(resolver_address, mainnet_client.provider());
            let name = alloy_contract
                .name(FixedBytes::from_slice(address_hash.as_slice()))
                .call()
                .await?;
            debug!("ResolvedENS name: {:?}", name);
            Ok(Some(name))
        } else {
            Ok(None)
        }
    }

    /// This function gets the ENS description for an address.
    pub async fn get_ens_description(
        address: Address,
        key: &str,
        mainnet_client: &ENSRegistryInstance<DynProvider, Ethereum>,
    ) -> Result<Option<String>, ConsumerError> {
        debug!("Getting ENS description {} for {}", key, address);

        // First, get the ENS name for this address using reverse resolution
        let ens_name = Self::get_ens_name(address, mainnet_client).await?;

        if let Some(name) = ens_name {
            debug!("Found ENS name: {} for address: {}", name, address);

            // Now get the social fields using forward resolution on the ENS name
            let name_hash = Self::namehash(&name);
            let resolver_address = mainnet_client
                .resolver(FixedBytes::from_slice(name_hash.as_slice()))
                .call()
                .await?;

            if resolver_address != Address::ZERO {
                debug!(
                    "Found resolver: {} for ENS name: {}",
                    resolver_address, name
                );
                let alloy_contract =
                    ENSNameInstance::new(resolver_address, mainnet_client.provider());
                let value = alloy_contract
                    .text(
                        FixedBytes::from_slice(name_hash.as_slice()),
                        key.to_string(),
                    )
                    .call()
                    .await?;
                debug!("Resolved ENS {} for {}: {:?}", key, name, value);
                Ok(Some(value))
            } else {
                debug!("No resolver found for ENS name: {}", name);
                Ok(None)
            }
        } else {
            debug!("No ENS name found for address: {}", address);
            Ok(None)
        }
    }

    /// This function gets the resolver address for an address hash.
    async fn get_resolver_address(
        address: Address,
        address_hash: &[u8],
        mainnet_client: &ENSRegistryInstance<DynProvider, Ethereum>,
    ) -> Result<Address, ConsumerError> {
        let resolver_address = mainnet_client
            .resolver(FixedBytes::from_slice(address_hash))
            .call()
            .await?;

        if resolver_address == Address::ZERO {
            debug!("No resolver found for {}", address);
        } else {
            debug!("Resolver found for {}: {}", address, resolver_address);
        }

        Ok(resolver_address)
    }

    /// This function prepares the name for the ENS resolver.
    fn prepare_name(address: Address) -> String {
        let addr_str = address.to_string().to_lowercase();
        format!("{}.addr.reverse", addr_str.trim_start_matches("0x"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::providers::ProviderBuilder;

    #[test]
    fn test_namehash() {
        // Test empty string
        assert_eq!(Ens::namehash(""), vec![0u8; 32]);

        // Test "eth"
        let eth_hash = "93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae";
        assert_eq!(hex::encode(Ens::namehash("eth")), eth_hash);

        // Test "foo.eth"
        let foo_eth_hash = "de9b09fd7c5f901e23a3f19fecc54828e9c848539801e86591bd9801b019f84f";
        assert_eq!(hex::encode(Ens::namehash("foo.eth")), foo_eth_hash);

        // Test "alice.eth"
        let alice_eth_hash = "787192fc5378cc32aa956ddfdedbf26b24e8d78e40109add0eea2c1a012c3dec";
        assert_eq!(hex::encode(Ens::namehash("alice.eth")), alice_eth_hash);
    }

    #[tokio::test]
    async fn test_get_ens_name() {
        // Load environment variables from .env file
        dotenvy::dotenv().ok();

        // Test addresses with known ENS names
        let test_addresses = vec![
            "0xB95ca3D3144e9d1DAFF0EE3d35a4488A4A5C9Fc5", // Example address
            "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045", // vitalik.eth
            "0x983110309620D911731Ac0932219af06091b6744", // ens.eth
        ];

        // Create a provider
        // BASE_MAINNET_RPC_PROVIDER: The RPC endpoint URL for Ethereum mainnet blockchain access
        let rpc_url = std::env::var("BASE_MAINNET_RPC_PROVIDER").unwrap();
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse().unwrap());
        let dyn_provider = DynProvider::new(provider);
        let mainnet_client = ENSRegistryInstance::new(
            "0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e"
                .parse()
                .unwrap(),
            dyn_provider,
        );

        println!("Testing ENS name resolution for addresses:");

        for addr_str in test_addresses {
            let test_address: Address = addr_str.parse().unwrap();

            match Ens::get_ens_name(test_address, &mainnet_client).await {
                Ok(Some(name)) => {
                    if name.is_empty() {
                        println!("⚠️  {}: Empty ENS name", test_address);
                    } else {
                        println!("✅ {}: {}", test_address, name);
                    }
                }
                Ok(None) => {
                    println!("❌ {}: No ENS name found", test_address);
                }
                Err(e) => {
                    println!("⚠️  {}: Error - {}", test_address, e);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_get_ens_social_fields() {
        // Load environment variables from .env file
        dotenvy::dotenv().ok();

        // Test addresses with ENS social fields
        let test_addresses = vec![
            "0xB95ca3D3144e9d1DAFF0EE3d35a4488A4A5C9Fc5", // 0xvital.eth
            "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045", // vitalik.eth
        ];

        // Create a provider (you may need to adjust this based on your test setup)
        // BASE_MAINNET_RPC_PROVIDER: The RPC endpoint URL for Ethereum mainnet blockchain access
        let rpc_url = std::env::var("BASE_MAINNET_RPC_PROVIDER").unwrap();
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse().unwrap());
        let dyn_provider = DynProvider::new(provider);
        let mainnet_client = ENSRegistryInstance::new(
            "0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e"
                .parse()
                .unwrap(),
            dyn_provider,
        );

        // Define the fields we want to test
        let fields = vec![
            "description",
            "com.twitter",
            "com.discord",
            "com.github",
            "org.telegram",
            "email",
            "url",
            "location",
            "name",
        ];

        for addr_str in test_addresses {
            let test_address: Address = addr_str.parse().unwrap();
            println!("Testing ENS social fields for address: {}", test_address);

            for field in &fields {
                match Ens::get_ens_description(test_address, field, &mainnet_client).await {
                    Ok(Some(value)) => {
                        if value.is_empty() {
                            println!("⚠️  {}: Empty value", field);
                        } else {
                            println!("✅ {}: {}", field, value);
                        }
                    }
                    Ok(None) => {
                        println!("❌ {}: No value found", field);
                    }
                    Err(e) => {
                        println!("⚠️  {}: Error - {}", field, e);
                    }
                }
            }
            println!(); // Add spacing between addresses
        }
    }
}
