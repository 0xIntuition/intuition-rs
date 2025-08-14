use alloy::{primitives::U256, providers::Provider};
use eyre::Result;
use integration_tests_rs::{user::approve_vault, AtomCreationResult, IntuitionClient};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting Intuition Integration Test (Refactored)\n");

    // Step 1: Initialize the Intuition client with default configuration
    let client = IntuitionClient::with_default_config();

    // Step 2: Initialize admin user
    println!("📝 Initializing admin user...");
    let client = client.initialize_admin().await?;

    // Step 3: Create and fund Alice (user index 0)
    let eth_amount = U256::from(1_000_000_000_000_000_000u64); // 1 ETH in wei
    let token_amount = U256::from(100) * U256::from(10).pow(U256::from(18)); // 100 tokens

    println!("\n📝 Creating and funding Alice...");
    let alice = client
        .get_and_fund_user(0, eth_amount, token_amount)
        .await?;

    // Step 4: Alice approves MultiVault to manage tokens
    let approval_amount = U256::from(100) * U256::from(10).pow(U256::from(18)); // 100 tokens
    approve_vault(
        &alice.contracts.mock_trust,
        client.config.multi_vault_address,
        approval_amount,
    )
    .await?;

    // Step 5: Alice creates atoms using the refactored function
    println!("\n🔮 Alice creating atoms...");
    let atom_data = vec![
        "the ticker".to_string(),
        "is".to_string(),
        "trust".to_string(),
    ];
    let deposit_per_atom = U256::from(1) * U256::from(10).pow(U256::from(18)); // 1 token per atom

    match client
        .get_or_create_atoms(&alice, atom_data.clone(), deposit_per_atom)
        .await?
    {
        AtomCreationResult::Created(atoms) => {
            println!("✅ Atoms created successfully!");
            for (i, atom) in atoms.iter().enumerate() {
                println!(
                    "  Atom {}: {} → ID: 0x{}",
                    i + 1,
                    String::from_utf8_lossy(&atom.data),
                    hex::encode(&atom.id[..8])
                );
            }
        }
        AtomCreationResult::AlreadyExists(atoms) => {
            println!("✅ Atoms already exist!");
            for (i, atom) in atoms.iter().enumerate() {
                println!(
                    "  Atom {}: {} → ID: 0x{}",
                    i + 1,
                    String::from_utf8_lossy(&atom.data),
                    hex::encode(&atom.id[..8])
                );
            }
        }
        AtomCreationResult::Error(error_msg) => {
            println!("❌ Error creating atoms: {}", error_msg);
        }
    }

    // Step 6: Show final balances
    println!("\n📊 Final balances:");
    let final_eth_balance = alice.user.provider.get_balance(alice.user.address).await?;
    let final_token_balance = alice
        .contracts
        .mock_trust
        .balanceOf(alice.user.address)
        .call()
        .await?;

    println!(
        "  Alice ETH: {} ETH",
        integration_tests_rs::format_ether(final_eth_balance)?
    );
    println!(
        "  Alice tokens: {} tokens",
        integration_tests_rs::format_tokens(final_token_balance, 18)?
    );

    Ok(())
}
