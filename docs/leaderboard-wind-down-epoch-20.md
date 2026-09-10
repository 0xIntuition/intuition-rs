# Leaderboard Wind-Down — Epoch 20 Freeze (GWTH-4354)

**Date:** 2026-09-10
**Ticket:** GWTH-4354
**Migration:** `infrastructure/hasura/migrations/intuition/1771526444000_leaderboard_wind_down_epoch_20`

## Decision

The Season 2 leaderboard (`portal.intuition.systems/leaderboard`) is winding down.
**Epoch 20 (2026-08-11 → 2026-08-25) is the final epoch that pays out leaderboard
IQ.** From Epoch 21 onward, `settle_season2_epoch` no longer inserts
`leaderboard_pnl` or `leaderboard_roi` entries into `season2_iq_ledger`.

**Fee IQ (`entry_type = 'fee'`, protocol fees converted to IQ) continues
unchanged for every epoch, including 21+.** These are separate entry types
awarded by the same settlement function; only the two leaderboard entry types
are frozen.

This corrects and supersedes the recommendations in
`docs/leaderboard-wind-down-assessment-2026-08-12.md` — see the "Update
2026-09-10" section appended to that doc.

## Live-DB evidence (verified 2026-09-10)

Checked across `intuition-mainnet-next`, `intuition-mainnet-nested-triples`
and `intuition-testnet-next`:

| Fact | Value |
|---|---|
| `season2_iq_ledger` row count | 0 in every environment |
| `season2_epoch.settled_at` | NULL for every epoch — settlement has never run |
| `season2_trust_price_snapshot` | healthy, still capturing (last snapshot 2026-09-10 12:00 UTC, ~400 rows) |
| `protocol_fee_accrued` | accruing normally (epoch 22 has 149,135 rows) |
| `season2_epoch` calendar | epoch 20 = 2026-08-11 → 2026-08-25; epoch 21 = 2026-08-25 → 2026-09-08; epoch 22 = 2026-09-08 → 2026-09-22 |

Because `season2_iq_ledger` is empty everywhere, this migration's cleanup
`DELETE` (reversing any leaderboard IQ already settled past the cutoff) has
**zero rows to act on today** — see "No-op in practice" below.

## Why the price-capture CronJobs were deliberately NOT suspended

The original 2026-08-12 assessment recommended stopping the 12h TRUST/USD
price-capture CronJobs as part of a clean wind-down (its step 3, item 2).
**That recommendation was wrong and has been overridden.**

Fee IQ is computed as:

```
fee_iq = (amount_trust × average_trust_usd_for_epoch) × 2000
```

`average_trust_usd_for_epoch` comes from `AVG(price_usd)` over
`season2_trust_price_snapshot` rows inside the epoch's `[start_at, end_at)`
window. If that window has **zero snapshots**, `settle_season2_epoch` raises:

```
No TRUST/USD snapshots found for Season 2 epoch %
```

Snapshots are point-in-time captures — there is no way to backfill a missed
12h window after the fact. If the CronJobs had been suspended, every epoch
from the suspension date onward would have a permanent snapshot gap, and
**fee IQ for those epochs could never be settled**, not even manually,
because there is no average price to compute it from. Since fee IQ must
keep working for epoch 21+ (see Decision above), suspending price capture
would have silently broken the one thing this wind-down explicitly needs to
keep working.

The correction: **the price-capture CronJobs stay running** in all three
namespaces (`intuition-mainnet-next`, `intuition-mainnet-nested-triples`,
`intuition-testnet-next`). This was approved by the ticket owner. The cost
of leaving them running is negligible — one small table, one CoinGecko call
every 12h, no relation to the leaderboard read path being wound down.

## Why the Hasura leaderboard functions were NOT untracked

The original assessment also suggested untracking the leaderboard read
functions from Hasura metadata (step 3, item 3) once the frontend removes
the live leaderboard page. **That is not being done, for two separate
reasons — one about what the Epoch 1-20 archive needs, and one about the
all-time functions nobody uses.**

### 1. The Epochs 1-20 archive needs `get_pnl_leaderboard_period_min_threshold` at runtime

The frontend is keeping a read-only historical archive of Epochs 1-20 (the
epochs that actually paid out). That archive is not a static snapshot — it
queries live at render time. The call chain is:

```
portal GraphQL query
  -> Hasura Action `get_pnl_leaderboard_period_min_threshold`
     (infrastructure/hasura/metadata/actions.yaml:73,
      handler: '{{chartApiUrl}}', timeout: 300)
  -> chart-api REST: GET /api/v1/leaderboard/pnl/period/min-threshold
     (apps/chart-api/src/app.rs, apps/chart-api/src/endpoints/leaderboard.rs)
  -> Postgres function `get_pnl_leaderboard_period`
```

