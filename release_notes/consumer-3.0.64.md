# consumer:3.0.64

**Date:** 2026-03-09

## Fix: Position upsert no longer overwrites trigger-accumulated asset totals

### Problem

The `total_deposit_assets_after_total_fees` and `total_redeem_assets_for_receiver` fields on `position` were written by **both** DB triggers (which accumulate `+= assets` on each deposit/redemption INSERT) and the consumer's position upsert (which overwrites via `ON CONFLICT SET`). The consumer's upsert would overwrite the trigger-accumulated values with stale or single-event values.

**Result:** 191 positions had incorrect `total_deposit_assets_after_total_fees` and 66 had incorrect `total_redeem_assets_for_receiver` (testnet-next).

### Fix

Removed `total_deposit_assets_after_total_fees` and `total_redeem_assets_for_receiver` from the position upsert's `ON CONFLICT DO UPDATE SET` clause. DB triggers are now the sole owner of these fields. The initial INSERT still sets them correctly for new positions (first deposit sets the deposit amount; first redemption starts at 0).

### Changes

- `apps/models/src/position.rs`
  - Position upsert `ON CONFLICT` clause no longer overwrites `total_deposit_assets_after_total_fees` or `total_redeem_assets_for_receiver`

### Impact

- **Scope:** All position upserts (deposited + redeemed event processing).
- **Paired with:** `hasura-migrations-3.2.0` migration `1771526419000` which recalculates correct values for all affected positions.

---

## Fix: Definitive vault.position_count race condition fix

### Problem

The consumer processes up to 10 events concurrently. When a `Deposited` event creates a position before the `SharePriceChanged` event creates the vault, the `increment_vault_position_count()` trigger tries to UPDATE a vault row that doesn't exist yet — the increment is permanently lost. The existing BEFORE INSERT trigger on vault can't see the uncommitted position due to MVCC isolation. This caused recurring undercounts (stored=0, actual=1) that reappeared after every manual fix.

### Fix

**Vault pre-creation:** Before creating any position, the consumer now inserts a minimal vault stub (`INSERT ... ON CONFLICT DO NOTHING`). This ensures the vault row always exists when position-count triggers fire. When `SharePriceChanged` arrives later, its upsert fills in the real vault data without touching `position_count`.

### Changes

- `apps/models/src/vault.rs`
  - Added `Vault::ensure_exists(term_id, curve_id, schema, executor)` — inserts a zero-valued vault stub if none exists

- `apps/consumer/src/mode/decoded/deposited/event.rs`
  - `handle_positions()` — calls `Vault::ensure_exists()` before any position creation/update

### Impact

- **Scope:** Deposited event processing. Redeemed events already require the vault to exist.
- **Paired with:** `hasura-migrations-3.2.1` migration `1771526421000` which replaces the 4 incremental position-count triggers with a single full-recalculate trigger for additional resilience.
