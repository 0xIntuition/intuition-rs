use crate::{
    UniversalResolver::UniversalResolverInstance,
    error::ConsumerError,
    mode::{ipfs_upload::types::IpfsUploadMessage, types::ResolverConsumerContext},
};
use alloy::{
    primitives::{Address, Bytes, U256},
    providers::DynProvider,
    sol_types::SolValue,
};
use alloy_network::Ethereum;
use tracing::{debug, warn};

/// ENS name and avatar for an address.
#[derive(Clone, Debug)]
pub struct Ens {
    pub name: Option<String>,
    pub image: Option<String>,
}

/// Ethereum L1 coin type per SLIP-44 — used with UniversalResolver.reverse().
const COIN_TYPE_ETH: u64 = 60;

impl Ens {
    /// Resolves the ENS primary name and avatar for an address.
    ///
    /// Uses the ENS Universal Resolver (ENSIP-23) which handles L1 primary names,
    /// L2 primary names (Base, Optimism, Linea — via CCIP-Read), and offchain
    /// resolvers in a single call.
    pub async fn get_ens(
        address: Address,
        consumer_context: &ResolverConsumerContext,
    ) -> Result<Ens, ConsumerError> {
        let name = Self::get_ens_name(address, &consumer_context.universal_resolver).await?;
        let mut image = None;
        if let Some(name_str) = &name {
            image = Self::get_ens_avatar(name_str, consumer_context).await?;
        }
        Ok(Ens { name, image })
    }

    /// Gets the ENS avatar URL for a given name.
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

    /// Reverse-resolves an address to its primary ENS name via the Universal Resolver.
    ///
    /// Calls `UniversalResolver.reverse(addr_bytes, 60)` which returns the primary
    /// name regardless of whether it was set on L1 or L2. The RPC provider (Alchemy)
    /// handles CCIP-Read transparently, so this is a single contract call for the
    /// consumer — no client-side OffchainLookup handling needed.
    ///
    /// Returns `Ok(None)` if no primary name is set or if the lookup fails for any
    /// reason (network error, unsupported UR version, etc.). Never panics or blocks
    /// message processing.
    pub async fn get_ens_name(
        address: Address,
        universal_resolver: &UniversalResolverInstance<DynProvider, Ethereum>,
    ) -> Result<Option<String>, ConsumerError> {
        debug!("Getting ENS name for {} via Universal Resolver", address);

        // ABI-encode the address as a 20-byte `bytes` argument
        let lookup_address = Bytes::from(address.abi_encode_packed());
        let coin_type = U256::from(COIN_TYPE_ETH);

        match universal_resolver
            .reverse(lookup_address, coin_type)
            .call()
            .await
        {
            Ok(result) => {
                let name = result.primary;
                if name.is_empty() {
                    debug!("No ENS primary name set for {}", address);
                    Ok(None)
                } else {
                    debug!("Resolved ENS name for {}: {}", address, name);
                    Ok(Some(name))
                }
            }
            Err(e) => {
                // Don't propagate contract call errors — the consumer must keep
                // processing messages. Log the error for observability.
                warn!(
                    "ENS reverse lookup failed for {} (non-fatal, returning None): {}",
                    address, e
                );
                Ok(None)
            }
        }
    }
}
