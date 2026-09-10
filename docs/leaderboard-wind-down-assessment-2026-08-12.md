# Leaderboard Wind-Down — Backend Effort Assessment

**Date:** 2026-08-12
**Context:** Product plans to wind down the portal leaderboard (`portal.intuition.systems/leaderboard`) — future epochs will no longer award IQ points for trading / holding a leaderboard position. Question asked: how big is the backend lift to stop it?

**Answer: small.** The minimal version is zero code (stop running one script). The clean version is roughly half a day of work plus a review cycle.

---

## How the system actually works (two separable pieces)

### 1. The leaderboard *display* (what the portal page shows)

- Portal queries Hasura-tracked SQL functions: `get_pnl_leaderboard`, `get_pnl_leaderboard_stats`, `get_account_pnl_rank`, `get_vault_leaderboard` (see `infrastructure/hasura/metadata/databases/intuition/functions/functions.yaml`).
- Rankings are computed **at query time** from timescale data. There is **no background accrual process** feeding the display.
- If the frontend removes the page, the queries simply stop. Backend needs zero changes for that; optionally the functions can be untracked from Hasura metadata to remove them from the GraphQL schema.

### 2. The IQ points *awarding* (Season 2 settlement)

Defined in migration `1771526400000_add_season2_iq_points_mvp`:

- `season2_epoch` — pre-seeded calendar, epochs **8..30**, 14 days each, 2026-02-24 → 2027-01-12.
- `scripts/season2_capture_trust_price.sh` — snapshots TRUST/USD from CoinGecko into `season2_trust_price_snapshot` every 12h (00:00 / 12:00 UTC).
- `scripts/season2_settle_epoch.sh --epoch N [--finalize]` — run **after** an epoch ends (+12h grace). Calls `settle_season2_epoch(N)`, which writes `season2_iq_ledger` entries of three types:
  - `fee` — every account's protocol fees for the epoch converted to IQ (`protocol_fee_accrued` × avg TRUST/USD × 2000).
  - `leaderboard_pnl` — top-25 payout curve on the nominal PnL board (`season2_leaderboard_payout` ranks 1–25).
  - `leaderboard_roi` — top-25 payout curve on the ROI (`pnl_pct`) board.
  Then optionally `finalize_season2_epoch(N)` freezes it.
- **IQ points only ever come into existence when settlement runs.** There is no continuous accrual. Hasura has no cron triggers (`cron_triggers.yaml` is `[]`) and this repo has no k8s CronJob manifests — the two scripts are run manually or scheduled outside this repo (likely gcp-deployment; see open items).
- `chart-api` serves `GET /api/v1/accounts/{id}/season2/iq` by reading the ledger — pure historical read; keeps working unchanged after wind-down.

## What "stopping" requires

### Minimal (zero code)

Stop running `season2_settle_epoch.sh` for epochs after the last rewarded one. No IQ is ever created for those epochs. Historical ledger, epoch prices, and the chart-api endpoint are untouched.

### Clean (recommended, ~half a day + review)

1. **Guard the settlement function** — one small Hasura migration adding a cutoff check to `settle_season2_epoch` (refuse epochs > last rewarded epoch), and/or delete the unneeded future rows from `season2_epoch`. Prevents an accidental manual settlement months later.
2. **Stop the 12h TRUST price capture** wherever it is scheduled. Harmless if left running (tiny table, one CoinGecko call), but pointless.
3. **Optionally untrack the leaderboard read functions** from Hasura metadata once the frontend removes the page — the period-leaderboard queries are the expensive part of this feature, and untracking guarantees nobody keeps hitting them. If the frontend keeps a read-only historical page instead, leave them tracked.

### Not involved at all

- No indexer/consumer changes, no contract changes, no data migrations, no backfills.
- `protocol_fee_accrued` keeps accruing from chain events regardless — harmless; it just never gets converted to IQ once settlement stops.

## Decision points for planning

1. **Does fee IQ stop too?** Settlement awards three entry types in one function. "No IQ for leaderboard positions" kills `leaderboard_pnl`/`leaderboard_roi`; if `fee` IQ should *continue*, that's a small migration to `settle_season2_epoch` (skip the two leaderboard inserts) rather than simply not running the script — still small, but different work.
2. **Does the display stay?** Keeping a read-only leaderboard page (no rewards) costs the backend nothing. Removing it is a frontend change; backend just untracks functions afterwards.
3. **Cutoff epoch** — need the last epoch that still pays out, to write the guard.

## Scheduling — verified in cluster (2026-08-12)

Checked `gke_be-cluster_us-west2_debug-cluster`:

- **Price capture IS scheduled** as k8s CronJobs (every 12h, `0,12` UTC with per-env minute offsets), active in three namespaces:
  - `intuition-mainnet-next` — `intuition-mainnet-next-season2-price-capture` (7 0,12 * * *)
  - `intuition-mainnet-nested-triples` — `mainnet-nested-season2-price-capture` (9 0,12 * * *)
  - `intuition-testnet-next` — `intuition-testnet-next-season2-price-capture` (5 0,12 * * *)
  - (`intuition-testnet-a` has one too, already suspended)
  Each runs a `postgres:17-alpine` pod executing a ConfigMap-mounted `capture.sh` with `DATABASE_URL` from the `season2-db-credentials` secret. The manifests live in the gcp-deployment repo — wind-down step is to suspend/delete them there.
- **Settlement is NOT scheduled anywhere** — no CronJob, Job, or Deployment for `settle_season2_epoch` exists in the cluster. Epoch settlement is a manual run of `scripts/season2_settle_epoch.sh`. This confirms the minimal wind-down is genuinely zero code: just stop running that script.

---

## Update 2026-09-10

GWTH-4354 implemented the "clean" wind-down (decision point 1 above resolved
as: **fee IQ continues, leaderboard IQ freezes at Epoch 20**). Two of this
assessment's "Clean" recommendations turned out to be wrong once we checked
live data and the actual read path, and were corrected before implementation:

- **"Stop the 12h TRUST price capture" (item 2) — wrong, not done.** Fee IQ's
  formula divides by the epoch's average TRUST/USD price, computed from
  `season2_trust_price_snapshot`. Snapshots are point-in-time and cannot be
  backfilled — an epoch with zero snapshots makes `settle_season2_epoch`
  raise and makes fee IQ for that epoch permanently unsettleable. Since this
  wind-down explicitly keeps fee IQ running past Epoch 20, the CronJobs stay
  running in all three namespaces.
- **"Optionally untrack the leaderboard read functions" (item 3) — wrong,
  not done.** The frontend is keeping a read-only archive of Epochs 1-20,
  and that archive queries `get_pnl_leaderboard_period` live via the
  `get_pnl_leaderboard_period_min_threshold` Hasura Action / chart-api route
  on every page load — untracking anything in that chain breaks the archive
  outright, not just future epochs.

The cutoff epoch from decision point 3 is **Epoch 20**.

Full rationale, live-DB evidence, and the corrected read-path chain (portal
→ Hasura Action → chart-api → Postgres function, not a directly-tracked
Postgres function as implied above) are in
`docs/leaderboard-wind-down-epoch-20.md`. That doc also covers the four
all-time leaderboard functions (`get_pnl_leaderboard`,
`get_pnl_leaderboard_stats`, `get_account_pnl_rank`,
`get_vault_leaderboard`) referenced in "How the system actually works" above
— confirmed still unused by the portal, and deliberately left tracked rather
than untracked, pending their own ticket if they're ever removed.
