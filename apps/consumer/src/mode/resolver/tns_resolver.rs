use alloy::{
    primitives::{Address, FixedBytes, keccak256},
    providers::DynProvider,
    sol,
};
use alloy_network::Ethereum;
use tracing::{debug, warn};

sol! {
    #[sol(rpc)]
    interface TNSRegistry {
        function resolver(bytes32 node) external view returns (address);
        function owner(bytes32 node) external view returns (address);
    }

    #[sol(rpc)]
    interface TNSResolver {
        function addr(bytes32 node) external view returns (address);
        function name(bytes32 node) external view returns (string);
        function text(bytes32 node, string key) external view returns (string);
        function contenthash(bytes32 node) external view returns (bytes);
    }
}

pub const TNS_REGISTRY_ADDRESS: &str = "0x3220B4EDbA3a1661F02f1D8D241DBF55EDcDa09e";
pub const INTUITION_RPC_URL: &str = "https://intuition.calderachain.xyz";

#[derive(Clone, Debug)]
pub struct Tns {
    pub name: Option<String>,
    pub address: Option<Address>,
    pub avatar: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum TnsError {
    #[error("Contract call failed: {0}")]
    ContractError(String),
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
}

impl From<TnsError> for crate::error::ConsumerError {
    fn from(e: TnsError) -> Self {
        crate::error::ConsumerError::TnsError(e.to_string())
    }
}

impl Tns {
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

    /// Ensures the name ends with `.trust`
    fn ensure_trust_suffix(name: &str) -> String {
        if name.ends_with(".trust") {
            name.to_string()
        } else {
            format!("{}.trust", name)
        }
    }

    /// Looks up the resolver contract for a given node hash.
    /// Returns `None` if no resolver is set (address is zero).
    async fn get_resolver(
        node_bytes: FixedBytes<32>,
        registry: &TNSRegistry::TNSRegistryInstance<DynProvider, Ethereum>,
    ) -> Result<Option<TNSResolver::TNSResolverInstance<DynProvider, Ethereum>>, TnsError> {
        let resolver_address = registry
            .resolver(node_bytes)
            .call()
            .await
            .map_err(|e| TnsError::ContractError(e.to_string()))?;

        if resolver_address == Address::ZERO {
            return Ok(None);
        }

        Ok(Some(TNSResolver::TNSResolverInstance::new(
            resolver_address,
            registry.provider().clone(),
        )))
    }

    /// Computes the node hash for a name
    fn node_for_name(name: &str) -> FixedBytes<32> {
        let full_name = Self::ensure_trust_suffix(name);
        let node = Self::namehash(&full_name);
        FixedBytes::from_slice(node.as_slice())
    }

    /// Forward resolves a TNS name to an address and avatar
    pub async fn resolve_name(
        name: &str,
        registry: &TNSRegistry::TNSRegistryInstance<DynProvider, Ethereum>,
    ) -> Result<Tns, TnsError> {
        let full_name = Self::ensure_trust_suffix(name);
        let node_bytes = Self::node_for_name(&full_name);

        let resolver = match Self::get_resolver(node_bytes, registry).await? {
            Some(r) => r,
            None => {
                debug!("No resolver found for {full_name}");
                return Ok(Tns {
                    name: None,
                    address: None,
                    avatar: None,
                });
            }
        };

        let addr = resolver
            .addr(node_bytes)
            .call()
            .await
            .map_err(|e| TnsError::ContractError(e.to_string()))?;

        let avatar = resolver
            .text(node_bytes, "avatar".to_string())
            .call()
            .await
            .ok()
            .filter(|s| !s.is_empty());

        debug!("Resolved {full_name}: addr={addr}, avatar={avatar:?}");

        Ok(Tns {
            name: Some(full_name),
            address: if addr != Address::ZERO {
                Some(addr)
            } else {
                None
            },
            avatar,
        })
    }

