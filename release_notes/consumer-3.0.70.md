# consumer:3.0.70

**Date:** 2026-09-14

## Fix: atoms left `Resolved` + `Unknown` (or stuck `Pending`) by a decoded/resolver race

### Problem

`AtomCreatedEventHandler::process_event` enqueued the resolver message inside
`get_supported_atom_metadata`, **before** performing two more full-row upserts of its in-memory
atom (`type=Unknown, label='Unknown', resolving_status=Pending`). When the resolver finished its
IPFS fetch inside that window (about 1 s for a pinned CID), the resolver's label/type write was
overwritten by the decoded consumer's stale copy while the resolver's separate
`mark_as_resolved` still landed. Depending on the exact interleaving the atom ended as
`Resolved/Unknown/'Unknown'` or as `Pending` with correct content.

Observed on mainnet: 80 `Resolved/Unknown` atoms on the Portal indexer (Quest 4, Deep3 upload,
Featured Lists), 44 on the public indexer, and 561 / 1131 / 1272 `Pending/TextObject` atoms on
mainnet-nested-triples / mainnet-next / testnet-next respectively.

Full write-up: `docs/atom-resolver-race-investigation-2026-09-14.md`.

### Fix

- Decoded consumer enqueues the resolver message only **after** its last write to the atom row
  (`classify_atom_data` + `AtomDataKind::needs_resolver`). The redundant third upsert is removed.
- `Atom::update_metadata` writes `type, emoji, label, image, resolving_status` in one targeted
  `UPDATE`; the resolver sets `Resolved` in that same statement (no more separate
  `mark_as_resolved` for atom / CAIP-22 resolution).
- New resolver backfill sweep (`mode/resolver/backfill.rs`) that re-processes atoms whose
  resolution did not converge.

### New environment variables (resolver consumer)

| variable                           | default | meaning                                              |
|------------------------------------|---------|------------------------------------------------------|
| `RESOLVER_BACKFILL_INTERVAL_SECS`  | `300`   | seconds between sweeps; `0` disables the sweep       |
| `RESOLVER_BACKFILL_BATCH_SIZE`     | `200`   | max atoms re-processed per sweep                     |
| `RESOLVER_BACKFILL_CONCURRENCY`    | `4`     | atoms resolved concurrently inside one sweep         |

Sweep selection: `Pending` untouched for 2 min; `Resolved` with `type='Unknown'`; `Failed`
created in the last 24 h whose last attempt is older than 10 min. Atoms with no decodable
`data` are skipped. An item that errors during the sweep is marked `Failed` so it follows the
bounded retry schedule.

### Also in this release

- The resolver no longer sends an empty `{"image":""}` message to the IPFS upload consumer for
  schema.org objects whose `image` is an empty string.
- Paired with `hasura-migrations-3.4.1` (drops the btree indexes on `description` columns that
  made `thing` upserts fail for long descriptions).

### Changes

- `apps/models/src/atom.rs` — `Atom::update_metadata`, `Atom::find_stale_for_resolution`,
  `StaleResolutionParams`
- `apps/consumer/src/mode/metadata.rs` — `AtomDataKind`, `classify_atom_data`;
  `get_supported_atom_metadata` no longer enqueues; `update_atom_metadata` uses the targeted
  update
- `apps/consumer/src/mode/decoded/atom_created/event_handler.rs` — enqueue after persist
- `apps/consumer/src/mode/resolver/types.rs` — status written with metadata
- `apps/consumer/src/mode/resolver/backfill.rs` — new sweep, spawned from
  `ConsumerMode::process_messages`
- `apps/consumer/src/config.rs` — new env vars

### Build

Built by the new `Build consumer image` workflow (`.github/workflows/build-consumer.yml`,
`workflow_dispatch` with `version=3.0.70`): native `linux/amd64` + `linux/arm64` on GitHub-hosted
runners, merged into one multi-arch tag `ghcr.io/0xintuition/consumer:3.0.70`.

### Impact

- **Scope:** decoded consumer (AtomCreated) and resolver consumer (all environments).
- **Data repair:** live rows were fixed by re-enqueueing resolver messages on 2026-09-14; after
  deployment the sweep keeps every environment converged without manual action.
