use alloy::{
    primitives::{Address, Bytes, B256, U256},
    signers::local::PrivateKeySigner,
};

#[derive(Debug)]
pub struct User<P> {
    pub address: Address,
    pub signer: PrivateKeySigner,
    pub provider: P,
    pub eth_balance: U256,
    pub token_balance: U256,
}

#[derive(Debug)]
pub struct ContractInstances<P> {
    pub mock_trust: crate::contracts::MockTrustInstance<P>,
    pub multi_vault: crate::contracts::MultiVaultInstance<P>,
}

#[derive(Debug)]
pub struct FundedUser<P> {
    pub user: User<P>,
    pub contracts: ContractInstances<P>,
}

#[derive(Debug, Clone)]
pub struct AtomData {
    pub id: B256,
    pub data: Bytes,
}

#[derive(Debug, Clone)]
pub struct TripleData {
    pub id: B256,
    pub subject_id: B256,
    pub predicate_id: B256,
    pub object_id: B256,
}

#[derive(Debug)]
pub enum TripleCreationResult {
    Created(Vec<TripleData>),
    AlreadyExists(Vec<TripleData>),
    Error(String),
}

#[derive(Debug)]
pub struct Config {
    pub rpc_url: String,
    pub mock_trust_address: Address,
    pub multi_vault_address: Address,
    pub admin_private_key: String,
    pub mnemonic: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rpc_url: std::env::var("RPC_URL").unwrap_or_else(|_| "http://localhost:8545".to_string()),
            mock_trust_address: alloy::primitives::address!("70AC54941671aa51008e4224264187A779aa672e"),
            multi_vault_address: alloy::primitives::address!("9525f7c38353D3A8FDA555C4aB10ed8891d67236"),
            admin_private_key: std::env::var("ADMIN_PRIVATE_KEY").unwrap_or_else(|_| 
                // Only use hardcoded key for local development
                "0x3c0afbd619ed4a8a11cfbd8c5794e08dc324b6809144a90c58bc0ff24219103b".to_string()
            ),
            mnemonic: std::env::var("MNEMONIC").unwrap_or_else(|_|
                // Only use hardcoded mnemonic for local development
                "legal winner thank year wave sausage worth useful legal winner thank yellow".to_string()
            ),
        }
    }
}