    /// Reverse resolves an address to a TNS name and avatar
    pub async fn reverse_resolve(
        address: Address,
        registry: &TNSRegistry::TNSRegistryInstance<DynProvider, Ethereum>,
    ) -> Result<Tns, TnsError> {
        debug!("Reverse resolving TNS name for {address}");

        let reverse_name = Self::prepare_reverse_name(address);
        let node = Self::namehash(&reverse_name);
        let node_bytes = FixedBytes::from_slice(node.as_slice());

        let resolver = match Self::get_resolver(node_bytes, registry).await? {
            Some(r) => r,
            None => {
                debug!("No reverse resolver found for {address}");
                return Ok(Tns {
                    name: None,
                    address: Some(address),
                    avatar: None,
                });
            }
        };

        let name = resolver
            .name(node_bytes)
            .call()
            .await
            .map_err(|e| TnsError::ContractError(e.to_string()))?;

        if name.is_empty() {
            return Ok(Tns {
                name: None,
                address: Some(address),
                avatar: None,
            });
        }

        // Try fetching avatar from multiple locations:
        // 1. Reverse resolver with reverse node
        // 2. Reverse resolver with forward name's node (where TNS commonly stores it)
        // 3. Forward name's own resolver
        let avatar = match resolver
            .text(node_bytes, "avatar".to_string())
            .call()
            .await
            .ok()
            .filter(|s| !s.is_empty())
        {
            Some(avatar) => Some(avatar),
            None => {
                // Try forward name's node on the reverse resolver
                let fwd_node = Self::node_for_name(&name);
                match resolver
                    .text(fwd_node, "avatar".to_string())
                    .call()
                    .await
                    .ok()
                    .filter(|s| !s.is_empty())
                {
                    Some(avatar) => Some(avatar),
                    None => {
                        // Fall back to forward name's own resolver
                        match Self::get_text_record(&name, "avatar", registry).await {
                            Ok(avatar) => avatar,
                            Err(e) => {
                                warn!("Failed to fetch avatar for {name}: {e}");
                                None
                            }
                        }
                    }
                }
            }
        };

        debug!("Reverse resolved {address}: name={name}, avatar={avatar:?}");

        Ok(Tns {
            name: Some(name),
            address: Some(address),
            avatar,
        })
    }

    /// Fetches a text record for a given name
    pub async fn get_text_record(
        name: &str,
        key: &str,
        registry: &TNSRegistry::TNSRegistryInstance<DynProvider, Ethereum>,
    ) -> Result<Option<String>, TnsError> {
        let node_bytes = Self::node_for_name(name);

        let resolver = match Self::get_resolver(node_bytes, registry).await? {
            Some(r) => r,
            None => return Ok(None),
        };

        let value = resolver
            .text(node_bytes, key.to_string())
            .call()
            .await
            .ok()
            .filter(|s| !s.is_empty());

        Ok(value)
    }

