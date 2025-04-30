use crate::{
    error::ConsumerError,
    mode::{resolver::types::ResolverConsumerMessage, types::DecodedConsumerContext},
};
use alloy::primitives::{U256, Uint};
use models::{
    account::{Account, AccountType},
    traits::SimpleCrud,
    types::U256Wrapper,
    vault::Vault,
};
use tracing::info;

/// Shortens an address string by taking first 6 and last 4 chars
pub fn short_id(address: &str) -> String {
    format!("{}...{}", &address[..6], &address[address.len() - 4..])
}

/// Returns the absolute triple ID for a given vault ID by determining if it's a counter vault
/// and adjusting the ID accordingly
#[allow(dead_code)]
pub fn get_absolute_triple_id(vault_id: U256) -> U256 {
    // Calculate max value: (2^255 * 2 - 1) / 2
    let max = (U256::from(2).pow(U256::from(255)) * U256::from(2) - U256::from(1)) / U256::from(2);

    // Check if this is a counter vault by comparing against max
    let is_counter_vault = max < vault_id;

    if is_counter_vault {
        // For counter vaults, calculate: 2^255 * 2 - 1 - vault_id
        U256::from(2).pow(U256::from(255)) * U256::from(2) - U256::from(1) - vault_id
    } else {
        vault_id
    }
}

/// This function gets or creates an account
pub async fn get_or_create_account(
    id: String,
    decoded_consumer_context: &DecodedConsumerContext,
) -> Result<Account, ConsumerError> {
    if let Some(account) = Account::find_by_id(
        id.clone(),
        &decoded_consumer_context.pg_pool,
        &decoded_consumer_context.backend_schema,
    )
    .await?
    {
        info!("Returning existing account for: {}", id);
        Ok(account)
    } else {
        info!("Creating account for: {}", id);
        let account = Account::builder()
            .id(id.clone())
            .label(short_id(&id))
            .account_type(AccountType::Default)
            .build()
            .upsert(
                &decoded_consumer_context.pg_pool,
                &decoded_consumer_context.backend_schema,
            )
            .await
            .map_err(ConsumerError::ModelError)?;

        // Now we need to enqueue the message to be processed by the resolver. In this
        // process we check if the account has ENS data associated, and if it does, we
        // update the account with the ENS data (name [label] and image)
        let message = ResolverConsumerMessage::new_account(account.clone());
        decoded_consumer_context
            .client
            .send_message(serde_json::to_string(&message)?, None)
            .await?;
        Ok(account)
    }
}

pub async fn update_account_with_atom_id(
    account: &mut Account,
    atom_id: U256Wrapper,
    decoded_consumer_context: &DecodedConsumerContext,
) -> Result<(), ConsumerError> {
    account.atom_id = Some(atom_id);
    account
        .upsert(
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;
    info!("Updated account: {:?}", account);

    // Now we need to enqueue the message to be processed by the resolver. In this
    // process we check if the account has ENS data associated, and if it does, we
    // update the account with the ENS data (name [label] and image)
    let message = ResolverConsumerMessage::new_account(account.clone());
    decoded_consumer_context
        .client
        .send_message(serde_json::to_string(&message)?, None)
        .await?;
    Ok(())
}

pub async fn update_vault(
    vault_id: Uint<256, 4>,
    decoded_consumer_context: &DecodedConsumerContext,
    block_number: i64,
) -> Result<(), ConsumerError> {
    // Update vault
    let mut vault = Vault::find_by_id(
        vault_id.into(),
        &decoded_consumer_context.pg_pool,
        &decoded_consumer_context.backend_schema,
    )
    .await?
    .ok_or(ConsumerError::VaultNotFound)?;

    let current_share_price: U256Wrapper = decoded_consumer_context
        .fetch_current_share_price(vault_id, block_number)
        .await?
        .into();
    let total_shares = decoded_consumer_context
        .fetch_total_shares_in_vault(vault_id, block_number)
        .await?;

    vault.current_share_price = current_share_price.clone();
    vault.market_cap = Some(
        U256Wrapper::from(total_shares) * current_share_price
            / U256Wrapper::from(U256::from(10).pow(U256::from(18))),
    );
    vault
        .upsert(
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?;
    Ok(())
}
