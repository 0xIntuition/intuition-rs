use alloy::{
    primitives::{B256, U256},
};
use eyre::Result;
use hex;
use sha2::{Digest, Sha256};

use crate::{
    types::{AtomData, TripleData, TripleCreationResult},
    utils::{decode_multivault_error, ContractError, format_tokens},
};

pub async fn create_triples<P: alloy::providers::Provider>(
    multi_vault: &crate::contracts::MultiVaultInstance<P>,
    subject_ids: Vec<B256>,
    predicate_ids: Vec<B256>,
    object_ids: Vec<B256>,
    deposit_amount: U256,
) -> Result<TripleCreationResult> {
    if subject_ids.len() != predicate_ids.len() || predicate_ids.len() != object_ids.len() {
        return Ok(TripleCreationResult::Error(
            "All arrays (subjects, predicates, objects) must have the same length".to_string()
        ));
    }

    if subject_ids.is_empty() {
        return Ok(TripleCreationResult::Error(
            "No triples provided".to_string()
        ));
    }

    let triple_data_vec: Vec<TripleData> = subject_ids
        .iter()
        .zip(predicate_ids.iter())
        .zip(object_ids.iter())
        .map(|((subject_id, predicate_id), object_id)| TripleData {
            id: compute_triple_id(*subject_id, *predicate_id, *object_id),
            subject_id: *subject_id,
            predicate_id: *predicate_id,
            object_id: *object_id,
        })
        .collect();

    println!("🔗 Creating triples with {} deposit...", format_tokens(deposit_amount, 18)?);
    for (i, triple) in triple_data_vec.iter().enumerate() {
        println!("  Triple {}: (0x{}, 0x{}, 0x{}) → ID: 0x{}", 
            i + 1, 
            hex::encode(&triple.subject_id[..8]),
            hex::encode(&triple.predicate_id[..8]),
            hex::encode(&triple.object_id[..8]),
            hex::encode(&triple.id[..8])
        );
    }

    match multi_vault.createTriples(subject_ids, predicate_ids, object_ids, deposit_amount).send().await {
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
                println!("  ✅ Triples created successfully!");
                return Ok(TripleCreationResult::Created(triple_data_vec));
            } else {
                return Ok(TripleCreationResult::Error("No logs found in receipt".to_string()));
            }
        }
        Err(e) => {
            println!("\n⚠️  Transaction failed, analyzing error...");
            
            let error_msg = e.to_string();
            println!("  Raw error: {}", error_msg);

            if let Some(revert_data_hex) = extract_revert_data(&error_msg) {
                if let Ok(revert_data) = hex::decode(&revert_data_hex[2..]) {
                    // Check if this is specifically a TripleExists error (signature 0x280ef262)
                    if revert_data.len() >= 4 && &revert_data[0..4] == &[0x28, 0x0e, 0xf2, 0x62] {
                        if revert_data.len() >= 36 { // 4 bytes signature + 32 bytes for first parameter
                            let remaining_data = &revert_data[4..];
                            if remaining_data.len() >= 32 {
                                let subject_id = B256::from_slice(&remaining_data[0..32]);
                                println!("  ✅ Triple already exists!");
                                println!("    Subject: 0x{}", hex::encode(subject_id));
                                println!("    This means the triple has already been created in a previous run.");
                                return Ok(TripleCreationResult::AlreadyExists(triple_data_vec));
                            }
                        }
                    }
                    
                    match decode_multivault_error(&revert_data) {
                        ContractError::TripleExists { subject_id, predicate_id, object_id } => {
                            println!("  ✅ Triple already exists!");
                            println!("    Subject: 0x{}", hex::encode(subject_id));
                            println!("    Predicate: 0x{}", hex::encode(predicate_id));
                            println!("    Object: 0x{}", hex::encode(object_id));
                            println!("    This means the triple has already been created in a previous run.");
                            return Ok(TripleCreationResult::AlreadyExists(triple_data_vec));
                        }
                        ContractError::AtomExists { atom_data: _ } => {
                            let error_msg = "One of the atoms used in the triple does not exist".to_string();
                            println!("  ❌ Error: {}", error_msg);
                            return Ok(TripleCreationResult::Error(error_msg));
                        }
                        ContractError::Other(msg) => {
                            println!("  ❌ Contract error: {}", msg);
                            return Ok(TripleCreationResult::Error(msg));
                        }
                        _ => {
                            let error_msg = format!("Unexpected contract error: {}", e);
                            println!("  ❌ {}", error_msg);
                            return Ok(TripleCreationResult::Error(error_msg));
                        }
                    }
                }
            }
            
            let error_msg = format!("Failed to create triples: {}", e);
            println!("  ❌ {}", error_msg);
            return Ok(TripleCreationResult::Error(error_msg));
        }
    }
}

pub async fn create_single_triple<P: alloy::providers::Provider>(
    multi_vault: &crate::contracts::MultiVaultInstance<P>,
    subject_id: B256,
    predicate_id: B256,
    object_id: B256,
    deposit_amount: U256,
) -> Result<TripleCreationResult> {
    create_triples(
        multi_vault, 
        vec![subject_id], 
        vec![predicate_id], 
        vec![object_id], 
        deposit_amount
    ).await
}

pub async fn create_triples_from_atoms<P: alloy::providers::Provider>(
    multi_vault: &crate::contracts::MultiVaultInstance<P>,
    subjects: Vec<&AtomData>,
    predicates: Vec<&AtomData>,
    objects: Vec<&AtomData>,
    deposit_amount: U256,
) -> Result<TripleCreationResult> {
    let subject_ids: Vec<B256> = subjects.iter().map(|atom| atom.id).collect();
    let predicate_ids: Vec<B256> = predicates.iter().map(|atom| atom.id).collect();
    let object_ids: Vec<B256> = objects.iter().map(|atom| atom.id).collect();

    create_triples(multi_vault, subject_ids, predicate_ids, object_ids, deposit_amount).await
}

pub fn compute_triple_id(subject_id: B256, predicate_id: B256, object_id: B256) -> B256 {
    let mut hasher = Sha256::new();
    hasher.update(subject_id.as_slice());
    hasher.update(predicate_id.as_slice());
    hasher.update(object_id.as_slice());
    B256::from_slice(&hasher.finalize()[..])
}

pub fn print_triple_summary(triples: &[TripleData]) {
    println!("🔗 Triple Summary:");
    for (i, triple) in triples.iter().enumerate() {
        println!(
            "  {}. (0x{}, 0x{}, 0x{}) → ID: 0x{}",
            i + 1,
            hex::encode(&triple.subject_id[..8]),
            hex::encode(&triple.predicate_id[..8]),
            hex::encode(&triple.object_id[..8]),
            hex::encode(&triple.id[..8])
        );
    }
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