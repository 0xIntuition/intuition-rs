use alloy::{
    network::TransactionBuilder,
    primitives::{Address, U256},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
};
use eyre::Result;
use std::str::FromStr;

use crate::{
    contracts::create_contract_instances,
    types::{Config, FundedUser, User},
    utils::{check_balance, create_wallet_provider, derive_wallet_from_mnemonic, format_ether, format_tokens},
};

pub async fn get_and_fund_user<P: alloy::providers::Provider + Clone>(
    config: &Config,
    user_index: u32,
    eth_amount: U256,
    token_amount: U256,
    admin_provider: &P,
) -> Result<FundedUser<impl alloy::providers::Provider + Clone>> {
    let user_signer = derive_wallet_from_mnemonic(&config.mnemonic, user_index)?;
    let user_address = user_signer.address();
    let user_provider = create_wallet_provider(user_signer.clone(), &config.rpc_url)?;

    println!("📝 Setting up user {} (address: {})", user_index, user_address);

    let initial_eth_balance = check_balance(&user_provider, user_address).await?;
    let contracts = create_contract_instances(user_provider.clone(), config);
    
    let initial_token_balance = contracts
        .mock_trust
        .balanceOf(user_address)
        .call()
        .await?;

    println!(
        "  Initial ETH: {} ETH",
        format_ether(initial_eth_balance)?
    );
    println!(
        "  Initial tokens: {} tokens", 
        format_tokens(initial_token_balance, 18)?
    );

    fund_user_with_eth(admin_provider, user_address, eth_amount).await?;

    let admin_contracts = create_contract_instances(admin_provider.clone(), config);
    fund_user_with_tokens(
        &admin_contracts.mock_trust,
        user_address,
        token_amount
    ).await?;

    let final_eth_balance = check_balance(&user_provider, user_address).await?;
    let final_token_balance = contracts
        .mock_trust
        .balanceOf(user_address)
        .call()
        .await?;

    println!(
        "  Final ETH: {} ETH",
        format_ether(final_eth_balance)?
    );
    println!(
        "  Final tokens: {} tokens",
        format_tokens(final_token_balance, 18)?
    );

    let user = User {
        address: user_address,
        signer: user_signer,
        provider: user_provider.clone(),
        eth_balance: final_eth_balance,
        token_balance: final_token_balance,
    };

    let contracts = create_contract_instances(user_provider, config);

    Ok(FundedUser { user, contracts })
}

pub async fn create_admin_user(config: &Config) -> Result<FundedUser<impl alloy::providers::Provider + Clone>> {
    let admin_signer = PrivateKeySigner::from_str(&config.admin_private_key)?;
    let admin_address = admin_signer.address();
    let admin_provider = create_wallet_provider(admin_signer.clone(), &config.rpc_url)?;

    println!("📝 Setting up admin user (address: {})", admin_address);

    let eth_balance = check_balance(&admin_provider, admin_address).await?;
    let contracts = create_contract_instances(admin_provider.clone(), config);
    
    let token_balance = contracts
        .mock_trust
        .balanceOf(admin_address)
        .call()
        .await?;

    println!(
        "  Admin ETH: {} ETH",
        format_ether(eth_balance)?
    );
    println!(
        "  Admin tokens: {} tokens",
        format_tokens(token_balance, 18)?
    );

    let user = User {
        address: admin_address,
        signer: admin_signer,
        provider: admin_provider.clone(),
        eth_balance,
        token_balance,
    };

    Ok(FundedUser { user, contracts })
}

async fn fund_user_with_eth<P: alloy::providers::Provider>(
    admin_provider: &P,
    user_address: Address,
    amount: U256,
) -> Result<()> {
    println!("💸 Funding user with {} ETH...", format_ether(amount)?);

    let tx = TransactionRequest::default()
        .with_to(user_address)
        .with_value(amount)
        .with_gas_limit(21_000); // Set explicit gas limit for ETH transfers

    let pending_tx = admin_provider.send_transaction(tx).await?;
    let tx_hash = pending_tx.tx_hash();
    println!("  Transaction hash: {}", tx_hash);

    let receipt = pending_tx.get_receipt().await?;
    println!(
        "  Transaction confirmed in block: {}",
        receipt.block_number.unwrap()
    );

    Ok(())
}

async fn fund_user_with_tokens<P: alloy::providers::Provider>(
    mock_trust: &crate::contracts::MockTrustInstance<P>,
    user_address: Address,
    amount: U256,
) -> Result<()> {
    println!("🪙 Funding user with {} tokens...", format_tokens(amount, 18)?);

    let transfer_call = mock_trust.transfer(user_address, amount);
    let pending_tx = transfer_call.send().await?;
    let tx_hash = pending_tx.tx_hash();
    println!("  Transaction hash: {}", tx_hash);

    let receipt = pending_tx.get_receipt().await?;
    println!(
        "  Transaction confirmed in block: {}",
        receipt.block_number.unwrap()
    );

    Ok(())
}

pub async fn approve_vault<P: alloy::providers::Provider>(
    mock_trust: &crate::contracts::MockTrustInstance<P>,
    vault_address: Address,
    amount: U256,
) -> Result<()> {
    println!("✅ Approving vault to manage {} tokens...", format_tokens(amount, 18)?);

    let approve_call = mock_trust.approve(vault_address, amount);
    let pending_tx = approve_call.send().await?;
    let tx_hash = pending_tx.tx_hash();
    println!("  Transaction hash: {}", tx_hash);

    let receipt = pending_tx.get_receipt().await?;
    println!(
        "  Transaction confirmed in block: {}",
        receipt.block_number.unwrap()
    );

    let allowance = mock_trust
        .allowance(*mock_trust.address(), vault_address)
        .call()
        .await?;
    println!("  Allowance set: {} tokens", format_tokens(allowance, 18)?);

    Ok(())
}