    fn prepare_reverse_name(address: Address) -> String {
        let addr_str = address.to_string().to_lowercase();
        format!("{}.addr.reverse", addr_str.trim_start_matches("0x"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namehash_empty() {
        assert_eq!(Tns::namehash(""), vec![0u8; 32]);
    }

    #[test]
    fn test_namehash_trust() {
        let hash = Tns::namehash("trust");
        assert_eq!(hash.len(), 32);
        assert_ne!(hash, vec![0u8; 32]);
    }

    #[test]
    fn test_namehash_alice_trust() {
        let hash = Tns::namehash("alice.trust");
        assert_eq!(hash.len(), 32);
        assert_ne!(hash, vec![0u8; 32]);
        assert_ne!(hash, Tns::namehash("trust"));
    }

    #[test]
    fn test_namehash_consistency() {
        let hash1 = Tns::namehash("alice.trust");
        let hash2 = Tns::namehash("alice.trust");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_namehash_different_names() {
        let hash_alice = Tns::namehash("alice.trust");
        let hash_bob = Tns::namehash("bob.trust");
        assert_ne!(hash_alice, hash_bob);
    }

    #[test]
    fn test_prepare_reverse_name() {
        let address: Address = "0xf1016a7fe89eb9d244c3bfb270071b24619e36c6"
            .parse()
            .unwrap();
        let reverse = Tns::prepare_reverse_name(address);
        assert_eq!(
            reverse,
            "f1016a7fe89eb9d244c3bfb270071b24619e36c6.addr.reverse"
        );
    }

    #[test]
    fn test_ensure_trust_suffix() {
        assert_eq!(Tns::ensure_trust_suffix("alice"), "alice.trust");
        assert_eq!(Tns::ensure_trust_suffix("alice.trust"), "alice.trust");
        assert_eq!(Tns::ensure_trust_suffix("bob.sub"), "bob.sub.trust");
    }

    #[tokio::test]
    #[ignore = "Requires live Intuition RPC - run with --ignored"]
    async fn test_reverse_resolve_live() {
        use alloy::providers::ProviderBuilder;

        let provider = ProviderBuilder::new()
            .connect_http(INTUITION_RPC_URL.parse().unwrap());
        let dyn_provider = DynProvider::new(provider);
        let registry = TNSRegistry::TNSRegistryInstance::new(
            TNS_REGISTRY_ADDRESS.parse::<Address>().unwrap(),
            dyn_provider,
        );

        // Test with the address you're trying to resolve
        let address: Address = "0xf1016a7Fe89EB9D244c3bfB270071b24619e36C6"
            .parse()
            .unwrap();
        let result = Tns::reverse_resolve(address, &registry).await;
        println!("Reverse resolve result for {address}: {result:?}");

        assert!(result.is_ok(), "RPC call should not fail");
        let tns = result.unwrap();
        println!("  name: {:?}", tns.name);
        println!("  avatar: {:?}", tns.avatar);
    }

    #[tokio::test]
    #[ignore = "Requires live Intuition RPC - run with --ignored"]
    async fn test_avatar_text_record() {
        use alloy::providers::ProviderBuilder;

        let provider = ProviderBuilder::new()
            .connect_http(INTUITION_RPC_URL.parse().unwrap());
        let dyn_provider = DynProvider::new(provider);
        let registry = TNSRegistry::TNSRegistryInstance::new(
            TNS_REGISTRY_ADDRESS.parse::<Address>().unwrap(),
            dyn_provider,
        );

        let name = "sammy.trust";
        println!("Querying avatar text record for: {name}");

        // Check what resolver we get for this name
        let node_bytes = Tns::node_for_name(name);
        println!("  node_bytes: 0x{}", hex::encode(node_bytes));

        let resolver = Tns::get_resolver(node_bytes, &registry).await;
        println!("  resolver result: {resolver:?}");

        // Try fetching avatar
        let avatar = Tns::get_text_record(name, "avatar", &registry).await;
        println!("  avatar result: {avatar:?}");

        // Also try other common keys
        let url = Tns::get_text_record(name, "url", &registry).await;
        println!("  url result: {url:?}");

        // Try fetching avatar from the reverse resolver directly
        let address: Address = "0xf1016a7Fe89EB9D244c3bfB270071b24619e36C6"
            .parse()
            .unwrap();
        let reverse_name = Tns::prepare_reverse_name(address);
        let reverse_node = Tns::namehash(&reverse_name);
        let reverse_node_bytes = FixedBytes::from_slice(reverse_node.as_slice());
        println!("  reverse node: 0x{}", hex::encode(reverse_node_bytes));

        let reverse_resolver = Tns::get_resolver(reverse_node_bytes, &registry).await;
        println!("  reverse resolver: {reverse_resolver:?}");

        if let Ok(Some(rev_resolver)) = reverse_resolver {
            let avatar_from_reverse = rev_resolver
                .text(reverse_node_bytes, "avatar".to_string())
                .call()
                .await;
            println!("  avatar from reverse resolver (reverse node): {avatar_from_reverse:?}");

            // Try with forward name node on reverse resolver
            let fwd_node = Tns::node_for_name(name);
            let avatar_fwd_node = rev_resolver
                .text(fwd_node, "avatar".to_string())
                .call()
                .await;
            println!("  avatar from reverse resolver (forward node): {avatar_fwd_node:?}");
        }
    }
}
