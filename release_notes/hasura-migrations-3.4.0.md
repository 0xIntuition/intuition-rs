# hasura-migrations-3.4.0

**Date:** 2026-09-10

**Image:** `ghcr.io/0xintuition/hasura-migrations:3.4.0` (also tagged `latest`)
**Previous:** `3.3.99` (built 2026-07-13)

Two independent changes ship in this image. The second has been sitting
unreleased for nearly two months, because `3.3.99` was built one day before it
merged.

---

## 1. Season 2 leaderboard wind-down (GWTH-4354)

Epoch 20 is the final leaderboard epoch. From Epoch 21 onward there is no
leaderboard IQ. **Fee IQ is unaffected and keeps accruing for every epoch.**
These are separate entry types produced by one function, so this is a selective
guard, not an off-switch.

### New migration `1771526444000_leaderboard_wind_down_epoch_20`

- Adds `season2_last_leaderboard_epoch()` — an `IMMUTABLE` cutoff constant
  returning 20, so the cutoff is greppable rather than a literal buried in the
  settlement body.
- Re-declares `settle_season2_epoch` with an identical signature and
  `RETURNS TABLE` column list. The body is byte-identical to the original
  apart from wrapping the `leaderboard_pnl` and `leaderboard_roi` inserts in
  `IF p_epoch <= season2_last_leaderboard_epoch()`.
- Reverses any leaderboard IQ already settled past the cutoff:
  `DELETE FROM season2_iq_ledger WHERE epoch > season2_last_leaderboard_epoch()
  AND entry_type IN ('leaderboard_pnl','leaderboard_roi')`.

`season2_epoch` is deliberately **not** trimmed — epochs 21-30 must keep
existing so fee IQ can still be settled for them.

### Two pre-existing bugs fixed in the same migration

Validating the cutoff against a real database surfaced two crash bugs present
since `1771526400000_add_season2_iq_points_mvp`. Both were reproduced against
the deployed function, not inferred:

| Error | Cause | Fix |
| --- | --- | --- |
| `42702 column reference "epoch" is ambiguous` | every `ON CONFLICT (epoch, ...)` collides with the function's own `RETURNS TABLE` OUT column named `epoch` | `#variable_conflict use_column` |
| `42P07 relation "_tmp_active_accounts" already exists` | `get_pnl_leaderboard_period`'s temp tables are `ON COMMIT DROP` and survive between calls in one transaction; settlement calls it twice | see migration `1771526445000` below |

Bug 1 meant **`settle_season2_epoch` could never settle any epoch at all.**
That is why `season2_iq_ledger` is empty in every environment — not because
nobody ran settlement, but because settlement could not succeed. Fixing it was
not optional scope: an unfixed function cannot award fee IQ for Epoch 21+
either, which is the one thing the wind-down needs to keep working.

### New migration `1771526445000_fix_pnl_leaderboard_period_temp_tables`

Fixes the `42P07` defect in the callee rather than working around it in the
caller. `get_pnl_leaderboard_period` carried an undocumented "call me at most
once per transaction" contract; it now drops its seven temp tables on entry,
so every call is self-cleaning.

Split into its own migration deliberately: Postgres cannot prepend to a
function body, so `CREATE OR REPLACE` means restating all 637 lines of a
function that has been redefined 28 times and is actively maintained. Keeping
it separate means it can be reviewed and reverted on its own terms.

`1771526444000` retains its own caller-side `DROP` blocks. They are redundant
once `1771526445000` applies, but they are idempotent no-ops and keeping them
means `1771526445000` can be reverted without breaking settlement.

**This touches the live read path** — `get_pnl_leaderboard_period` backs the
Hasura Action `get_pnl_leaderboard_period_min_threshold` via chart-api, which
serves the portal's read-only Epoch 1-20 leaderboard archive. Verified in a
rolled-back transaction on both portal databases that three consecutive calls
in one transaction succeed and same-argument calls agree on row count
(testnet-next 0/0/0, mainnet-nested-triples 25/25/25).

### Verified on real mainnet data

`up.sql` applied in a transaction against `intuition-mainnet-nested-triples`,
both epochs settled with real data, then rolled back:

