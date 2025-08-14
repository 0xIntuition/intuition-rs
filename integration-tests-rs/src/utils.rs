use alloy::{
    primitives::{Bytes, B256, U256},
    signers::local::PrivateKeySigner,
    providers::{Provider, ProviderBuilder},
    sol_types::SolError,
};
use eyre::Result;
use hex;
use sha2::{Digest, Sha256};
use std::str::FromStr;

// use crate::types::Config;

pub fn derive_wallet_from_mnemonic(mnemonic_phrase: &str, index: u32) -> Result<PrivateKeySigner> {
    let mut hasher = Sha256::new();
    hasher.update(mnemonic_phrase.as_bytes());
    hasher.update(index.to_le_bytes());
    let hash = hasher.finalize();

    let private_key_hex = hex::encode(&hash[..]);
    Ok(PrivateKeySigner::from_str(&private_key_hex)?)
}

pub fn create_wallet_provider(signer: PrivateKeySigner, rpc_url: &str) -> Result<impl alloy::providers::Provider + Clone> {
    let provider = ProviderBuilder::new()
        .wallet(signer)
        .connect_http(rpc_url.parse()?);
    Ok(provider)
}

pub fn compute_atom_id(data: &Bytes) -> B256 {
    let mut hasher = Sha256::new();
    hasher.update(data);
    B256::from_slice(&hasher.finalize()[..])
}

// Use manual conversion for token amounts since workspace Alloy version has API differences
pub fn wei_to_tokens(amount_wei: U256, decimals: u32) -> U256 {
    amount_wei * U256::from(10).pow(U256::from(decimals))
}

pub fn tokens_to_wei(amount_tokens: U256, decimals: u32) -> U256 {
    amount_tokens / U256::from(10).pow(U256::from(decimals))
}

pub fn format_ether(amount_wei: U256) -> Result<String> {
    let eth_value = amount_wei.as_limbs()[0] as f64 / 1e18;
    Ok(format!("{:.6}", eth_value))
}

pub fn format_tokens(amount: U256, decimals: u32) -> Result<String> {
    let divisor = 10_u64.pow(decimals) as f64;
    let token_value = amount.as_limbs()[0] as f64 / divisor;
    Ok(format!("{:.6}", token_value))
}

pub async fn check_balance<P: Provider>(
    provider: &P,
    address: alloy::primitives::Address,
) -> Result<U256> {
    Ok(provider.get_balance(address).await?)
}

pub fn format_balance(balance: U256, decimals: &str) -> Result<String> {
    match decimals {
        "ether" => format_ether(balance),
        _ => Ok(format!("{}", balance)),
    }
}

pub fn parse_contract_error(error_msg: &str) -> Option<String> {
    if error_msg.contains("data: \"0x") {
        if let Some(start) = error_msg.find("data: \"0x") {
            let data_start = start + 9; // Skip "data: \"0x"
            if let Some(end) = error_msg[data_start..].find('"') {
                let hex_data = &error_msg[data_start..data_start + end];
                if let Ok(_revert_data) = hex::decode(hex_data) {
                    return Some(format!("0x{}", hex_data));
                }
            }
        }
    }
    None
}

#[derive(Debug)]
pub enum ContractError {
    AtomExists { atom_data: Bytes },
    AtomDataTooLong,
    NoAtomDataProvided,
    TripleExists {
        subject_id: B256,
        predicate_id: B256,
        object_id: B256,
    },
    Other(String),
}

pub fn decode_multivault_error(revert_data: &[u8]) -> ContractError {
    use crate::contracts::MultiVault;
    
    if let Ok(atom_exists_error) = MultiVault::MultiVault_AtomExists::abi_decode(revert_data) {
        return ContractError::AtomExists {
            atom_data: atom_exists_error.atomData,
        };
    }
    
    if let Ok(_) = MultiVault::MultiVault_AtomDataTooLong::abi_decode(revert_data) {
        return ContractError::AtomDataTooLong;
    }
    
    if let Ok(_) = MultiVault::MultiVault_NoAtomDataProvided::abi_decode(revert_data) {
        return ContractError::NoAtomDataProvided;
    }
    
    if let Ok(triple_error) = MultiVault::MultiVault_TripleExists::abi_decode(revert_data) {
        return ContractError::TripleExists {
            subject_id: triple_error.subjectId,
            predicate_id: triple_error.predicateId,
            object_id: triple_error.objectId,
        };
    }
    
    ContractError::Other(format!("Unknown error: 0x{}", hex::encode(revert_data)))
}