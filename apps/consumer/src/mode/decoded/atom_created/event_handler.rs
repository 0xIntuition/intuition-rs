use super::event::AtomCreatedEvent;
use crate::{
    error::ConsumerError,
    mode::{
        decoded::utils::{EventHandler, get_block_timestamp},
        metadata::{classify_atom_data, get_supported_atom_metadata},
        resolver::types::ResolverConsumerMessage,
        types::DecodedConsumerContext,
    },
    schemas::types::DecodedMessage,
};
use models::{
    atom::Atom,
    event::{Event, EventType},
    traits::SimpleCrud,
    types::{FixedBytesWrapper, U256Wrapper},
};
use std::fmt::Debug;
use tracing::{debug, info};

#[derive(Debug)]
pub struct AtomCreatedEventHandler<T>(pub T);

impl<T> AtomCreatedEventHandler<T>
where
    T: AtomCreatedEvent + Debug + Sync + Send,
{
    /// Sends the atom to the resolver consumer. Must be called after every write this
    /// handler performs on the atom row.
    async fn enqueue_for_resolution(
        &self,
        atom: &Atom,
        decoded_consumer_context: &DecodedConsumerContext,
    ) -> Result<(), ConsumerError> {
        let message = ResolverConsumerMessage::new_atom(atom.term_id.0.to_string());
        decoded_consumer_context
            .client
            .send_message(serde_json::to_string(&message)?, None)
            .await
    }
}

