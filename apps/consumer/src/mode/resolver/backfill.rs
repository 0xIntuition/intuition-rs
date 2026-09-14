//! Periodic safety net for atom resolution.
//!
//! Atom metadata is written by two consumers (decoded + resolver). Even with the
//! enqueue-after-persist ordering in the decoded consumer, an atom can still end up
//! unresolved: a lost stream message, a resolver crash mid-batch, or IPFS content that
//! was not pinned yet when the first attempt ran. This sweep runs inside the resolver
//! consumer and re-processes such atoms through the exact same code path as a stream
//! message (`ResolverMessageType::Atom(..).process`), which is idempotent.
//!
//! Selection lives in [`Atom::find_stale_for_resolution`]; see
//! `docs/atom-resolver-race-investigation-2026-09-14.md` for the incident that
//! motivated it.
use crate::{
    error::ConsumerError,
    mode::{resolver::types::ResolverMessageType, types::ResolverConsumerContext},
    traits::AtomUpdater,
};
use futures::{StreamExt, stream};
use models::{
    atom::{Atom, StaleResolutionParams},
    traits::SimpleCrud,
    types::FixedBytesWrapper,
};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, warn};

/// Seconds between two sweeps unless `RESOLVER_BACKFILL_INTERVAL_SECS` overrides it.
pub const DEFAULT_INTERVAL_SECS: u64 = 300;
/// Max atoms re-processed per sweep unless `RESOLVER_BACKFILL_BATCH_SIZE` overrides it.
pub const DEFAULT_BATCH_SIZE: i64 = 200;
/// Concurrent resolutions per sweep unless `RESOLVER_BACKFILL_CONCURRENCY` overrides it.
pub const DEFAULT_CONCURRENCY: usize = 4;
/// A `Pending` atom untouched for this long is considered stuck.
pub const PENDING_MIN_AGE_SECS: i64 = 120;
/// `Failed` atoms are retried only while younger than this.
pub const FAILED_RETRY_WINDOW_HOURS: i64 = 24;
/// `Failed` atoms are retried at most once per this interval.
pub const FAILED_MIN_AGE_SECS: i64 = 600;

/// Outcome of one sweep.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BackfillReport {
    /// Atoms returned by the selection query.
    pub selected: usize,
    /// Atoms the resolver processed without error (resolved *or* deliberately marked
    /// `Failed` because their content is not supported).
    pub processed: usize,
    /// Atoms whose processing returned an error; they are marked `Failed` so they
    /// follow the bounded retry schedule instead of being retried every sweep forever.
    pub errored: usize,
}

/// Spawns the sweep loop on the current runtime. Returns immediately.
pub fn spawn(context: ResolverConsumerContext) {
    tokio::spawn(run_loop(context));
}

/// Runs sweeps forever, sleeping `RESOLVER_BACKFILL_INTERVAL_SECS` between them.
/// The first sweep runs one interval after startup so a restarting pod first drains
/// its stream backlog.
pub async fn run_loop(context: ResolverConsumerContext) {
    let interval_secs = context
        .server_initialize
        .env
        .resolver_backfill_interval_secs
        .unwrap_or(DEFAULT_INTERVAL_SECS);
    if interval_secs == 0 {
        info!("Resolver backfill sweep disabled (RESOLVER_BACKFILL_INTERVAL_SECS=0)");
        return;
    }
    info!(
        "Resolver backfill sweep enabled: every {interval_secs}s, {:?}",
        params(&context)
    );
    let interval = Duration::from_secs(interval_secs);
    loop {
        sleep(interval).await;
        match run_once(&context).await {
            Ok(report) if report.selected == 0 => {
                debug!("Resolver backfill sweep: nothing to do")
            }
            Ok(report) => info!("Resolver backfill sweep: {report:?}"),
            Err(e) => warn!("Resolver backfill sweep failed: {e}"),
        }
    }
}

/// Selection thresholds for this consumer.
pub fn params(context: &ResolverConsumerContext) -> StaleResolutionParams {
    StaleResolutionParams {
        pending_min_age_secs: PENDING_MIN_AGE_SECS,
        failed_retry_window_hours: FAILED_RETRY_WINDOW_HOURS,
        failed_min_age_secs: FAILED_MIN_AGE_SECS,
        limit: context
            .server_initialize
            .env
            .resolver_backfill_batch_size
            .unwrap_or(DEFAULT_BATCH_SIZE),
    }
}

