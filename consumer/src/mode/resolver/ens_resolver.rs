use crate::{
    error::ConsumerError, mode::types::ResolverConsumerContext, ENSName::ENSNameInstance,
    ENSRegistry::ENSRegistryInstance,
};
use alloy::{
    primitives::{keccak256, Address, FixedBytes},
    providers::RootProvider,
    transports::http::Http,
};
use log::info;
use models::image_guard::ImageGuard;
use reqwest::Client;

/// This struct represents the ENS name and avatar for an address.
#[derive(Debug)]
pub struct Ens {
    pub name: Option<String>,
    pub image: Option<String>,
}

impl Ens {
    /// This function downloads an avatar, classifies it and stores it in the database
    pub async fn download_image_classify_and_store(
        url: String,
        consumer_context: &ResolverConsumerContext,
    ) -> Result<(), ConsumerError> {
        // Clone the URL to avoid borrowing issues
        let url_clone = url.clone();
        let (name, extension) = Self::get_name_and_extension(&url_clone);
        // Download image bytes
        let image_bytes = reqwest::get(&url_clone.to_string())
            .await?
            .bytes()
            .await?
            .to_vec();

        // Create multipart form with content type and filename
        let part = reqwest::multipart::Part::bytes(image_bytes)
            .mime_str(&format!("image/{}", extension))?
            .file_name(name); // Add filename
        let form = reqwest::multipart::Form::new().part("file", part);

        // Send request with multipart form
        let endpoint = format!("{}/upload", consumer_context.image_guard_url);
        let client = reqwest::Client::new();

        let response: Vec<ImageGuard> = client
            .post(endpoint)
            .multipart(form)
            .send()
            .await?
            .json()
            .await?;

        info!("Image classification response: {response:?}");
        Ok(())
    }

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
        Ok(Ens { name, image })
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
                    Self::download_image_classify_and_store(url.clone(), consumer_context).await?;
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
        mainnet_client: &ENSRegistryInstance<Http<Client>, RootProvider<Http<Client>>>,
    ) -> Result<Option<String>, ConsumerError> {
        info!("Getting ENS name for {}", address);
        let address_hash = Self::namehash(&Self::prepare_name(address));
        let resolver_address =
            Self::get_resolver_address(address, &address_hash, mainnet_client).await?;

        if resolver_address != Address::ZERO {
            let alloy_contract = ENSNameInstance::new(resolver_address, mainnet_client.provider());
            let name = alloy_contract
                .name(FixedBytes::from_slice(address_hash.as_slice()))
                .call()
                .await?
                ._0;
            info!("ResolvedENS name: {:?}", name);
            Ok(Some(name))
        } else {
            Ok(None)
        }
    }

    /// This function gets the name and extension from a URL.
    fn get_name_and_extension(url: &str) -> (String, String) {
        let filename = url.split('/').last().unwrap_or_default();
        let (name, extension) = filename.split_once('.').unwrap_or_default();
        (name.to_string(), extension.to_string())
    }

    /// This function gets the resolver address for an address hash.
    async fn get_resolver_address(
        address: Address,
        address_hash: &[u8],
        mainnet_client: &ENSRegistryInstance<Http<Client>, RootProvider<Http<Client>>>,
    ) -> Result<Address, ConsumerError> {
        let resolver_address = mainnet_client
            .resolver(FixedBytes::from_slice(address_hash))
            .call()
            .await?
            ._0;

        if resolver_address == Address::ZERO {
            info!("No resolver found for {}", address);
        } else {
            info!("Resolver found for {}: {}", address, resolver_address);
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
}
