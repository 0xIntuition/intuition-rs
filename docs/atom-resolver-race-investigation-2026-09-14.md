# Atoms stuck as "Unknown" after creation — root cause and fix (2026-09-14)

Reported in Notion: *Quest 4: 28 atoms stuck as Unknown on the Portal's indexer* (GWTH-4542),
plus the Deep3 QC upload (~174 atoms) and the community atoms from 9/13.
Environments: `intuition-mainnet-nested-triples` (Portal private indexer),
`intuition-mainnet-next` (public indexer), `intuition-testnet-next`.

## TL;DR

* The IPFS fetch did **not** fail. For every stuck atom the resolver had already fetched the
  JSON and written the `json_object`, `thing`/`organization` and `atom_value` rows. Only the
  `atom.label` / `atom.type` columns were wrong.
* Root cause is a **lost-update race between the decoded consumer and the resolver consumer**
  on the `atom` row. The decoded consumer enqueues the resolver message *before* it performs
  two more full-row upserts of its stale in-memory atom (type `Unknown`, label `Unknown`,
  status `Pending`). When the resolver finishes inside that window, its metadata write is
  overwritten while its final status-only `UPDATE ... SET resolving_status='Resolved'` still
  lands, producing `Resolved` + `Unknown`.
* The same race has two other visible outcomes that have existed for months:
  `Pending` atoms that already carry a value (561 `Pending/TextObject` on the Portal indexer,
  1272 on testnet-next) and `Pending/Unknown` atoms with a `json_object`.
* Fix (consumer): enqueue the resolver message only after the decoded consumer's last write,
  write resolver-owned columns + status in one targeted `UPDATE`, and add a periodic backfill
  sweep inside the resolver that re-processes any atom whose resolution did not converge.
* Live data was repaired by re-enqueueing resolver messages (the resolver is idempotent).

## Symptom on the wire

Portal indexer, 2026-09-14, before remediation:

| resolving_status | type       | has atom_value | count |
|------------------|------------|----------------|-------|
| Resolved         | Unknown    | yes            | 79    |
| Resolved         | Unknown    | no             | 1     |
| Pending          | TextObject | yes            | 561   |
| Pending          | Unknown    | yes            | 6     |
| Pending          | Unknown    | no             | 28    |
| Pending          | Thing      | yes            | 4     |
| Pending          | JsonObject | yes            | 6     |
| Failed           | Unknown    | mostly yes     | 126   |

All 28 Quest 4 atoms: `type=Unknown, label='Unknown', resolving_status='Resolved'`, and for each
one `json_object.data` held the full schema.org JSON (`"@type":"Thing","name":"Privacy Stance"`),
`thing.name` was correct and `atom_value.thing_id` + `atom_value.json_object_id` were set.
No writer produces that state on its own; only an interleaving of two writers can.

The `Resolved/Unknown` variant only started in the week of 2026-09-07 (51 that week, 28 on 9/14).
That week is Deep3's bulk upload (13.8k / 38k / 28.8k `Thing` atoms per day on 9/9–9/11) and the
Quest 4 burst (96 atoms in ~90 s). Both consumers logged sqlx "slow statement" warnings on the
`atom` upsert during those bursts: the decoded consumer's writes got slower, which is exactly
what widens the window described below. The `Pending`-with-value variant has been present since
March (min `created_at` 2026-03-16) at lower rates.

The resolver image (3.0.69, deployed 2026-04-17) did not change in September; it did remove the
transaction around the resolver's writes (#207), which makes the resolver faster and therefore
more likely to finish inside the decoded consumer's window.

## The race, step by step

Decoded consumer, `AtomCreatedEventHandler::process_event`
(`apps/consumer/src/mode/decoded/atom_created/event_handler.rs`):

1. `create_atom_wallet_account_and_atom` — upsert #1: `Pending`, `Unknown`, label `NULL`.
2. `get_supported_atom_metadata` — for anything that is not an address / CAIP-10 / CAIP-22 /
   schema.org URL it **sends the resolver message immediately**, then parses what it can inline.
   For `ipfs://…` data that yields `AtomMetadata::unknown()` (label `"Unknown"`).
