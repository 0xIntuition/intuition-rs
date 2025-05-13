use std::str::FromStr;

use alloy::primitives::{U256, Uint};
use models::{
    deposit::Deposit, position::Position, share_price_change::SharePriceChange, signal::Signal,
    term::TermType, traits::SimpleCrud, types::U256Wrapper, vault::Vault,
};
use sqlx::{Postgres, Transaction};
use tracing::info;

use crate::{
    EthMultiVault::Deposited,
    EthMultiVaultV1_5::Deposited as DepositedV1_5,
    error::ConsumerError,
    mode::{
        decoded::utils::{VaultUpdate, update_vault},
        types::DecodedConsumerContext,
        utils::{get_or_create_account, get_or_create_vault},
    },
    schemas::types::DecodedMessage,
    traits::{SharePriceEvent, VaultManager},
};

impl VaultManager for &Deposited {
    fn term_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.vaultId))
    }

    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from_str("1")?)
    }

    async fn total_shares(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        block_number: Option<i64>,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(decoded_consumer_context
            .fetch_total_shares_in_vault(
                self.vaultId,
                block_number.ok_or(ConsumerError::BlockNumberNotFound)?,
            )
            .await?
            .into())
    }

    async fn current_share_price(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        block_number: Option<i64>,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(decoded_consumer_context
            .fetch_current_share_price(
                self.vaultId,
                block_number.ok_or(ConsumerError::BlockNumberNotFound)?,
            )
            .await?
            .into())
    }

    async fn position_count(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<i32, ConsumerError> {
        Ok(Position::count_by_vault_and_curve(
            self.vaultId.into(),
            "1".try_into()?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await? as i32)
    }
}

/// This impl is used to convert the `Deposited` event into a `SharePriceEvent`
impl SharePriceEvent for &Deposited {}

impl VaultManager for &DepositedV1_5 {
    fn term_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from(self.vaultId))
    }

    fn curve_id(&self) -> Result<U256Wrapper, ConsumerError> {
        Ok(U256Wrapper::from_str("1")?)
    }

    async fn total_shares(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        _block_number: Option<i64>,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(SharePriceChange::fetch_current_share_price(
            self.vaultId.into(),
            1.try_into()?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .total_shares)
    }

    async fn current_share_price(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        _block_number: Option<i64>,
    ) -> Result<U256Wrapper, ConsumerError> {
        Ok(SharePriceChange::fetch_current_share_price(
            self.vaultId.into(),
            1.try_into()?,
            &decoded_consumer_context.pg_pool,
            &decoded_consumer_context.backend_schema,
        )
        .await?
        .share_price)
    }

    async fn position_count(
        &self,
        _decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<i32, ConsumerError> {
        Ok(0)
    }
}

/// This impl is used to convert the `DepositedV1_5` event into a `SharePriceEvent`
impl SharePriceEvent for &DepositedV1_5 {}

pub trait DepositedEvent: SharePriceEvent + VaultManager + Clone {
    fn sender(&self) -> Result<String, ConsumerError>;
    fn receiver(&self) -> Result<String, ConsumerError>;
    fn receiver_total_shares_in_vault(&self) -> Result<Uint<256, 4>, ConsumerError>;
    fn vault_id(&self) -> Result<Uint<256, 4>, ConsumerError>;
    fn is_triple(&self) -> Result<bool, ConsumerError>;
    fn is_atom_wallet(&self) -> Result<bool, ConsumerError>;
    fn entry_fee(&self) -> Result<Uint<256, 4>, ConsumerError>;
    fn sender_assets_after_total_fees(&self) -> Result<Uint<256, 4>, ConsumerError>;
    fn shares_for_receiver(&self) -> Result<Uint<256, 4>, ConsumerError>;
    fn curve_id(&self) -> Result<Uint<256, 4>, ConsumerError>;
    async fn create_deposit(
        &self,
        event: &DecodedMessage,
        backend_schema: &str,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Deposit, ConsumerError> {
        Deposit::builder()
            .id(DecodedMessage::event_id(event))
            .sender_id(self.sender()?)
            .receiver_id(self.receiver()?)
            .receiver_total_shares_in_vault(U256Wrapper::from(
                self.receiver_total_shares_in_vault()?,
            ))
            .sender_assets_after_total_fees(U256Wrapper::from(
                self.sender_assets_after_total_fees()?,
            ))
            .shares_for_receiver(U256Wrapper::from(self.shares_for_receiver()?))
            .entry_fee(U256Wrapper::from(self.entry_fee()?))
            .term_id(U256Wrapper::from(self.vault_id()?))
            .curve_id(DepositedEvent::curve_id(self)?)
            .is_triple(self.is_triple()?)
            .is_atom_wallet(self.is_atom_wallet()?)
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .block_timestamp(event.block_timestamp)
            .transaction_hash(event.transaction_hash.clone())
            .build()
            .upsert(backend_schema, tx.as_mut())
            .await
            .map_err(ConsumerError::ModelError)
    }
    async fn create_signal(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
        vault: &Vault,
    ) -> Result<(), ConsumerError> {
        if self.sender_assets_after_total_fees()? > U256::from(0) {
            let signal = if !self.is_triple()? {
                Signal::builder()
                    .id(DecodedMessage::event_id(event))
                    .account_id(self.sender()?.to_lowercase())
                    .delta(U256Wrapper::from(self.sender_assets_after_total_fees()?))
                    .atom_id(vault.term_id.clone())
                    .deposit_id(DecodedMessage::event_id(event))
                    .block_number(U256Wrapper::try_from(event.block_number)?)
                    .block_timestamp(event.block_timestamp)
                    .transaction_hash(event.transaction_hash.clone())
                    .term_id(vault.term_id.clone())
                    .curve_id(DepositedEvent::curve_id(self)?)
                    .build()
            } else {
                Signal::builder()
                    .id(DecodedMessage::event_id(event))
                    .account_id(self.sender()?.to_lowercase())
                    .delta(U256Wrapper::from(self.sender_assets_after_total_fees()?))
                    .triple_id(vault.term_id.clone())
                    .deposit_id(DecodedMessage::event_id(event))
                    .block_number(U256Wrapper::try_from(event.block_number)?)
                    .block_timestamp(event.block_timestamp)
                    .transaction_hash(event.transaction_hash.clone())
                    .term_id(vault.term_id.clone())
                    .curve_id(DepositedEvent::curve_id(self)?)
                    .build()
            };
            signal
                .upsert(
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool,
                )
                .await?;
        } else {
            info!("Sender assets after total fees is 0, nothing to do.");
        }
        Ok(())
    }
    /// This function initializes the accounts and vault
    async fn initialize_accounts_and_vault(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<Vault, ConsumerError> {
        // Create accounts
        let _sender = get_or_create_account(self.sender()?, decoded_consumer_context).await?;
        let _receiver = get_or_create_account(self.receiver()?, decoded_consumer_context).await?;

        get_or_create_vault(
            self.clone(),
            Some(event.block_number),
            decoded_consumer_context,
            if self.is_triple()? {
                TermType::Triple
            } else {
                TermType::Atom
            },
        )
        .await
    }
    /// This function formats the position ID
    fn format_position_id(&self) -> Result<String, ConsumerError> {
        Ok(format!(
            "{}-1-{}",
            self.vault_id()?,
            self.receiver()?.to_lowercase()
        ))
    }
    /// This function creates a new position
    async fn create_new_position(
        &self,
        position_id: String,
        backend_schema: &str,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Position, ConsumerError> {
        Position::builder()
            .id(position_id.clone())
            .account_id(self.receiver()?.to_lowercase())
            .term_id(U256Wrapper::from(self.vault_id()?))
            .curve_id(DepositedEvent::curve_id(self)?)
            .shares(self.receiver_total_shares_in_vault()?)
            .build()
            .upsert(backend_schema, tx.as_mut())
            .await
            .map_err(ConsumerError::ModelError)
    }
    /// This function updates the position
    async fn update_position(
        &self,
        backend_schema: &str,
        tx: &mut Transaction<'_, Postgres>,
        position: &mut Position,
    ) -> Result<Position, ConsumerError> {
        position.shares = U256Wrapper::from(self.receiver_total_shares_in_vault()?);
        position
            .upsert(backend_schema, tx.as_mut())
            .await
            .map_err(ConsumerError::ModelError)
    }
    /// This function handles the positions
    async fn handle_positions(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        tx: &mut Transaction<'_, Postgres>,
        current_share_price: Option<U256Wrapper>,
        total_shares: Option<Uint<256, 4>>,
    ) -> Result<(), ConsumerError> {
        let position_id = self.format_position_id()?;
        info!("Handling position with ID: {}", position_id);
        let position = Position::find_by_id(
            position_id.clone(),
            &decoded_consumer_context.backend_schema,
            tx.as_mut(),
        )
        .await?;

        if position.is_none() && self.receiver_total_shares_in_vault()? > U256::from(0) {
            info!("Creating new position with ID: {}", position_id);
            self.create_new_position(
                position_id.to_string(),
                &decoded_consumer_context.backend_schema,
                tx,
            )
            .await?;
        } else if let Some(mut position) = position {
            if self.receiver_total_shares_in_vault()? > U256::from(0) {
                info!(
                    "Position found, updating existing position with ID: {} and current shares: {}",
                    position_id, position.shares
                );
                if position.shares != U256Wrapper::from(self.receiver_total_shares_in_vault()?) {
                    self.update_position(
                        &decoded_consumer_context.backend_schema,
                        tx,
                        &mut position,
                    )
                    .await?;
                }
            } else {
                info!("No need to update positions, receiver total shares in vault is 0.");
            }
        } else {
            info!(
                "No need to update positions. Position not found and receiver total shares in vault is 0."
            );
        }

        if let Some(current_share_price) = current_share_price {
            if let Some(total_shares) = total_shares {
                // Update vault values
                update_vault(
                    VaultUpdate::Deposited {
                        sender_assets_after_total_fees: U256Wrapper::from(
                            self.sender_assets_after_total_fees()?,
                        ),
                    },
                    self.vault_id()?,
                    decoded_consumer_context,
                    tx,
                    current_share_price,
                    total_shares,
                )
                .await?;
            }
        }

        Ok(())
    }
}

impl DepositedEvent for &Deposited {
    fn sender(&self) -> Result<String, ConsumerError> {
        Ok(self.sender.to_string())
    }
    fn receiver(&self) -> Result<String, ConsumerError> {
        Ok(self.receiver.to_string())
    }
    fn receiver_total_shares_in_vault(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.receiverTotalSharesInVault)
    }
    fn vault_id(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.vaultId)
    }
    fn is_triple(&self) -> Result<bool, ConsumerError> {
        Ok(self.isTriple)
    }
    fn is_atom_wallet(&self) -> Result<bool, ConsumerError> {
        Ok(self.isAtomWallet)
    }
    fn entry_fee(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.entryFee)
    }
    fn sender_assets_after_total_fees(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.senderAssetsAfterTotalFees)
    }
    fn shares_for_receiver(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.sharesForReceiver)
    }
    fn curve_id(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(Uint::from(1))
    }
}

impl DepositedEvent for &DepositedV1_5 {
    fn sender(&self) -> Result<String, ConsumerError> {
        Ok(self.sender.to_string())
    }
    fn receiver(&self) -> Result<String, ConsumerError> {
        Ok(self.receiver.to_string())
    }
    fn receiver_total_shares_in_vault(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.receiverTotalSharesInVault)
    }
    fn vault_id(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.vaultId)
    }
    fn is_triple(&self) -> Result<bool, ConsumerError> {
        Ok(self.isTriple)
    }
    fn is_atom_wallet(&self) -> Result<bool, ConsumerError> {
        Ok(self.isAtomWallet)
    }
    fn entry_fee(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.entryFee)
    }
    fn sender_assets_after_total_fees(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.senderAssetsAfterTotalFees)
    }
    fn shares_for_receiver(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(self.sharesForReceiver)
    }
    fn curve_id(&self) -> Result<Uint<256, 4>, ConsumerError> {
        Ok(Uint::from(1))
    }
}
