use crate::{
    error::ConsumerError,
    mode::{
        ipfs_upload::types::IpfsUploadMessage, resolver::ens_resolver::Ens,
        types::ResolverConsumerContext,
    },
};
use alloy::{
    primitives::{Address, FixedBytes, keccak256},
    providers::{DynProvider, ProviderBuilder},
    sol,
};
use alloy_network::Ethereum;
use tracing::{debug, warn};

sol! {
    #[sol(rpc)]
    interface TNSRegistry {
        function resolver(bytes32 node) external view returns (address);
    }

    #[sol(rpc)]
    interface TNSResolver {
        function name(bytes32 node) external view returns (string);
        function text(bytes32 node, string key) external view returns (string);
    }
}

/// TNS registry contract on the Intuition (Caldera) chain.
const TNS_REGISTRY_ADDRESS: &str = "0x3220B4EDbA3a1661F02f1D8D241DBF55EDcDa09e";
/// RPC endpoint for the Intuition (Caldera) chain where TNS lives.
const INTUITION_RPC_URL: &str = "https://intuition.calderachain.xyz";

type TnsRegistry = TNSRegistry::TNSRegistryInstance<DynProvider, Ethereum>;
type TnsResolver = TNSResolver::TNSResolverInstance<DynProvider, Ethereum>;

/// Trust Name Service (TNS) resolution.
///
/// Mirrors [`Ens`]: reverse-resolves an address to its primary `.trust` name and
/// avatar, uploads the avatar to IPFS as a side effect, and returns an [`Ens`] so
/// the pipeline treats TNS and ENS results uniformly.
pub struct Tns;

impl Tns {
    /// Resolves the TNS primary name and avatar for an address.
    ///
    /// TNS is attempted before ENS in `process_account`.
    pub async fn get_tns(
        address: Address,
        consumer_context: &ResolverConsumerContext,
    ) -> Result<Ens, ConsumerError> {
        let registry = Self::registry()?;
        let name = Self::get_tns_name(address, &registry).await;
        let mut image = None;
        if let Some(name) = &name {
            image = Self::get_tns_avatar(name, &registry, consumer_context).await?;
        }
        Ok(Ens { name, image })
    }

    /// Gets the avatar text record for a name and forwards it to the IPFS upload
    /// consumer. Mirrors [`Ens::get_ens_avatar`].
    async fn get_tns_avatar(
        name: &str,
        registry: &TnsRegistry,
        consumer_context: &ResolverConsumerContext,
    ) -> Result<Option<String>, ConsumerError> {
        let node = Self::namehash(&Self::ensure_trust_suffix(name));
        let avatar = match Self::get_resolver(node, registry).await {
            Some(resolver) => resolver
                .text(node, "avatar".to_string())
                .call()
                .await
                .ok()
                .filter(|s| !s.is_empty()),
            None => None,
        };

        if let Some(url) = &avatar {
            debug!("Sending image to IPFS upload consumer: {}", url);
            consumer_context
                .client
                .send_message(
                    serde_json::to_string(&IpfsUploadMessage { image: url.clone() })?,
                    None,
                )
                .await?;
        }
        Ok(avatar)
    }

    /// Reverse-resolves an address to its primary TNS name.
    ///
    /// Returns `None` if no name is set or the lookup fails for any reason. Never
    /// blocks message processing — mirrors [`Ens::get_ens_name`], which swallows
    /// lookup errors so resolution falls back to ENS.
    pub async fn get_tns_name(address: Address, registry: &TnsRegistry) -> Option<String> {
        let reverse_name = format!(
            "{}.addr.reverse",
            address.to_string().to_lowercase().trim_start_matches("0x")
        );
        let node = Self::namehash(&reverse_name);
        let resolver = Self::get_resolver(node, registry).await?;
        match resolver.name(node).call().await {
            Ok(name) if !name.is_empty() => {
                debug!("Resolved TNS name for {}: {}", address, name);
                Some(name)
            }
            Ok(_) => None,
            Err(e) => {
                warn!("TNS reverse lookup failed for {} (non-fatal): {}", address, e);
                None
            }
        }
    }

    /// Builds the alloy client for the TNS registry on the Intuition (Caldera) chain.
    fn registry() -> Result<TnsRegistry, ConsumerError> {
        let provider =
            DynProvider::new(ProviderBuilder::new().connect_http(INTUITION_RPC_URL.parse()?));
        let address = TNS_REGISTRY_ADDRESS
            .parse::<Address>()
            .map_err(|e| ConsumerError::AddressParse(e.to_string()))?;
        Ok(TNSRegistry::TNSRegistryInstance::new(address, provider))
    }

    /// Looks up the resolver contract for a node hash. Returns `None` if unset or
    /// if the lookup fails (non-fatal — resolution falls back to ENS).
    async fn get_resolver(node: FixedBytes<32>, registry: &TnsRegistry) -> Option<TnsResolver> {
        match registry.resolver(node).call().await {
            Ok(address) if address != Address::ZERO => {
                Some(TNSResolver::TNSResolverInstance::new(
                    address,
                    registry.provider().clone(),
                ))
            }
            Ok(_) => None,
            Err(e) => {
                warn!("TNS resolver lookup failed (non-fatal): {}", e);
                None
            }
        }
    }

    /// Ensures a name carries the `.trust` TLD before hashing.
    fn ensure_trust_suffix(name: &str) -> String {
        if name.ends_with(".trust") {
            name.to_string()
        } else {
            format!("{name}.trust")
        }
    }

    /// Namehash of a name (EIP-137).
    fn namehash(name: &str) -> FixedBytes<32> {
        if name.is_empty() {
            return FixedBytes::ZERO;
        }
        let mut hash = vec![0u8; 32];
        for label in name.rsplit('.') {
            hash.append(&mut keccak256(label.as_bytes()).to_vec());
            hash = keccak256(hash.as_slice()).to_vec();
        }
        FixedBytes::from_slice(hash.as_slice())
    }
}
