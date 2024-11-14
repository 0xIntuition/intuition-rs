use std::str::FromStr;

use alloy::{
    contract::{ContractInstance, Interface},
    json_abi::{Function, JsonAbi, Param, StateMutability},
    network::Ethereum,
    primitives::{keccak256, Address, Bytes, FixedBytes, TxKind},
    providers::{Provider, ProviderBuilder, RootProvider},
    rpc::types::{TransactionInput, TransactionRequest},
    transports::http::Http,
};
use alloy_ccip_read::CCIPReader;
use log::info;
use reqwest::Client;

use crate::{
    error::ConsumerError,
    D3Connect::{self, D3ConnectInstance},
    ENSRegistry::ENSRegistryInstance,
};

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
    // let node = hex::encode(namehash(&name));

    let resolver_address = mainnet_client
        .resolver(FixedBytes::from_slice(namehash(&name).as_slice()))
        .call()
        .await?
        ._0;

    info!("Resolver address: {:?}", resolver_address);
    if resolver_address == Address::ZERO {
        info!("No resolver found for {}", address);
        return Ok(None);
    }

    info!("Resolver found for {}: {}", address, resolver_address);

    let provider = ProviderBuilder::new()
        .on_http("https://eth-mainnet.g.alchemy.com/v2/qqDU-DK2v3BXXNVwrIs4oECQXQMPKXW0".parse()?);

    let alloy_contract = D3ConnectInstance::new(resolver_address, provider.clone());

    let name = alloy_contract
        .name(FixedBytes::from_slice(namehash(&name).as_slice()))
        .call()
        .await?
        ._0;
    println!("Name: {:?}", name);
    // let reader = CCIPReader::new(provider.boxed());

    // let call = D3Connect::reverseResolveCall {
    //     addr: resolver_address,
    //     network: "".into(),
    // };

    // println!("Caling ccip");

    // let name = alloy_ccip_read::ens::query_resolver_non_wildcarded(&reader, resolver_address, call)
    //     .await
    //     .map_err(|e| ConsumerError::Ens(e.to_string()))?
    //     .map(|name| name._0);

    // ccip_read_used |= name.ccip_read_used();

    // let name = alloy_ccip_read::ens::query_resolver_non_wildcarded(
    //     &reader,
    //     resolver_address,
    //     namehash(&name),
    // )
    // .await
    // .map_err(|e| ConsumerError::Ens(e.to_string()))?;

    // let contract: ContractInstance<Http<Client>, _, Ethereum> = ContractInstance::new(
    //     resolver_address,
    //     Interface::new(JsonAbi::new(vec![Function {
    //         name: "name".to_string(),
    //         inputs: vec![Param {
    //             name: "bytes32".to_string(),
    //             components: vec![],
    //             internal_type: None,
    //             ty: "bytes32".to_string(),
    //         }],
    //         outputs: vec![Param {
    //             name: "string".to_string(),
    //             components: vec![],
    //             internal_type: None,
    //             ty: "string".to_string(),
    //         }],
    //         constant: None,
    //         state_mutability: StateMutability::View,
    //     }])),
    //     provider,
    // );

    // // let name = mainnet_client.(resolver_address, node).call().await?._0;

    // Create transaction request to call 'name(bytes32)' function
    // Takes the first 4 bytes of the keccak256 hash of "name(bytes32)" to create the function selector
    // This is how Ethereum identifies which function to call on a contract
    // let name_selector = keccak256(b"name(bytes32)")[..4].to_vec();
    // let call_data = [name_selector, namehash(&name)].concat();

    // let request = TransactionRequest::default()
    //     .to(resolver_address)
    //     .input(TransactionInput::new(Bytes::from(call_data)));

    // let name = provider
    //     .send_transaction(request)
    //     .await
    //     .map_err(|e| ConsumerError::Ens(e.to_string()))?
    //     .get_receipt()
    //     .await
    //     .map_err(|e| ConsumerError::Ens(e.to_string()))?;

    info!("Name: {:?}", name);
    // Decode the result - ENS names are ABI-encoded strings
    // if name.serialize()?.to_string().is_empty() {
    //     return Ok(None);
    // }

    // Skip first 32 bytes (length offset) and next 32 bytes (string length)
    // let name_str =
    //     String::from_utf8(name.[64..].to_vec()).map_err(|e| ConsumerError::Ens(e.to_string()))?;
    let name_str = name;

    Ok(Some(name_str))
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
