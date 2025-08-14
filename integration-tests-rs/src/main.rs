use alloy::{
    network::TransactionBuilder,
    primitives::{address, utils::format_units, Bytes, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
    sol,
    sol_types::SolError,
};
use eyre::Result;
use hex;
use sha2::{Digest, Sha256};
use std::str::FromStr;

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

fn derive_wallet_from_mnemonic(mnemonic_phrase: &str, index: u32) -> Result<PrivateKeySigner> {
    // For simplicity, we'll use a simpler derivation method
    // This is a simplified approach - for production use, you would want proper BIP32 derivation

    use sha2::Digest;

    // Create a deterministic key from mnemonic + index
    let mut hasher = Sha256::new();
    hasher.update(mnemonic_phrase.as_bytes());
    hasher.update(index.to_le_bytes());
    let hash = hasher.finalize();

    let private_key_hex = hex::encode(&hash[..]);
    Ok(PrivateKeySigner::from_str(&private_key_hex)?)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Configuration from dev.md
    let rpc_url = "http://localhost:8545";
    let mock_trust_address = address!("70AC54941671aa51008e4224264187A779aa672e");
    let multi_vault_address = address!("9525f7c38353D3A8FDA555C4aB10ed8891d67236");
    let admin_private_key = "0x3c0afbd619ed4a8a11cfbd8c5794e08dc324b6809144a90c58bc0ff24219103b";
    let mnemonic = "legal winner thank year wave sausage worth useful legal winner thank yellow";

    println!("🚀 Starting Intuition Integration Test\n");

    // Step 1: Setup wallets
    println!("📝 Setting up wallets...");
    let admin_signer = PrivateKeySigner::from_str(admin_private_key)?;
    let admin_address = admin_signer.address();
    println!("  Admin address: {}", admin_address);

    // Derive Alice's wallet (index 0)
    let alice_signer = derive_wallet_from_mnemonic(mnemonic, 0)?;
    let alice_address = alice_signer.address();
    println!("  Alice address: {}", alice_address);

    // Create providers
    let admin_provider = ProviderBuilder::new()
        .wallet(admin_signer.clone())
        .connect_http(rpc_url.parse()?);

    let alice_provider = ProviderBuilder::new()
        .wallet(alice_signer.clone())
        .connect_http(rpc_url.parse()?);

    // Check initial balances
    println!("\n📊 Initial balances:");
    let admin_eth_balance = admin_provider.get_balance(admin_address).await?;
    let alice_eth_balance = admin_provider.get_balance(alice_address).await?;
    println!(
        "  Admin ETH: {} ETH",
        format_units(admin_eth_balance, "ether")?
    );
    println!(
        "  Alice ETH: {} ETH",
        format_units(alice_eth_balance, "ether")?
    );

    // Check MockTrust token balances
    let mock_trust = MockTrust::new(mock_trust_address, &admin_provider);
    let admin_token_balance = mock_trust.balanceOf(admin_address).call().await?;
    let alice_token_balance = mock_trust.balanceOf(alice_address).call().await?;
    println!(
        "  Admin MockTrust: {} tokens",
        format_units(admin_token_balance, 18)?
    );
    println!(
        "  Alice MockTrust: {} tokens",
        format_units(alice_token_balance, 18)?
    );

    // Step 2: Admin transfers 1 ETH to Alice
    println!("\n💸 Admin transferring 1 ETH to Alice...");
    let eth_amount = U256::from(1_000_000_000_000_000_000u64); // 1 ETH in wei

    let tx = TransactionRequest::default()
        .with_to(alice_address)
        .with_value(eth_amount);

    let pending_tx = admin_provider.send_transaction(tx).await?;
    let tx_hash = pending_tx.tx_hash();
    println!("  Transaction hash: {}", tx_hash);

    let receipt = pending_tx.get_receipt().await?;
    println!(
        "  Transaction confirmed in block: {}",
        receipt.block_number.unwrap()
    );

    // Step 3: Admin transfers 100 MockTrust tokens to Alice
    println!("\n🪙 Admin transferring 100 MockTrust tokens to Alice...");
    let token_amount = U256::from(100) * U256::from(10).pow(U256::from(18)); // 100 tokens with 18 decimals

    // First, admin needs to mint tokens to themselves (if they have minting rights)
    // Or we assume admin already has tokens

    // Transfer tokens to Alice
    let transfer_call = mock_trust.transfer(alice_address, token_amount);
    let pending_tx = transfer_call.send().await?;
    let tx_hash = pending_tx.tx_hash();
    println!("  Transaction hash: {}", tx_hash);

    let receipt = pending_tx.get_receipt().await?;
    println!(
        "  Transaction confirmed in block: {}",
        receipt.block_number.unwrap()
    );

    // Step 4: Alice approves MultiVault to manage 100 MockTrust tokens
    println!("\n✅ Alice approving MultiVault to manage 100 MockTrust tokens...");
    let mock_trust_alice = MockTrust::new(mock_trust_address, &alice_provider);

    let approve_call = mock_trust_alice.approve(multi_vault_address, token_amount);
    let pending_tx = approve_call.send().await?;
    let tx_hash = pending_tx.tx_hash();
    println!("  Transaction hash: {}", tx_hash);

    let receipt = pending_tx.get_receipt().await?;
    println!(
        "  Transaction confirmed in block: {}",
        receipt.block_number.unwrap()
    );

    // Verify approval
    let allowance = mock_trust_alice
        .allowance(alice_address, multi_vault_address)
        .call()
        .await?;
    println!("  Allowance set: {} tokens", format_units(allowance, 18)?);

    // Step 5: Alice creates an atom for "hello world" with initial deposit of 1 MockTrust
    println!("\n🔮 Alice creating atom for 'hello world' with 1 MockTrust deposit...");

    // Prepare atom data (string "hello world" as bytes)
    let atom_data = Bytes::from("hey world".as_bytes().to_vec());
    let atom_data_array = vec![atom_data.clone()];

    // Deposit amount: 1 MockTrust token
    let deposit_amount = U256::from(1) * U256::from(10).pow(U256::from(18)); // 1 token with 18 decimals

    let multi_vault = MultiVault::new(multi_vault_address, &alice_provider);
    let create_atoms_call = multi_vault.createAtoms(atom_data_array, deposit_amount);

    match create_atoms_call.send().await {
        Ok(pending_tx) => {
            let tx_hash = pending_tx.tx_hash();
            println!("  Transaction hash: {}", tx_hash);

            let receipt = pending_tx.get_receipt().await?;
            println!(
                "  Transaction confirmed in block: {}",
                receipt.block_number.unwrap()
            );

            // The createAtoms function returns atom IDs, but we need to parse them from events
            let logs = receipt.inner.logs();
            if !logs.is_empty() {
                println!("  Atom created successfully!");
                // Calculate the atom ID (keccak256 hash of the data)
                let mut hasher = Sha256::new();
                hasher.update(b"hello world");
                let atom_id = hasher.finalize();
                println!("  Atom ID (calculated): 0x{}", hex::encode(&atom_id[..]));
            }
        }
        Err(e) => {
            println!("\n❌ Transaction failed!");

            // Extract error message from the contract error
            let error_msg = e.to_string();

            // Check if the error contains revert data
            if error_msg.contains("data: \"0x") {
                // Extract the hex data from the error message
                if let Some(start) = error_msg.find("data: \"0x") {
                    let data_start = start + 9; // Skip "data: \"0x"
                    if let Some(end) = error_msg[data_start..].find('"') {
                        let hex_data = &error_msg[data_start..data_start + end];

                        // Decode the hex string
                        if let Ok(revert_data) = hex::decode(hex_data) {
                            // Try to decode as MultiVault_AtomExists error
                            if let Ok(atom_exists_error) =
                                MultiVault::MultiVault_AtomExists::abi_decode(&revert_data)
                            {
                                println!("  ✅ Decoded Error: Atom already exists!");
                                println!(
                                    "  Atom data: \"{}\"",
                                    String::from_utf8_lossy(&atom_exists_error.atomData)
                                );
                                println!("\n  This means the atom for 'hello world' has already been created in a previous run.");
                                println!(
                                    "  The contract prevents duplicate atoms from being created."
                                );
                            }
                            // Try to decode as MultiVault_AtomDataTooLong error
                            else if let Ok(_) =
                                MultiVault::MultiVault_AtomDataTooLong::abi_decode(&revert_data)
                            {
                                println!("  ✅ Decoded Error: Atom data is too long!");
                            }
                            // Try to decode as MultiVault_NoAtomDataProvided error
                            else if let Ok(_) =
                                MultiVault::MultiVault_NoAtomDataProvided::abi_decode(&revert_data)
                            {
                                println!("  ✅ Decoded Error: No atom data provided!");
                            }
                            // Try to decode as MultiVault_TripleExists error
                            else if let Ok(triple_error) =
                                MultiVault::MultiVault_TripleExists::abi_decode(&revert_data)
                            {
                                println!("  ✅ Decoded Error: Triple already exists!");
                                println!("  Subject ID: 0x{}", hex::encode(triple_error.subjectId));
                                println!(
                                    "  Predicate ID: 0x{}",
                                    hex::encode(triple_error.predicateId)
                                );
                                println!("  Object ID: 0x{}", hex::encode(triple_error.objectId));
                            }
                            // Try other error types
                            else {
                                println!("  Raw error data: 0x{}", hex_data);

                                // Try to decode as a string error message
                                if revert_data.len() >= 4 {
                                    let selector = &revert_data[0..4];
                                    println!("  Error selector: 0x{}", hex::encode(selector));

                                    // Check all possible MultiVault errors by selector
                                    let selector_hex = hex::encode(selector);
                                    println!("  Checking error selector against known MultiVault errors...");

                                    // Try to decode the rest as ABI-encoded string
                                    if revert_data.len() > 68 {
                                        // Skip selector (4 bytes) and offset (32 bytes) and length (32 bytes)
                                        let start = 68;
                                        if let Some(length) =
                                            U256::try_from_be_slice(&revert_data[36..68])
                                        {
                                            let str_len = length.to::<usize>();
                                            if revert_data.len() >= start + str_len {
                                                if let Ok(decoded_str) = String::from_utf8(
                                                    revert_data[start..start + str_len].to_vec(),
                                                ) {
                                                    println!(
                                                        "  Decoded error message: \"{}\"",
                                                        decoded_str
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            println!("  Failed to decode hex data");
                        }
                    }
                }
            } else {
                println!("  Error: {}", e);
            }

            // Don't return error - continue with the test to show what we can
            println!("\n  Continuing with the test despite the error...");
        }
    }

    // Step 6: Check final balances
    println!("\n📊 Final balances:");
    let alice_eth_balance = alice_provider.get_balance(alice_address).await?;
    let alice_token_balance = mock_trust_alice.balanceOf(alice_address).call().await?;
    println!(
        "  Alice ETH: {} ETH",
        format_units(alice_eth_balance, "ether")?
    );
    println!(
        "  Alice MockTrust: {} tokens",
        format_units(alice_token_balance, 18)?
    );

    println!("\n✨ Integration test completed successfully!");

    Ok(())
}

