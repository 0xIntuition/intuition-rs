# hasura-migrations-3.1.94

## Fix: Continuous Aggregate Refresh Cannot Run Inside a Function

### Migration `1771526413000_add_gross_share_columns` (hotfix)

**Problem:** The migration wrapped `CALL refresh_continuous_aggregate(...)` inside a PL/pgSQL helper function with `SET statement_timeout = '60min'`. TimescaleDB's `refresh_continuous_aggregate` is a procedure that manages its own transactions and cannot execute from within a function — Postgres raises `"refresh_continuous_aggregate() cannot be executed from a function"` (error 25001).

**Fix:** Replaced the helper function with direct session-level statements:
```sql
SET statement_timeout = '60min';
CALL refresh_continuous_aggregate('position_change_hourly', NULL, NULL);
CALL refresh_continuous_aggregate('position_change_daily', NULL, NULL);
RESET statement_timeout;
```

Since Hasura runs migrations with `--no-transaction`, the session-level `SET` persists across statements, giving the refresh calls the extended timeout without needing a function wrapper.

Applied to both `up.sql` and `down.sql`.
