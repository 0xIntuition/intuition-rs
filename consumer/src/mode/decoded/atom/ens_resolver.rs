// use alloy::{
//     primitives::{keccak256, Address, Bytes},
//     providers::{Provider, RootProvider},
//     transports::http::Http,
// };
// use reqwest::Client;

// use crate::error::ConsumerError;

// // Function signature for ENS resolver functions
// const RESOLVER_SIGNATURE: &str = "resolver(bytes32)";
// const NAME_SIGNATURE: &str = "name(bytes32)";

// async fn get_ens_name(
//     provider: &RootProvider<Http<Client>>,
//     ens_contract: Address,
//     address: Address,
// ) -> Result<String, ConsumerError> {
//     // Convert address to ENS node (reverse lookup node)
//     let reverse_name = format!("{:x}.addr.reverse", address);
//     let node = keccak256(reverse_name.as_bytes());

//     // Encode resolver(bytes32) call
//     let resolver_data =
//         Bytes::from([&keccak256(RESOLVER_SIGNATURE.as_bytes())[..4], &node[..]].concat());

//     // Get resolver address
//     let resolver: Address = provider
//         .call(alloy::transports::Call {
//             to: Some(ens_contract),
//             data: resolver_data,
//             ..Default::default()
//         })
//         .await?
//         .try_into()
//         .map_err(|e| ConsumerError::Ens(e.to_string()))?;

//     // If resolver is zero address, name not found
//     if resolver == Address::ZERO {
//         return Ok("".to_string());
//     }

//     // Encode name(bytes32) call
//     let name_data = Bytes::from([&keccak256(NAME_SIGNATURE.as_bytes())[..4], &node[..]].concat());

//     // Get name from resolver
//     let result = provider
//         .call(alloy::network::Call {
//             to: Some(resolver),
//             data: name_data,
//             ..Default::default()
//         })
//         .await?;

//     // Decode result (skip first 32 bytes for dynamic string offset)
//     let name =
//         String::from_utf8(result[64..].to_vec()).map_err(|e| ConsumerError::Ens(e.to_string()))?;

//     Ok(name)
// }
