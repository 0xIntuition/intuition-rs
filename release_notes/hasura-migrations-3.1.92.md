# hasura-migrations-3.1.92

## Fix: Separate Migration for Period PnL Output-Ratio

### Migration `1771526412000_fix_period_pnl_output_ratio`

**Problem:** The output-ratio fix described in 3.1.91 was applied as an in-place edit to the already-deployed `1771526411000` migration file. Hasura tracks applied migrations by version number — editing a previously applied migration does not re-execute it. As a result, the deployed database still had the buggy share-ratio code despite the source file being updated.

**Fix:** Created a new migration (`1771526412000`) that re-deploys the two period functions (`get_pnl_leaderboard_period`, `get_vault_leaderboard_period`) with the output-ratio method. This ensures the fix is actually applied on environments that already ran `1771526411000`.

**No schema changes** — response format is unchanged. Functionally identical to what 3.1.91 described.