`get_pnl_leaderboard_period_min_threshold` is a **Hasura Action**, not a
tracked Postgres function — it does not appear in `pg_proc` in either portal
database. It is a GraphQL field whose resolver is an HTTP request-transform
to chart-api, which in turn calls the Postgres function. Untracking anything
in this chain (the Action, or the chart-api route) breaks every Epoch 1-20
archive page load, for every epoch, indefinitely — not just for future
epochs. Nothing in this chain is touched by this migration or by this
wind-down.

### 2. The four all-time leaderboard functions are unused by the portal — left tracked anyway

Separately, `infrastructure/hasura/metadata/databases/intuition/functions/functions.yaml`
tracks four all-time (not period-scoped) leaderboard functions:
`get_pnl_leaderboard`, `get_pnl_leaderboard_stats`, `get_account_pnl_rank`,
`get_vault_leaderboard`. These are **not used by the current portal** — the
only local references are the portal's introspected `schema.graphql` and a
dead codegen file (`app/lib/graphql/queries/leaderboard.graphql`) that no
route imports.

Decision: **leave them tracked.** They are fields on the public GraphQL
schema; external consumers of the API cannot be ruled out; untracking them
is a breaking API change that buys nothing for this ticket (the IQ cutoff is
enforced in `settle_season2_epoch`, not in any read path); and there is no
evidence-gathering done yet on who might be calling them. If they are ever
removed, that should be its own ticket with a deprecation notice and at
least a day of Hasura query-log sampling first, not a side effect of this
migration.

## What the migration does

`infrastructure/hasura/migrations/intuition/1771526444000_leaderboard_wind_down_epoch_20`:

1. Adds `season2_last_leaderboard_epoch() RETURNS INTEGER` — a single,
   greppable, commented cutoff constant (`SELECT 20`) instead of a bare `20`
   buried inside `settle_season2_epoch`'s body.