/// Runs a single sweep: selects stale atoms and re-processes them.
pub async fn run_once(context: &ResolverConsumerContext) -> Result<BackfillReport, ConsumerError> {
    let stale =
        Atom::find_stale_for_resolution(context.backend_schema(), &params(context), context.pool())
            .await?;
    let mut report = BackfillReport {
        selected: stale.len(),
        ..Default::default()
    };
    if stale.is_empty() {
        return Ok(report);
    }

    let concurrency = context
        .server_initialize
        .env
        .resolver_backfill_concurrency
        .unwrap_or(DEFAULT_CONCURRENCY)
        .max(1);

    let outcomes: Vec<bool> = stream::iter(stale)
        .map(|term_id| async move { resolve_one(context, &term_id).await.is_ok() })
        .buffer_unordered(concurrency)
        .collect()
        .await;

    for ok in outcomes {
        if ok {
            report.processed += 1;
        } else {
            report.errored += 1;
        }
    }
    Ok(report)
}

/// Re-processes one atom through the regular resolver path. On error the atom is
/// marked `Failed` (best effort) so the retry schedule stays bounded.
async fn resolve_one(
    context: &ResolverConsumerContext,
    term_id: &FixedBytesWrapper,
) -> Result<(), ConsumerError> {
    let atom_id = term_id.0.to_string();
    match ResolverMessageType::Atom(atom_id.clone())
        .process(context)
        .await
    {
        Ok(()) => {
            debug!("Backfill re-processed atom {atom_id}");
            Ok(())
        }
        Err(e) => {
            warn!(
                "Backfill could not resolve atom {atom_id}: {e}. Marking it Failed so it follows the bounded retry schedule"
            );
            match Atom::find_by_id(term_id.clone(), context.backend_schema(), context.pool()).await
            {
                Ok(Some(atom)) => {
                    if let Err(mark_err) = atom
                        .mark_as_failed(context.backend_schema(), context.pool())
                        .await
                    {
                        warn!("Backfill could not mark atom {atom_id} as Failed: {mark_err}");
                    }
                }
                Ok(None) => warn!("Backfill: atom {atom_id} disappeared while resolving"),
                Err(find_err) => warn!("Backfill could not reload atom {atom_id}: {find_err}"),
            }
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::resolver::test_support::{
        AtomSeed, RecordingClient, TestDb, resolver_context, term_id,
    };
    use models::{
        atom::{AtomResolvingStatus, AtomType},
        atom_value::AtomValue,
        thing::Thing,
    };
    use std::collections::BTreeSet;

    const THING_JSON: &str = r#"{"@context":"https://schema.org","@type":"Thing","name":"Privacy Stance","description":"A statement about personal privacy"}"#;
    const UNSUPPORTED_JSON: &str =
        r#"{"@context":"https://schema.org","@type":"Article","name":"not supported"}"#;
    const CAIP22: &str = "caip22:eip155:1/erc721:0x8004AA63c570c570eBF15376c0dB199918BFe9Fb/1";

    fn ids(v: &[FixedBytesWrapper]) -> BTreeSet<String> {
        v.iter().map(|i| i.0.to_string()).collect()
    }

    fn strict_params() -> StaleResolutionParams {
        StaleResolutionParams {
            pending_min_age_secs: 120,
            failed_retry_window_hours: 24,
            failed_min_age_secs: 600,
            limit: 100,
        }
    }

    #[tokio::test]
    async fn selection_picks_only_atoms_whose_resolution_did_not_converge() {
        let Some(db) = TestDb::connect().await else {
            return;
        };
        let resolved_unknown = db
            .seed_atom(
                &AtomSeed::new(0x01, Some("ipfs://bafy1"))
                    .status("Resolved")
                    .label(Some("Unknown")),
            )
            .await;
        let pending_old = db
            .seed_atom(
                &AtomSeed::new(0x02, Some("hello"))
                    .atom_type("TextObject")
                    .updated_secs_ago(600),
            )
            .await;
        let pending_fresh = db
            .seed_atom(
                &AtomSeed::new(0x03, Some("hello"))
                    .atom_type("TextObject")
                    .updated_secs_ago(5),
            )
            .await;
        let failed_recent = db
            .seed_atom(
                &AtomSeed::new(0x04, Some("ipfs://bafy4"))
                    .status("Failed")
                    .created_secs_ago(3_600)
                    .updated_secs_ago(1_800),
            )
            .await;
        let failed_old = db
            .seed_atom(
                &AtomSeed::new(0x05, Some("ipfs://bafy5"))
                    .status("Failed")
                    .created_secs_ago(3 * 86_400)
                    .updated_secs_ago(3 * 86_400),
            )
            .await;
        let failed_just_tried = db
            .seed_atom(
                &AtomSeed::new(0x06, Some("ipfs://bafy6"))
                    .status("Failed")
                    .created_secs_ago(3_600)
                    .updated_secs_ago(60),
            )
            .await;
        let healthy = db
            .seed_atom(
                &AtomSeed::new(0x07, Some(THING_JSON))
                    .atom_type("Thing")
                    .status("Resolved")
                    .label(Some("x"))
                    .updated_secs_ago(600),
            )
            .await;
        let no_data = db
            .seed_atom(&AtomSeed::new(0x08, None).status("Resolved"))
            .await;
        let empty_data = db
            .seed_atom(&AtomSeed::new(0x09, Some("")).updated_secs_ago(600))
            .await;

        let got = Atom::find_stale_for_resolution(&db.schema, &strict_params(), &db.pool)
            .await
            .unwrap();
        assert_eq!(
            ids(&got),
            ids(&[
                resolved_unknown.clone(),
                pending_old.clone(),
                failed_recent.clone()
            ]),
            "excluded: fresh pending {pending_fresh:?}, failed outside window {failed_old:?}, failed just tried {failed_just_tried:?}, healthy {healthy:?}, no data {no_data:?}, empty data {empty_data:?}"
        );

        // LIMIT applies newest-created first: the two atoms created "now" win over the
        // one created an hour ago.
        let limited = Atom::find_stale_for_resolution(
            &db.schema,
            &StaleResolutionParams {
                limit: 2,
                ..strict_params()
            },
            &db.pool,
        )
        .await
        .unwrap();
        assert_eq!(
            ids(&limited),
            ids(&[resolved_unknown.clone(), pending_old.clone()])
        );

        db.drop().await;
    }

    #[tokio::test]
    async fn update_metadata_touches_only_resolver_owned_columns() {
        let Some(db) = TestDb::connect().await else {
            return;
        };
        let id = db.seed_atom(&AtomSeed::new(0x10, Some("hello"))).await;
        let before = db.atom(&id).await;

        let mut atom = before.clone();
        atom.atom_type = AtomType::Thing;
        atom.label = Some("Label".into());
        atom.emoji = Some("🧩".into());
        atom.image = Some("ipfs://img".into());
        atom.resolving_status = AtomResolvingStatus::Resolved;
        // Poison every column the metadata write does not own: none of these may reach
        // the database.
        atom.wallet_id = "poison".into();
        atom.creator_id = "poison".into();
        atom.data = Some("poison".into());
        atom.raw_data = "poison".into();
        atom.transaction_hash = "poison".into();
        atom.log_index = 99;

        assert_eq!(atom.update_metadata(&db.schema, &db.pool).await.unwrap(), 1);

        let after = db.atom(&id).await;
        assert_eq!(after.atom_type, AtomType::Thing);
        assert_eq!(after.label.as_deref(), Some("Label"));
        assert_eq!(after.emoji.as_deref(), Some("🧩"));
        assert_eq!(after.image.as_deref(), Some("ipfs://img"));
        assert_eq!(after.resolving_status, AtomResolvingStatus::Resolved);
        assert_eq!(after.wallet_id, before.wallet_id);
        assert_eq!(after.creator_id, before.creator_id);
        assert_eq!(after.data, before.data);
        assert_eq!(after.raw_data, before.raw_data);
        assert_eq!(after.transaction_hash, before.transaction_hash);
        assert_eq!(after.log_index, before.log_index);

        let mut missing = after.clone();
        missing.term_id = term_id(0xEE);
        assert_eq!(
            missing.update_metadata(&db.schema, &db.pool).await.unwrap(),
            0
        );

        db.drop().await;
    }

    /// Replays the pre-fix interleaving with the real model calls, asserts it yields the
    /// exact production state (`Resolved` + `Unknown` + label "Unknown"), then asserts
    /// the sweep repairs it through the regular resolver path.
    #[tokio::test]
    async fn reproduces_the_race_and_the_sweep_repairs_it() {
        let Some(db) = TestDb::connect().await else {
            return;
        };
        let client = RecordingClient::new(db.pool.clone(), db.schema.clone());
        let context = resolver_context(&db, client.clone());

        // Decoded consumer upsert #1: Pending / Unknown, data decoded.
        let id = db.seed_atom(&AtomSeed::new(0x20, Some(THING_JSON))).await;

        // Decoded consumer's in-memory copy after its inline pass over an IPFS URI.
        let mut decoded_copy = db.atom(&id).await;
        decoded_copy.atom_type = AtomType::Unknown;
        decoded_copy.label = Some("Unknown".into());
        decoded_copy.emoji = Some("❓".into());
        decoded_copy.resolving_status = AtomResolvingStatus::Pending;

        // Old resolver: full-row upsert of the resolved metadata (status still Pending
        // because it was read before the fetch)...
        let mut resolver_copy = db.atom(&id).await;
        resolver_copy.atom_type = AtomType::Thing;
        resolver_copy.label = Some("Privacy Stance".into());
        resolver_copy.emoji = Some("🧩".into());
        resolver_copy.upsert(&db.schema, &db.pool).await.unwrap();
        // ...the decoded consumer's trailing upsert lands in between...
        decoded_copy.upsert(&db.schema, &db.pool).await.unwrap();
        // ...and the resolver's separate status-only update lands last.
        resolver_copy
            .mark_as_resolved(&db.schema, &db.pool)
            .await
            .unwrap();

        let stuck = db.atom(&id).await;
        assert_eq!(
            (
                stuck.resolving_status.clone(),
                stuck.atom_type.clone(),
                stuck.label.as_deref()
            ),
            (
                AtomResolvingStatus::Resolved,
                AtomType::Unknown,
                Some("Unknown")
            ),
            "the interleaving must reproduce the production state"
        );

        // The sweep picks it up and re-resolves it.
        let report = run_once(&context).await.unwrap();
        assert_eq!(
            report,
            BackfillReport {
                selected: 1,
                processed: 1,
                errored: 0
            }
        );
        let fixed = db.atom(&id).await;
        assert_eq!(fixed.atom_type, AtomType::Thing);
        assert_eq!(fixed.label.as_deref(), Some("Privacy Stance"));
        assert_eq!(fixed.emoji.as_deref(), Some("🧩"));
        assert_eq!(fixed.resolving_status, AtomResolvingStatus::Resolved);
        let thing = Thing::find_by_id(id.clone(), &db.schema, &db.pool)
            .await
            .unwrap()
            .expect("thing row");
        assert_eq!(thing.name.as_deref(), Some("Privacy Stance"));
        let value = AtomValue::find_by_id(id.clone(), &db.schema, &db.pool)
            .await
            .unwrap()
            .expect("atom_value row");
        assert_eq!(value.thing_id, Some(id.clone()));
        assert_eq!(value.json_object_id, Some(id.clone()));

        // Converged: nothing is selected any more, and no side messages were sent
        // (the atom has no image).
        assert_eq!(run_once(&context).await.unwrap(), BackfillReport::default());
        assert!(client.sent().is_empty(), "{:?}", client.sent());

        db.drop().await;
    }

    #[tokio::test]
    async fn unresolvable_atoms_are_marked_failed_and_retried_only_inside_the_window() {
        let Some(db) = TestDb::connect().await else {
            return;
        };
        let client = RecordingClient::new(db.pool.clone(), db.schema.clone());
        let context = resolver_context(&db, client);

        // Resolves to `Unknown` (unsupported schema.org type): the resolver marks it Failed.
        let unsupported = db
            .seed_atom(&AtomSeed::new(0x30, Some(UNSUPPORTED_JSON)).updated_secs_ago(600))
            .await;
        // Errors out (no RPC configured for chain 1): the sweep marks it Failed.
        let caip22 = db
            .seed_atom(
                &AtomSeed::new(0x31, Some(CAIP22))
                    .atom_type("Caip22")
                    .updated_secs_ago(600),
            )
            .await;
        // Failed long ago: outside the retry window, must never be selected again.
        let expired = db
            .seed_atom(
                &AtomSeed::new(0x32, Some("ipfs://bafy32"))
                    .status("Failed")
                    .created_secs_ago(3 * 86_400)
                    .updated_secs_ago(2 * 86_400),
            )
            .await;

        let report = run_once(&context).await.unwrap();
        assert_eq!(
            report,
            BackfillReport {
                selected: 2,
                processed: 1,
                errored: 1
            }
        );
        assert_eq!(
            db.atom(&unsupported).await.resolving_status,
            AtomResolvingStatus::Failed
        );
        assert_eq!(
            db.atom(&caip22).await.resolving_status,
            AtomResolvingStatus::Failed
        );
        assert_eq!(
            db.atom(&expired).await.resolving_status,
            AtomResolvingStatus::Failed
        );

        // Just retried: not selected again until FAILED_MIN_AGE_SECS has elapsed.
        assert_eq!(run_once(&context).await.unwrap(), BackfillReport::default());

        db.drop().await;
    }
}
