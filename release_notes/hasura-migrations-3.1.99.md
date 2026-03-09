# hasura-migrations-3.1.99

**Date:** 2026-03-09

## Fix: Redemption triggers use receiver_id instead of sender_id

### Migration `1771526418000_fix_redemption_use_receiver_id`

**Problem:** Two redemption triggers were using `sender_id` (the transaction originator / proxy) instead of `receiver_id` (the actual share owner) when updating positions and recording position changes. This caused incorrect attribution when redemptions were initiated by a proxy contract on behalf of a user.

**Fix:** Updated both trigger functions to use `NEW.receiver_id`:

1. **`update_position_redeem_assets()`** — now matches `position.account_id = NEW.receiver_id` so `total_redeem_assets_for_receiver` is updated on the correct position.

2. **`insert_position_change_from_redemption()`** — now inserts `NEW.receiver_id` as the `account_id` in `position_change` rows.

3. **Historical data fix** — updates any existing `position_change` rows where `sender_id != receiver_id` to use the correct `receiver_id`. Currently 0 rows affected, but included for correctness across environments.

**Paired with:** `consumer:3.0.62` which fixes the same sender/receiver issue in the Rust consumer.
