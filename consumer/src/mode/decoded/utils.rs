use crate::{error::ConsumerError, mode::types::DecodedConsumerContext};
use alloy::primitives::{U256, Uint};
use models::{traits::SimpleCrud, types::U256Wrapper, vault::Vault};
use sqlx::{Postgres, Transaction};

/// This enum represents the different types of updates that can be made to a vault
pub enum VaultUpdate {
    /// This variant represents a deposited event
    Deposited {
        /// The assets that were sent by the sender after total fees
        sender_assets_after_total_fees: U256Wrapper,
    },
    /// This variant represents a redeemed event
    Redeemed {
        /// The shares that were sent to the receiver
        shares_for_receiver: U256Wrapper,
    },
}

/// This function updates the vault with the new total assets
pub async fn update_vault(
    vault_update: VaultUpdate,
    vault_id: Uint<256, 4>,
    decoded_consumer_context: &DecodedConsumerContext,
    tx: &mut Transaction<'_, Postgres>,
    current_share_price: U256Wrapper,
    total_shares: Uint<256, 4>,
) -> Result<(), ConsumerError> {
    // Update vault
    let mut vault = Vault::find_by_id(
        vault_id.into(),
        &decoded_consumer_context.backend_schema,
        tx.as_mut(),
    )
    .await?
    .ok_or(ConsumerError::VaultNotFound)?;

    // Update the vault conditionally based on the type of update
    match vault_update {
        VaultUpdate::Deposited {
            sender_assets_after_total_fees,
        } => {
            vault.total_assets =
                Some(vault.total_assets.unwrap_or(0.try_into()?) + sender_assets_after_total_fees);
        }
        VaultUpdate::Redeemed {
            shares_for_receiver,
        } => {
            vault.total_assets =
                Some(vault.total_assets.unwrap_or(0.try_into()?) - shares_for_receiver);
        }
    }
    // Update regular fields
    vault.current_share_price = current_share_price.clone();
    vault.market_cap = Some(
        U256Wrapper::from(total_shares) * current_share_price
            / U256Wrapper::from(U256::from(10).pow(U256::from(18))),
    );
    vault
        .upsert(&decoded_consumer_context.backend_schema, tx.as_mut())
        .await?;
    Ok(())
}