impl<T> EventHandler for AtomCreatedEventHandler<T>
where
    T: AtomCreatedEvent + Debug + Sync + Send,
{
    async fn process_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        info!("Handling atom creation: {self:#?}",);

        // Check if the atom already exists, skip if it does
        match Atom::find_by_id(
            FixedBytesWrapper::from(self.0.term_id()?),
            &decoded_consumer_context.backend_schema,
            &decoded_consumer_context.pg_pool,
        )
        .await?
        {
            Some(atom) => {
                debug!("Atom already exists: {:?}", atom);
                return Ok(());
            }
            None => {
                debug!("Atom does not exist, creating it");
            }
        }

        // Get or create the vault and atom
        let mut atom = self
            .0
            .create_atom_wallet_account_and_atom(decoded_consumer_context, event)
            .await?;

        // Classify the atom data once. The same classification drives the inline
        // metadata and whether the resolver has to be involved at all.
        let data_kind = classify_atom_data(
            atom.data
                .as_deref()
                .ok_or(ConsumerError::AtomDataNotFound)?,
        )
        .await?;

        // get the supported atom metadata and update the atom metadata
        let supported_atom_metadata =
            get_supported_atom_metadata(&data_kind, &mut atom, decoded_consumer_context)
                .await?
                .update_atom_metadata(
                    &mut atom,
                    &decoded_consumer_context.backend_schema,
                    &decoded_consumer_context.pg_pool,
                )
                .await?;
        debug!("Updated atom metadata: {:?}", supported_atom_metadata);

        // Handle the account or caip10 type
        supported_atom_metadata
            .handle_account_or_caip10_type(&mut atom, decoded_consumer_context)
            .await?;
        debug!("Handled account or caip10 type");

        // Only now, after the LAST write to the atom row, hand the atom to the resolver.
        // Enqueueing earlier lets a fast resolver finish while this handler still holds a
        // stale in-memory copy, and the handler's later writes overwrite the resolver's
        // label/type (atoms end up `Resolved` + `Unknown`).
        if data_kind.needs_resolver() {
            self.enqueue_for_resolution(&atom, decoded_consumer_context)
                .await?;
        }

        // Create the event
        self.create_event(decoded_consumer_context, event).await?;

        Ok(())
    }
    async fn create_event(
        &self,
        decoded_consumer_context: &DecodedConsumerContext,
        event: &DecodedMessage,
    ) -> Result<(), ConsumerError> {
        Event::builder()
            .id(DecodedMessage::event_id(event))
            .event_type(EventType::AtomCreated)
            .atom_id(FixedBytesWrapper::from(self.0.term_id()?))
            .block_number(U256Wrapper::try_from(event.block_number)?)
            .created_at(get_block_timestamp(event.block_timestamp)?)
            .transaction_hash(event.transaction_hash.clone())
            .build()
            .upsert(
                &decoded_consumer_context.backend_schema,
                &decoded_consumer_context.pg_pool,
            )
            .await
            .map_err(ConsumerError::ModelError)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        mode::resolver::test_support::{RecordingClient, TestDb, decoded_context, term_id},
        schemas::types::ContractEvent,
        supported_contracts::v2_contract::Multivault::{self, MultivaultEvents},
    };
    use alloy::primitives::{Address, Bytes, FixedBytes};
    use models::atom::{AtomResolvingStatus, AtomType};

    fn atom_created(byte: u8, data: &str) -> (Multivault::AtomCreated, DecodedMessage) {
        let event = Multivault::AtomCreated {
            creator: Address::repeat_byte(0x11),
            termId: FixedBytes::repeat_byte(byte),
            atomData: Bytes::copy_from_slice(data.as_bytes()),
            atomWallet: Address::repeat_byte(0x22),
        };
        let message = DecodedMessage {
            body: ContractEvent::Multivault(MultivaultEvents::AtomCreated(event.clone())),
            block_hash: "0xblock".to_string(),
            block_number: 1,
            block_timestamp: 1_757_800_000,
            transaction_hash: format!("0xtx{byte:02x}"),
            log_index: 0,
            transaction_index: 0,
        };
        (event, message)
    }

    /// The resolver must only learn about an atom after this handler's LAST write to
    /// the atom row. Before the fix the message was sent from inside
    /// `get_supported_atom_metadata`, while the row still had `type=Unknown`,
    /// `label=NULL`, and two full-row upserts were still to come.
    #[tokio::test]
    async fn resolver_message_is_enqueued_only_after_the_atom_row_is_final() {
        let Some(db) = TestDb::connect().await else {
            return;
        };
        let client = RecordingClient::new(db.pool.clone(), db.schema.clone());
        let context = decoded_context(&db, client.clone());
        let (event, message) = atom_created(
            0xaa,
            r#"{"@context":"https://schema.org","@type":"Thing","name":"Privacy Stance"}"#,
        );

        AtomCreatedEventHandler(&event)
            .process_event(&context, &message)
            .await
            .unwrap();

        let atom_messages = client.atom_messages();
        assert_eq!(
            atom_messages.len(),
            1,
            "exactly one resolver message, got {:?}",
            client.sent()
        );
        let snapshot = atom_messages[0].atom_snapshot.as_ref().unwrap();
        assert_eq!(
            snapshot.atom_type,
            AtomType::Thing,
            "metadata must be persisted before the resolver is told about the atom"
        );
        assert_eq!(snapshot.label.as_deref(), Some("Privacy Stance"));
        assert_eq!(snapshot.resolving_status, AtomResolvingStatus::Pending);
        // ...and the row is never written again after the enqueue.
        assert_eq!(db.atom(&term_id(0xaa)).await, *snapshot);

        db.drop().await;
    }

    #[tokio::test]
    async fn address_atoms_are_resolved_inline_and_never_sent_to_the_atom_resolver() {
        let Some(db) = TestDb::connect().await else {
            return;
        };
        let client = RecordingClient::new(db.pool.clone(), db.schema.clone());
        let context = decoded_context(&db, client.clone());
        let (event, message) = atom_created(0xab, "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045");

        AtomCreatedEventHandler(&event)
            .process_event(&context, &message)
            .await
            .unwrap();

        assert!(
            client.atom_messages().is_empty(),
            "address atoms need no atom resolution, got {:?}",
            client.sent()
        );
        let atom = db.atom(&term_id(0xab)).await;
        assert_eq!(atom.atom_type, AtomType::Account);
        assert_eq!(atom.resolving_status, AtomResolvingStatus::Resolved);

        db.drop().await;
    }
}
