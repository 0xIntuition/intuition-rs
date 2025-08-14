use alloy::{
    primitives::Address,
    providers::Provider,
    sol,
};
use crate::types::{Config, ContractInstances};

sol!(
    #[sol(rpc)]
    MockTrust,
    "./abi/MockTrust.json"
);

sol!(
    #[sol(rpc)]
    MultiVault,
    "./abi/MultiVault.json"
);

pub type MockTrustInstance<P> = MockTrust::MockTrustInstance<P>;
pub type MultiVaultInstance<P> = MultiVault::MultiVaultInstance<P>;

pub fn create_contract_instances<P: Provider + Clone>(
    provider: P,
    config: &Config,
) -> ContractInstances<P> {
    let mock_trust = MockTrust::new(config.mock_trust_address, provider.clone());
    let multi_vault = MultiVault::new(config.multi_vault_address, provider);

    ContractInstances {
        mock_trust,
        multi_vault,
    }
}

pub fn create_mock_trust<P: Provider + Clone>(
    address: Address,
    provider: P,
) -> MockTrustInstance<P> {
    MockTrust::new(address, provider)
}

pub fn create_multi_vault<P: Provider + Clone>(
    address: Address,
    provider: P,
) -> MultiVaultInstance<P> {
    MultiVault::new(address, provider)
}