| Epoch | fee entries | fee IQ | leaderboard entries | pnl IQ | roi IQ |
| --- | --- | --- | --- | --- | --- |
| **20** (final) | 3,977 | 828,289 | **25 pnl + 25 roi** | 3,000,000 | 3,000,000 |
| **21** (past cutoff) | 16,297 | 2,996,047 | **0** | 0 | 0 |

Epoch 21 awards fee IQ and only fee IQ; Epoch 20 still pays the full 25-rank
payout on both boards. Post-cutoff settlement also gets much faster —
`settle_season2_epoch(21)` runs in **479 ms** versus **~27 s** for epoch 20,
since both `get_pnl_leaderboard_period` calls are skipped.

---

## 2. `uploadImage` Hasura action sent as JSON (#226, ENG-13513)

Merged 2026-07-14, one day after `3.3.99` was built, so it has never shipped.
Sends the `uploadImage` action request as JSON with the correct content type,
and rewrites `infrastructure/hasura/Dockerfile` to install the Hasura CLI via
`ADD` with `TARGETARCH` instead of `apt-get`, which removes the apt layer from
the image build.

---

## Deployment notes

- **Both new migrations are fully idempotent** — only
  `CREATE OR REPLACE FUNCTION`, `COMMENT ON FUNCTION`, and one scoped
  `DELETE`. No `CREATE TABLE`, no `INSERT`, no `ALTER`. This matters because
  these environments have no persistent Hasura migration state, so migrations
  re-apply from scratch on every deploy (see `hasura-migrations-3.3.5`).
- The reversal `DELETE` re-running on every deploy is intentional: it
  continuously enforces "no leaderboard IQ past Epoch 20" rather than being a
  one-shot cleanup.
- **No long-running statements.** Both migrations are DDL plus a `DELETE` on a
  table that is currently empty in every environment, so neither is exposed to
  the 1-minute `statement_timeout` that has previously failed migrations on
  `mainnet-next`.
- Environments bumped: `intuition-testnet-next`, `intuition-mainnet-next`,
  `intuition-mainnet-nested-triples`. `linea-v2` stays on `2.0.12`.

### Verify after deploy

```sql
-- cutoff constant present
SELECT season2_last_leaderboard_epoch();                  -- expect 20

-- callee fix present: two calls in one transaction must both succeed
BEGIN;
SELECT count(*) FROM get_pnl_leaderboard_period(
  TIMESTAMPTZ '2026-08-11 00:00:00+00', TIMESTAMPTZ '2026-08-25 00:00:00+00',
  25, 0, 'total_pnl', 'DESC', TRUE, 1, 0, NULL);
SELECT count(*) FROM get_pnl_leaderboard_period(
  TIMESTAMPTZ '2026-08-11 00:00:00+00', TIMESTAMPTZ '2026-08-25 00:00:00+00',
  25, 0, 'pnl_pct', 'DESC', TRUE, 1, 0, NULL);
ROLLBACK;

-- invariant: no leaderboard IQ past the cutoff
SELECT count(*) FROM season2_iq_ledger
WHERE epoch > 20 AND entry_type IN ('leaderboard_pnl','leaderboard_roi');  -- expect 0
```

Also confirm the portal's `/leaderboard` archive still returns rows for a
historical epoch, since `1771526445000` replaces a function on that read path.

### Repro scripts

- `scripts/season2_verify_leaderboard_cutoff.sql` — full cutoff harness,
  wrapped in `BEGIN;`/`ROLLBACK;`
- `scripts/season2_verify_leaderboard_period_temp_tables.sql` — callee-fix
  regression test
- `scripts/season2_check_verify_script_sync.sh` — drift guard for the embedded
  migration copies in both scripts

### Not done, deliberately

- **Price-capture CronJobs left running.** The wind-down spec called for
  suspending them, but fee IQ is `(amount_trust x average_trust_usd) x 2000`
  and settlement hard-fails without snapshots in the epoch window. Suspending
  would make fee IQ for Epoch 23+ permanently unsettleable, contradicting the
  requirement that fee IQ continues.
- **Hasura leaderboard functions left tracked.** The archive reads through
  them; the four all-time variants in `functions.yaml` are portal-unused but
  are public GraphQL fields, so removing them is a breaking API change
  deserving its own ticket.