3. `update_atom_metadata` — upsert #2 of the in-memory atom: `Unknown` / `"Unknown"` / `Pending`.
4. `atom.upsert` — upsert #3, identical content (redundant).

Resolver consumer, `ResolverMessageType::process_atom` (`apps/consumer/src/mode/resolver/types.rs`),
triggered by the message from step 2:

* a. fetch IPFS, write `json_object`, `thing`, `atom_value`;
* b. `update_atom_metadata` — full-row upsert: `Thing` / `"Privacy Stance"` / `Pending`
  (status comes from the copy read before the fetch);
* c. `mark_as_resolved` — `UPDATE atom SET resolving_status='Resolved'`.

| decoded step 3/4 lands…          | final row                                   | class seen in prod           |
|----------------------------------|---------------------------------------------|------------------------------|
| before resolver b                | correct                                     | healthy                      |
| between resolver b and c         | `Resolved` + `Unknown` + label `"Unknown"`  | 80 on Portal, 44 on public   |
| after resolver c                 | `Pending` + whatever decoded had inline     | 561 `Pending/TextObject`, 6 `Pending/Unknown` with value, 4 `Pending/Thing` |

With a pinned CID the resolver round trip is ~1 s (`Processed batch of 1 messages in 1.07 s`),
and under load the decoded consumer's own upserts take seconds (`created_at` 17:42:31,
last `atom.updated_at` 17:42:36 for Privacy Stance, resolver message enqueued 17:42:32.9).

