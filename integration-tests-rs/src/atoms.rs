use alloy::{
    primitives::{Bytes, B256, U256},
};
use eyre::Result;
use hex;

use crate::{
    types::AtomData,
    utils::{compute_atom_id, decode_multivault_error, ContractError, format_tokens},
};

#[derive(Debug)]
pub enum AtomCreationResult {
    Created(Vec<AtomData>),
    AlreadyExists(Vec<AtomData>),
    Error(String),
}

pub async fn get_or_create_atoms<P: alloy::providers::Provider>(
    multi_vault: &crate::contracts::MultiVaultInstance<P>,
    atom_data_list: Vec<String>,
    deposit_amount: U256,
) -> Result<AtomCreationResult> {
    let atom_bytes: Vec<Bytes> = atom_data_list
        .iter()
        .map(|data| Bytes::from(data.as_bytes().to_vec()))
        .collect();

    let atom_data_vec: Vec<AtomData> = atom_bytes
        .iter()
        .map(|bytes| AtomData {
            id: compute_atom_id(bytes),
            data: bytes.clone(),
        })
        .collect();

    // Calculate total deposit (deposit_amount is per atom)
    let total_deposit = deposit_amount * U256::from(atom_bytes.len());
    
    println!("🔮 Creating {} atoms with {} total deposit ({} per atom)...", 
        atom_bytes.len(),
        format_tokens(total_deposit, 18)?,
        format_tokens(deposit_amount, 18)?
    );
    for (i, atom) in atom_data_vec.iter().enumerate() {
        println!("  Atom {}: \"{}\" (ID: 0x{})", 
            i, 
            String::from_utf8_lossy(&atom.data),
            hex::encode(&atom.id[..8])
        );
    }

    match multi_vault.createAtoms(atom_bytes.clone(), total_deposit).send().await {
        Ok(pending_tx) => {
            let tx_hash = pending_tx.tx_hash();
            println!("  Transaction hash: {}", tx_hash);

            let receipt = pending_tx.get_receipt().await?;
            println!(
                "  Transaction confirmed in block: {}",
                receipt.block_number.unwrap()
            );

            let logs = receipt.inner.logs();
            if !logs.is_empty() {
                println!("  ✅ Atoms created successfully!");
                return Ok(AtomCreationResult::Created(atom_data_vec));
            } else {
                return Ok(AtomCreationResult::Error("No logs found in receipt".to_string()));
            }
        }
        Err(e) => {
            println!("\n⚠️  Transaction failed, analyzing error...");
            
            let error_msg = e.to_string();
            println!("  Raw error: {}", error_msg);

            if let Some(revert_data_hex) = extract_revert_data(&error_msg) {
                if let Ok(revert_data) = hex::decode(&revert_data_hex[2..]) {
                    match decode_multivault_error(&revert_data) {
                        ContractError::AtomExists { atom_data } => {
                            println!("  ✅ Atom already exists!");
                            println!("    Atom data: \"{}\"", String::from_utf8_lossy(&atom_data));
                            println!("    This means the atom has already been created in a previous run.");
                            return Ok(AtomCreationResult::AlreadyExists(atom_data_vec));
                        }
                        ContractError::AtomDataTooLong => {
                            let error_msg = "Atom data is too long!".to_string();
                            println!("  ❌ Error: {}", error_msg);
                            return Ok(AtomCreationResult::Error(error_msg));
                        }
                        ContractError::NoAtomDataProvided => {
                            let error_msg = "No atom data provided!".to_string();
                            println!("  ❌ Error: {}", error_msg);
                            return Ok(AtomCreationResult::Error(error_msg));
                        }
                        ContractError::TripleExists { subject_id, predicate_id, object_id } => {
                            let error_msg = format!(
                                "Triple already exists! Subject: 0x{}, Predicate: 0x{}, Object: 0x{}",
                                hex::encode(subject_id),
                                hex::encode(predicate_id),
                                hex::encode(object_id)
                            );
                            println!("  ❌ Error: {}", error_msg);
                            return Ok(AtomCreationResult::Error(error_msg));
                        }
                        ContractError::Other(msg) => {
                            println!("  ❌ Unknown contract error: {}", msg);
                            return Ok(AtomCreationResult::Error(msg));
                        }
                    }
                }
            }
            
            let error_msg = format!("Failed to create atoms: {}", e);
            println!("  ❌ {}", error_msg);
            return Ok(AtomCreationResult::Error(error_msg));
        }
    }
}

pub async fn create_single_atom<P: alloy::providers::Provider>(
    multi_vault: &crate::contracts::MultiVaultInstance<P>,
    atom_data: String,
    deposit_amount: U256,
) -> Result<AtomCreationResult> {
    get_or_create_atoms(multi_vault, vec![atom_data], deposit_amount).await
}

pub async fn batch_create_atoms<P: alloy::providers::Provider>(
    multi_vault: &crate::contracts::MultiVaultInstance<P>,
    atom_data_list: Vec<String>,
    deposit_per_atom: U256,
) -> Result<Vec<AtomCreationResult>> {
    let mut results = Vec::new();
    
    for atom_data in atom_data_list {
        let result = create_single_atom(multi_vault, atom_data, deposit_per_atom).await?;
        results.push(result);
    }
    
    Ok(results)
}

pub fn get_existing_atoms(data_list: Vec<String>) -> Vec<AtomData> {
    data_list
        .iter()
        .map(|data| {
            let bytes = Bytes::from(data.as_bytes().to_vec());
            AtomData {
                id: compute_atom_id(&bytes),
                data: bytes,
            }
        })
        .collect()
}

pub async fn check_atom_exists<P: alloy::providers::Provider>(
    _multi_vault: &crate::contracts::MultiVaultInstance<P>,
    _atom_id: B256,
) -> Result<bool> {
    // Note: This would require a contract method to check atom existence
    // For now, we'll return false and let the create operation handle it
    Ok(false)
}

fn extract_revert_data(error_msg: &str) -> Option<String> {
    if error_msg.contains("data: \"0x") {
        if let Some(start) = error_msg.find("data: \"0x") {
            let data_start = start + 9; // Skip "data: \"0x"
            if let Some(end) = error_msg[data_start..].find('"') {
                let hex_data = &error_msg[data_start..data_start + end];
                return Some(format!("0x{}", hex_data));
            }
        }
    }
    None
}

pub fn atoms_to_string_vec(atoms: &[AtomData]) -> Vec<String> {
    atoms
        .iter()
        .map(|atom| String::from_utf8_lossy(&atom.data).to_string())
        .collect()
}

pub fn print_atom_summary(atoms: &[AtomData]) {
    println!("📋 Atom Summary:");
    for (i, atom) in atoms.iter().enumerate() {
        println!(
            "  {}. \"{}\" → ID: 0x{}",
            i + 1,
            String::from_utf8_lossy(&atom.data),
            hex::encode(&atom.id[..8])
        );
    }
}