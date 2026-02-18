# Chart-API Interval / Parameter Verification

Verification of chart-api endpoints that accept parameters like `1d`, `1w`, etc. (no code changes made).

---

## 1. Weekly bucket alignment (most likely cause of wrong `1w` data)

- **Rust** (`gap_filler.rs`, `pnl_time.rs`): Weekly buckets are **Monday 00:00 UTC** — `truncate_to_bucket(..., Weekly)` uses `timestamp.weekday().num_days_from_monday()` and truncates to that Monday.
- **DB** (`1729947331636_consolidated_materialized_views_and_extensions/up.sql`): Views like `share_price_change_stats_weekly` and `term_total_state_change_stats_weekly` use `time_bucket('1 week'::interval, bucket)` with **no explicit origin**.

TimescaleDB’s default origin for week-sized buckets can be **Monday (e.g. 2000-01-03)** in newer docs, but behavior can differ by version (e.g. epoch = Thursday, or Saturday in some reports). If the DB’s week boundaries are not Monday 00:00 UTC, then:

- Expected buckets (Mon → Mon) and DB buckets (e.g. Thu → Thu) don’t line up.
- `fill_gaps` maps DB rows to buckets by truncating with the same Monday-based logic, so DB points can be assigned to the “wrong” week and you get wrong or empty data for **interval=1w**.

**What to verify:** In your TimescaleDB version, run:

```sql
SELECT time_bucket('1 week'::interval, '2024-01-15 12:00:00+00'::timestamptz);
```

If the result is **2024-01-08 00:00** (Monday), it matches Rust. If it’s **2024-01-11 00:00** (Thursday) or **2024-01-13 00:00** (Saturday), that’s the mismatch.

---

## 2. Intervals and parsing

- **Chart endpoints** (`Interval`): `1h`, `1d`, `1w`, `1m` are matched explicitly in `types.rs`; no mixup between 1d and 1w.
- **PnL endpoints** (`PnlInterval`): `1m`, `5m`, `1h`, `1w`, `1d` — again exact match; `1w` is before `1d`, so no substring confusion.
- **Postgres**: Chart uses `raw_data_fetcher::interval_to_postgres` and materialized views use `"1 h"`, `"1 day"`, `"1 week"`, `"1 month"`. PnL uses `PnlInterval::as_postgres_interval()` (`"1 minute"`, `"5 minutes"`, `"1 hour"`, `"1 week"`, `"1 day"`). All consistent; no wrong-interval bug found.

---

## 3. Timestamp parsing (`validation::parse_timestamp`)

- Numeric: `|value| >= 1_000_000_000_000` → milliseconds, else seconds. So 10-digit = seconds, 13-digit = milliseconds; logic is correct.
- Edge case: 10–11 digit values are always treated as **seconds**. If a client ever sent milliseconds in that range, they’d be interpreted as seconds (wrong range). No bug in the API logic itself; only in client usage.

---

## 4. Range and cache

- `range_start`/`range_end` are aligned with `align_range` / `align_pnl_range` and used as **exclusive end** in both Rust and SQL (`updated_at < $range_end`). Consistent.
- Cache keys use `range_start.timestamp()` and `range_end.timestamp()` (seconds); same aligned range → same key. No bug found there.

---

## 5. Summary

| Check | Result |
|-------|--------|
| 1d / 1w / 1h / 1m mixup | None found; all distinct and consistent. |
| Wrong Postgres interval | None; chart and PnL use correct strings. |
| Timestamp parsing | Correct for seconds vs ms; only ambiguity is client sending 10–11 digit ms. |
| Weekly bucket alignment | **Possible mismatch**: Rust = Monday 00:00 UTC; DB = depends on TimescaleDB `time_bucket('1 week')` default origin. |

**Recommendation:** Confirm in your DB what `time_bucket('1 week', ...)` actually returns for a few timestamps (e.g. mid-week). If the result is not Monday 00:00 UTC, the wrong data for **1w** is explained; fixing it would mean either changing the view to use an explicit Monday origin or changing Rust to use the same week start as the DB.
