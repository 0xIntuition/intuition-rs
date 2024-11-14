use crate::{error::ConsumerError, ENSName::ENSNameInstance, ENSRegistry::ENSRegistryInstance};
use alloy::{
    primitives::{keccak256, Address, FixedBytes},
    providers::RootProvider,
    transports::http::Http,
};
use log::info;
use reqwest::Client;

/// This struct represents the ENS name and avatar for an address.
#[derive(Debug)]
pub struct Ens {
    pub name: Option<String>,
    pub image: Option<String>,
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
    mainnet_client: &ENSRegistryInstance<Http<Client>, RootProvider<Http<Client>>>,
) -> Result<Ens, ConsumerError> {
    let name = get_ens_name(address, mainnet_client).await?;
    let mut image = None;
    if let Some(name_str) = &name {
        image = get_ens_avatar(name_str).await?;
    }
    Ok(Ens { name, image })
}

/// This function gets the ENS name for an address.
pub async fn get_ens_name(
    address: Address,
    mainnet_client: &ENSRegistryInstance<Http<Client>, RootProvider<Http<Client>>>,
) -> Result<Option<String>, ConsumerError> {
    info!("Getting ENS name for {}", address);
    let addr_str = address.to_string().to_lowercase();
    let name = format!("{}.addr.reverse", addr_str.trim_start_matches("0x"));

    let resolver_address = mainnet_client
        .resolver(FixedBytes::from_slice(namehash(&name).as_slice()))
        .call()
        .await?
        ._0;

    if resolver_address == Address::ZERO {
        info!("No resolver found for {}", address);
        return Ok(None);
    }

    info!("Resolver found for {}: {}", address, resolver_address);

    let alloy_contract = ENSNameInstance::new(resolver_address, mainnet_client.provider());

    let name = alloy_contract
        .name(FixedBytes::from_slice(namehash(&name).as_slice()))
        .call()
        .await?
        ._0;

    info!("ResolvedENS name: {:?}", name);

    Ok(Some(name))
}

/// Gets the ENS avatar URL for a given name
pub async fn get_ens_avatar(name: &str) -> Result<Option<String>, ConsumerError> {
    let url = format!("https://metadata.ens.domains/mainnet/avatar/{}", name);

    match reqwest::get(&url).await {
        Ok(response) => {
            if response.status() == 200 {
                Ok(Some(url))
            } else {
                Ok(None)
            }
        }
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namehash() {
        // Test empty string
        assert_eq!(namehash(""), vec![0u8; 32]);

        // Test "eth"
        let eth_hash = "93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae";
        assert_eq!(hex::encode(namehash("eth")), eth_hash);

        // Test "foo.eth"
        let foo_eth_hash = "de9b09fd7c5f901e23a3f19fecc54828e9c848539801e86591bd9801b019f84f";
        assert_eq!(hex::encode(namehash("foo.eth")), foo_eth_hash);

        // Test "alice.eth"
        let alice_eth_hash = "787192fc5378cc32aa956ddfdedbf26b24e8d78e40109add0eea2c1a012c3dec";
        assert_eq!(hex::encode(namehash("alice.eth")), alice_eth_hash);
    }
}