The CAIP-22 path already avoided this bug on purpose (comment in `metadata.rs`: *"We do NOT send
the message here because the atom type needs to be saved to the database first"*). The generic
path never got the same treatment.

## Why the Notion hypothesis was close but not right

"Resolver fetched before the pin propagated and never retries" describes the **`Failed`** class:
an IPFS fetch failure ends in `mark_atom_as_failed` (status `Failed`, not `Resolved`), and nothing
retried those except a deposit on the same atom. That class is real (126 on the Portal indexer,
none newer than June) and the backfill below covers it, but it is not what produced the Quest 4,
Deep3 or Featured List rows: those had the fetched content sitting in the DB.

## Fix (consumer, PR on branch `fix/atom-resolver-race-and-backfill`)

1. **Ordering** — `get_supported_atom_metadata` no longer enqueues. `process_event` classifies
   the data once (`classify_atom_data`), persists metadata, handles account/CAIP types, and only
   then enqueues the resolver message. The redundant third upsert is gone.
2. **Atomic metadata write** — `Atom::update_metadata` updates only `type, emoji, label, image,
   resolving_status` in one statement. The resolver sets `Resolved` in that same statement; the
   separate `mark_as_resolved` step is removed from the atom and CAIP-22 paths. Both consumers now
   only touch the columns they own, so a stale copy cannot resurrect other columns either.
3. **Backfill sweep** — `apps/consumer/src/mode/resolver/backfill.rs` runs inside the resolver
   consumer every `RESOLVER_BACKFILL_INTERVAL_SECS` (default 300, `0` disables) and re-processes
   up to `RESOLVER_BACKFILL_BATCH_SIZE` (default 200) atoms selected by
   `Atom::find_stale_for_resolution`:
   * `Pending` and untouched for 2 min;
   * `Resolved` with `type = 'Unknown'`;
   * `Failed`, created in the last 24 h, last attempt older than 10 min (covers "pin not
     propagated yet"; retries stop after the window so dead CIDs do not loop forever).
   Atoms without decodable `data` are skipped. A sweep item that errors is marked `Failed`, which
   puts it on the bounded retry schedule instead of being retried every sweep forever.
4. **Tests** — unit tests for the classifier; DB-backed tests (run when `TEST_DATABASE_URL` is
   set, see below) that reproduce the exact interleaving with the real model calls, assert the
   `Resolved/Unknown` outcome, and assert the sweep repairs it; plus selection-query, atomic
   update, convergence and "enqueue only after persist" tests.

```
docker run -d --name intuition-test-pg -e POSTGRES_PASSWORD=postgres -e POSTGRES_DB=storage -p 55432:5432 postgres:17
TEST_DATABASE_URL=postgres://postgres:postgres@127.0.0.1:55432/storage cargo test -p consumer
```

## Live remediation (done 2026-09-14, 20:00–20:05 UTC)

The resolver is idempotent, so the repair is simply re-enqueueing `{"message":{"Atom":"<term_id>"}}`
on `resolver_stream`. Canary: Privacy Stance on the Portal indexer, fixed in 1.07 s. Then every
atom matching `export-stuck-atom-ids.sql` (Resolved/Unknown, Pending with data, Failed with data),
ordered so the cheap classes go first:

| environment                      | re-enqueued |
|----------------------------------|-------------|
| intuition-mainnet-nested-triples | 796         |
| intuition-mainnet-next           | 1423        |
| intuition-testnet-next           | 2188        |

The id lists are in `docs/atom-resolver-race-2026-09-14/`. Results are in the section below.

## Remediation results

Counts of rows with `resolving_status <> 'Resolved' or type = 'Unknown'` after the sweeps
(2026-09-14 20:16 UTC, ~12 minutes after enqueueing):

| environment                      | before (see above)                                   | after                                                         |
|----------------------------------|------------------------------------------------------|---------------------------------------------------------------|
| intuition-mainnet-nested-triples | 80 Resolved/Unknown, 611 Pending, 126 Failed         | 0 Resolved/Unknown, 20 Pending/Unknown, 126 Failed/Unknown    |
| intuition-mainnet-next           | 44 Resolved/Unknown, 1275 Pending, 118 Failed        | 0 Resolved/Unknown, 20 Pending/Unknown + 1 Pending/TextObject (new atom), 126 Failed/Unknown |
| intuition-testnet-next           | 15 Resolved/Unknown, 1840 Pending, 348 Failed        | 0 Resolved/Unknown, 21 Pending/Unknown, 372 Failed/Unknown    |

All 28 Quest 4 atoms, Lighter and BitConnect (both indexers), the Deep3 rows and the 9/13
community atoms now carry their real label and type.

What is left, and why it is expected:

* **`Failed/Unknown`** — re-tried and failed again. 113 of the 126 on the Portal indexer are
  schema.org `@type: Movie` documents (unsupported type; resolver logs
  `Unsupported schema.org type: Movie`), the rest are dead CIDs or unsupported JSON. These are
  correctly `Failed`; supporting `Movie` is a product decision, not a bug.
* **`Pending/Unknown` with `data IS NULL`** (14 on the Portal indexer) — on-chain `atomData`
  that does not decode as UTF-8 text. Nothing to resolve; they were not re-enqueued.
* **`Pending/Unknown` with `ipfs://` data and a `json_object` row** (6 on the Portal indexer)
  — a **second, unrelated bug**: the resolver's `thing` upsert fails with
  `index row size 3416 exceeds btree version 4 maximum 2704 for index "idx_thing_description"`.
  A plain btree on a free-text column rejects long descriptions. The message is retried 5 times
  and then dropped as a poison pill, so the atom never converges. Fixed by migration
  `1789420000000_drop_description_btree_indexes` (`release_notes/hasura-migrations-3.4.1.md`);
  re-enqueue those atoms once it is deployed.

Side finding fixed in the same PR: atoms whose schema.org JSON has `"image": ""` made the
resolver send an empty `{"image":""}` message to the IPFS upload consumer on every resolution.
The resolver now skips empty images.

## How to check an environment

```sql
select resolving_status, type, count(*) from atom
where resolving_status <> 'Resolved' or type = 'Unknown' group by 1,2 order by 3 desc;
```

Anything `Resolved/Unknown` with data, or `Pending` older than a few minutes, means the race
hit again (before the fix ships) or the backfill is disabled.
