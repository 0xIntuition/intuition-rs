# hasura-migrations-3.2.0

**Date:** 2026-03-09

### Migration `1771526419000_fix_position_asset_totals`

**Problem:** The consumer's position upsert `ON CONFLICT` clause was overwriting `total_deposit_assets_after_total_fees` and `total_redeem_assets_for_receiver` with stale values, conflicting with the DB triggers that accumulate these fields. Result: 191 positions with incorrect deposit totals and 66 with incorrect redemption totals (testnet-next).

**Fix:** Recalculates both fields from the source `deposit` and `redemption` tables. Also runs `fix_wrong_position_counts()` to correct 9 vault position_count mismatches.

**Paired with:** `consumer:3.0.64` which removes these fields from the position upsert's `ON CONFLICT SET` clause so triggers are the sole owner going forward.

