# consumer:3.0.62

**Date:** 2026-03-09

## Fix: Redemption events use receiver_id instead of sender_id

### Problem

During redemptions, the `sender` is the transaction originator (e.g. a smart wallet proxy), while the `receiver` is the actual share owner. The consumer was incorrectly using `sender` for position lookups and signal creation, which meant:

1. **Position shares** were looked up by `sender_id` instead of `receiver_id`, so when a proxy contract initiated the redemption on behalf of a user, the position update would fail to find the correct position.
2. **Signals** were attributed to the `sender` (proxy) instead of the `receiver` (actual share owner).

### Fix

- `handle_position_shares()` now takes `receiver_account` and looks up the position by receiver ID
- `create_signal()` now sets `account_id` to `self.receiver()` instead of `self.sender()`
- `event_handler.rs` passes `receiver_account` instead of `sender_account` to `handle_position_shares()`

### Changes

- `apps/consumer/src/mode/decoded/redeemed/event.rs`
  - `handle_position_shares()` — parameter renamed from `sender_account` to `receiver_account`, position lookup uses receiver ID
  - `create_signal()` — signal `account_id` uses `self.receiver()` for both atom and triple signals

- `apps/consumer/src/mode/decoded/redeemed/event_handler.rs`
  - `process_event()` — passes `receiver_account` to `handle_position_shares()`

### Impact

- **Scope:** Decoded consumer, redemption event processing only. Deposits are unaffected (already use receiver correctly).
- **Paired with:** `hasura-migrations-3.1.99` which fixes the same sender/receiver issue in database triggers.