2. `CREATE OR REPLACE FUNCTION settle_season2_epoch(...)` with the identical
   signature and `RETURNS TABLE` column list as the original definition in
   `1771526400000_add_season2_iq_points_mvp` (Postgres rejects a
   `CREATE OR REPLACE` that changes a function's return type). The body is
   copied verbatim with exactly one behavioural change: the
   `leaderboard_pnl` and `leaderboard_roi` INSERT blocks are wrapped in
   `IF p_epoch <= season2_last_leaderboard_epoch() THEN ... END IF;`. Epoch
   validation, snapshot averaging, the `fee` INSERT block, the
   `settled_at` update, and the final recompute-and-return are byte-for-byte
   unchanged — settling epoch 21+ still succeeds and still awards fee IQ.
3. An idempotent `DELETE FROM season2_iq_ledger WHERE epoch >
   season2_last_leaderboard_epoch() AND entry_type IN ('leaderboard_pnl',
   'leaderboard_roi')` to reverse any leaderboard IQ already settled past
   the cutoff.
4. Does **not** touch `season2_epoch` — epochs 21-30 stay in the table so
   fee IQ can still be settled for them.

## "Reverse leaderboard IQ already settled for Epoch 21+" was a no-op in practice

The ticket requires that any leaderboard IQ already settled for Epoch 21+ be
reversed. Per the live-DB evidence above, `season2_iq_ledger` is **empty in
every environment** — settlement has never been run, so there is nothing to
reverse today. In practice, step 3 of the migration deletes 0 rows right
now.

It is still implemented as a real, idempotent `DELETE` (not skipped or left
as a TODO) so the guarantee holds if this migration lands after someone
manually runs `settle_season2_epoch` for epoch 21+ before it does, and so
re-applying the migration is always safe.

## Two pre-existing bugs found while validating this migration

Building `scripts/season2_verify_leaderboard_cutoff.sql` meant calling
`settle_season2_epoch` for real for the first time ever, against
`intuition-testnet-next`. It failed — twice, in two different ways, neither
related to the leaderboard cutoff. Both are fixed in this migration's `up.sql`
(changes 4 and 5 in its header) because an unfixed `settle_season2_epoch`
cannot award fee IQ for epoch 21+ either, which is the one thing this
wind-down explicitly needs to keep working. Both predate this migration —
they are present verbatim in `1771526400000_add_season2_iq_points_mvp` — and
were never caught in any environment because `settle_season2_epoch` has never
been successfully run anywhere (see the evidence table above). Between them,
the original, unpatched function could never successfully settle any epoch at
all, for any reason, ever — not just epoch 21+.

1. **`ON CONFLICT (epoch, ...)` is ambiguous.** `settle_season2_epoch`'s
   `RETURNS TABLE` declares its first OUT column as `epoch`, which PL/pgSQL
   treats as an implicit variable in scope for the whole function body. Every
   `ON CONFLICT (epoch, ...)` clause in the function (there are four: one in
   the `season2_epoch_price` upsert, three in the `season2_iq_ledger`
   inserts) then has a genuine ambiguity — Postgres can't tell whether
   `epoch` means the OUT variable or the table column — and raises `column
   reference "epoch" is ambiguous`. Fix: `#variable_conflict use_column` at
   the top of the function body, which tells PL/pgSQL to prefer the column
   interpretation everywhere in the function. Safe here because the function
   never intentionally references its OUT columns by their bare names (it
   consistently uses `v_`-prefixed locals and `p_`-prefixed parameters
   instead).
2. **`get_pnl_leaderboard_period`'s temp tables collide on a second call
   in the same transaction.** That function creates seven `ON COMMIT DROP`
   temp tables (`_tmp_active_accounts`, `_tmp_position_data`,
   `_tmp_prices_at_start`, `_tmp_prices_at_end`, `_tmp_vault_state_at_start`,
   `_tmp_vault_state_at_end`, `_tmp_realized_fifo`). `ON COMMIT DROP` only
   fires at actual transaction commit — never between statements within the
   same transaction/session. `settle_season2_epoch` calls
   `get_pnl_leaderboard_period` twice every time it settles an epoch at or
   under the cutoff (once for the nominal-PnL board, once for the ROI
   board), so the second call's `CREATE TEMP TABLE` always failed with
   `relation "..." already exists` — discovered one table at a time against
   `intuition-testnet-next` as each was reached. The same collision also
   hits a *later* `settle_season2_epoch` call in the same session, against
   temp tables left behind by an *earlier* call's ROI board query. Fix:
   `DROP TABLE IF EXISTS` for all seven, added in `settle_season2_epoch` in
   two places — right before the pnl block, and again between the pnl and
   roi blocks — so every `get_pnl_leaderboard_period` call starts clean
   regardless of what ran before it in the same session. This only touches
   `settle_season2_epoch`'s own body; `get_pnl_leaderboard_period` itself
   (a live, portal-facing function) is untouched, since those temp tables
   are self-contained scratch state used only inside its own body.

Both fixes are pre-existing-bug fixes, not part of the leaderboard cutoff
behaviour itself — `down.sql` deliberately reintroduces both when rolling
back, since a true revert to the pre-migration `settle_season2_epoch` has to
restore its pre-migration body verbatim, bugs included (see `down.sql`'s own
comments).

A third thing surfaced while building the verification script itself, not
the migration: `protocol_fee_accrued.epoch` turns out **not** to follow
`season2_epoch`'s 8-30 calendar. It's a separate, far more finely grained,
densely populated counter — real `intuition-testnet-next` data has rows for
nearly every integer from 1 to 322+ as of 2026-09-10. This is why the
verification script's synthetic fixture epochs are `-7` and `90000041`
rather than small positive numbers like `7` and `41` (an earlier version
used those and the byte-identical-fee assertion failed, correctly, because
real `protocol_fee_accrued` rows at epoch 7 and 41 leaked into the totals).
This is worth knowing about independently of this ticket: it means
`settle_season2_epoch`'s fee INSERT (`WHERE pfa.epoch = p_epoch::NUMERIC`)
has always been matching `protocol_fee_accrued` rows against a number that
isn't actually a Season 2 epoch number in the sense `season2_epoch` defines
it. Whether that's the intended design (`protocol_fee_accrued.epoch` means
something else that was always meant to be compared directly, e.g. a raw
on-chain epoch counter that happens to be fed the same integers) or a
latent correctness bug in the fee IQ formula is a separate, substantive
question this ticket did not investigate further and does not change —
fixing or even fully diagnosing it would mean understanding what emits
`protocol_fee_accrued` and what "epoch" is supposed to mean there, which is
out of scope for a migration that only touches the leaderboard cutoff. Flagging
it here so it isn't lost.

## Validation

`scripts/season2_verify_leaderboard_cutoff.sql` — a self-contained,
`BEGIN; ... ROLLBACK;`-wrapped differential test — proves the guard is
selective (blocks leaderboard IQ, not fee IQ) rather than the function being
broken. See the script header for the fixture design and
`docs/leaderboard-wind-down-epoch-20.md`'s sibling migration for what it
verifies against `intuition-testnet-next`.
