# hasura-migrations-3.1.91

## Fix: Period Functions Output-Ratio for Intra-Day Redemptions

### Migration `1771526411000_fix_partial_redemption_pnl_split` (updated)

**Problem:** The period leaderboard functions (`get_pnl_leaderboard_period`, `get_vault_leaderboard_period`) used a share-ratio method (`shares_removed / (shares_at_end + shares_removed)`) to split realized vs unrealized PnL. However, `shares_delta_period` is a **net** daily figure — when a deposit and redemption occur on the same day, the net delta is positive, so `shares_removed = 0`. This caused:

- `realized_pnl` to equal the full redemption amount (zero cost deducted)
- `unrealized_pnl` to go negative to compensate
- `realized_pnl_pct = 0` because the denominator was also zero

**Fix:** Switched period functions to the same output-ratio method used by the all-time functions: `period_redemptions / (period_redemptions + equity_at_end)`. This uses asset values (which are tracked gross) instead of share deltas (which are tracked net), correctly handling intra-day deposit+redemption scenarios.

**No schema changes** — response format is unchanged.
