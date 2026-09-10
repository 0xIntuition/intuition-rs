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

1. **A bare `epoch` reference is ambiguous — at FIVE sites, not four.**
   `settle_season2_epoch`'s `RETURNS TABLE` declares its first OUT column as
   `epoch`, which PL/pgSQL treats as an implicit variable in scope for the
   whole function body. Every bare `epoch` reference in a DML statement
   below then has a genuine ambiguity — Postgres can't tell whether `epoch`
   means the OUT variable or the table column — and raises `column
   reference "epoch" is ambiguous`. There are five such sites: the
   `ON CONFLICT (epoch)` clause in the `season2_epoch_price` upsert, the
   three `ON CONFLICT (epoch, entry_type, source_id)` clauses in the
   `season2_iq_ledger` inserts (fee, pnl, roi) — and a fifth that is easy to
   miss because it isn't an `ON CONFLICT` clause at all:
   `DELETE FROM season2_iq_ledger WHERE epoch = p_epoch` inside the
   `IF p_force THEN` branch. That fifth site is exactly as ambiguous as the
   other four, but it never surfaced during development (an earlier draft of
   this writeup also undercounted it as four) because `p_force` has never
   been exercised — every verification and live call so far used
   `p_force = FALSE`. Fix: `#variable_conflict use_column` at the top of the
   function body, which tells PL/pgSQL to prefer the column interpretation
   everywhere in the function. Safe here because the function never
   intentionally references its OUT columns by their bare names (it
   consistently uses `v_`-prefixed locals and `p_`-prefixed parameters
   instead).

   **Caution for future edits:** `#variable_conflict use_column` disables
   Postgres's `42702` ambiguity detection for every bare identifier in the
   *entire* function, not just these five sites — it's a function-wide
   pragma, not a per-statement one. If a future edit adds a bare reference
   that collides with a column name but actually intends the *variable*, it
   will silently resolve to the column instead of raising an error, with no
   warning at `CREATE FUNCTION` time or at call time. If this function is
   revisited, prefer `ON CONFLICT ON CONSTRAINT <constraint_name>` (e.g.
   `season2_iq_ledger_epoch_entry_source_unique`, `season2_epoch_price_pkey`)
   over broadening reliance on this pragma — it resolves each site's
   ambiguity individually, by naming the constraint instead of the column
   list, without touching the function's global identifier resolution rule.
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

A third thing surfaced while building the verification script, and it is
worth stating precisely because the first reading of it was wrong.

On `intuition-testnet-next`, `protocol_fee_accrued.epoch` spans 1 to 322
(269 distinct values) and clearly does *not* line up with `season2_epoch`'s
8-30 calendar. That initially looked like a latent correctness bug in the
fee IQ formula.

It is not. Checked against the environments that actually matter
(2026-09-10):

| Database | `protocol_fee_accrued.epoch` range | distinct | rows |
| --- | --- | --- | --- |
| `intuition-mainnet-nested-triples` | 0 - 22 | 23 | 363,172 |
| `intuition-mainnet-next` | 0 - 22 | 23 | 363,180 |
| `intuition-testnet-next` | 1 - 322 | 269 | 69,442 |

On mainnet the counter runs 0-22, which matches the on-chain
`TrustBonding.currentEpoch()` value of 22 read the same day. `season2_epoch`
covers epochs 8-30 of that same on-chain sequence, so the fee INSERT's
`WHERE pfa.epoch = p_epoch::NUMERIC` matches exactly the rows it should.
**Fee IQ settlement is correct in production.**

The 1-322 spread is a testnet-only artifact: the testnet chain cycles
epochs far faster, so its on-chain epoch counter has run far past the
Season 2 calendar that was designed around mainnet's 14-day cadence. The
only consequence is for testing: synthetic fixture epochs on testnet must
avoid colliding with real `protocol_fee_accrued` rows, which is why
`scripts/season2_verify_leaderboard_cutoff.sql` uses `-7` and `90000041`
rather than small positive integers like `7` and `41` (an earlier version
used those, and the byte-identical-fee assertion failed correctly because
real rows at those epochs leaked into the totals).

No follow-up needed. Recorded here so the testnet spread isn't mistaken for
a production bug the next time someone looks.

## Validation

`scripts/season2_verify_leaderboard_cutoff.sql` — a self-contained,
`BEGIN; ... ROLLBACK;`-wrapped differential test — proves the guard is
selective (blocks leaderboard IQ, not fee IQ) rather than the function being
broken. See the script header for the fixture design and
`docs/leaderboard-wind-down-epoch-20.md`'s sibling migration for what it
verifies against `intuition-testnet-next`.

## End-to-end verification on real mainnet data

The fixture-based script above proves the guard in isolation. This is the
production-shaped proof: `up.sql` applied inside a transaction against
`intuition-mainnet-nested-triples`, both epochs settled with **real** data,
then rolled back (nothing persisted).

| Epoch | fee entries | fee IQ | leaderboard entries | pnl IQ | roi IQ |
| --- | --- | --- | --- | --- | --- |
| **20** (<= cutoff) | 3,977 | 828,289 | **25 pnl + 25 roi** | 3,000,000 | 3,000,000 |
| **21** (> cutoff) | 16,297 | 2,996,047 | **0** | 0 | 0 |

Read together these are the two halves of the requirement:

- Epoch 21 awards **fee IQ and only fee IQ** — requirement 2 holds past the
  cutoff, and requirement 1 holds at the same time.
- Epoch 20 still awards **the full 25-rank payout on both boards** — the
  guard is selective, not a blanket off-switch. An implementation that
  simply broke settlement would fail this row.

Both epochs settling at all is also the proof that the two pre-existing bug
fixes (changes 4 and 5) work: before them, `settle_season2_epoch` raised
`42702 column reference "epoch" is ambiguous` on any epoch, and
`42P07 relation "_tmp_active_accounts" already exists` on the second
`get_pnl_leaderboard_period` call within one settlement. Both were confirmed
against the deployed function before the fix.

Total runtime for the epoch-20 case, including both leaderboard
computations: ~27s. For comparison, `settle_season2_epoch(21)` — past the
cutoff, so both `get_pnl_leaderboard_period` calls are skipped — took a
measured **479 ms** on the same environment
(`intuition-mainnet-nested-triples`): roughly a **56x** reduction, entirely
attributable to skipping the two leaderboard computations.

### Reversibility

`up.sql` then `down.sql`, applied in one transaction against
`intuition-testnet-next` and rolled back:

| Stage | `season2_last_leaderboard_epoch` | `settle_season2_epoch` |
| --- | --- | --- |
| after `up.sql` | 1 (created) | 1 |
| after `down.sql` | 0 (dropped) | 1 (restored) |

After the down migration, `settle_season2_epoch(21)` raises
`42702 column reference "epoch" is ambiguous` again — which is the intended
outcome. `down.sql` restores the pre-migration function *verbatim*, including
the two pre-existing bugs, because a rollback that quietly kept the fixes
would not be a true revert. The bugs are the pre-migration state.
