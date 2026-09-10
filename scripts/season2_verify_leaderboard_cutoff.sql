-- GWTH-4354: differential verification for the leaderboard wind-down at
-- Epoch 20 (infrastructure/hasura/migrations/intuition/1771526444000_leaderboard_wind_down_epoch_20).
--
-- Run with psql, e.g.:
--   kubectl exec -i -n intuition-testnet-next intuition-testnet-next-timescale-db-0 -- \
--     bash -c 'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -f -' < scripts/season2_verify_leaderboard_cutoff.sql
--
-- Everything runs inside BEGIN; ... ROLLBACK; so nothing persists, regardless
-- of pass/fail. Assertions use plpgsql ASSERT, which raises (and, combined
-- with -v ON_ERROR_STOP=1, aborts the script with a nonzero exit) on
-- failure — a broken or no-op implementation of the guard cannot pass this
-- script silently, PROVIDED plpgsql.check_asserts is on (see Step 0 below,
-- which pins it: ASSERT is a silent no-op when that GUC is off).
--
-- DRIFT GUARD: this script embeds up.sql and down.sql verbatim (see the next
-- comment block for why). Nothing at parse time enforces those copies stay
-- in sync with the real migration files. Run
-- `scripts/season2_check_verify_script_sync.sh` after editing either this
-- script or the migration to confirm the embedded copies still match
-- byte-for-byte — it is a standalone script, not wired into CI (this repo
-- has no DB-backed CI yet).
--
-- WHY THE MIGRATION IS EMBEDDED VERBATIM BELOW (not `\i`-included):
-- psql is invoked here with `-f -` reading the script over `kubectl exec -i`
-- stdin, so a `\i`/`\ir` meta-command would try to open a path on the POD's
-- filesystem, where this repo does not exist. Copy-pasting the migration's
-- up.sql (Step B below) and down.sql (Step H below) content into this script
-- byte-for-byte is the reliable way to make this script runnable via that
-- exact invocation while staying self-contained. If either migration file
-- changes, both embedded blocks must be re-synced — see the drift guard
-- above.
--
-- FIXTURE DESIGN (why real data for some assertions, synthetic for others):
-- `get_pnl_leaderboard_period` depends on deep, interdependent machinery
-- (hourly position snapshots, continuous aggregates, bonding-curve equity
-- valuation) that is impractical to seed realistically from scratch. So:
--   - The "an epoch <=20 inserts leaderboard entries" assertion (Step E)
--     uses REAL intuition-testnet-next trading data, via a synthetic
--     season2_epoch fixture (epoch 6) whose window is deliberately wide
--     (2020-01-01 .. now()) to be virtually guaranteed to contain real PnL
--     activity — empirically confirmed against intuition-testnet-next on
--     2026-09-10 (25 rows for both the total_pnl and pnl_pct sorts over
--     that window; 0 rows for the real epoch 20 window specifically, which
--     is why epoch 20 itself isn't reused directly here).
--   - The "fee IQ totals are byte-identical whether the epoch is <=20 or
--     >20" assertion (Step E) instead uses two fully synthetic, mutually
--     isolated season2_epoch fixtures (epoch -7 <=20, epoch 90000041 >20)
--     that share an IDENTICAL synthetic time window (2099-01-01 ..
--     2099-01-02, far outside any real epoch, so it cannot collide with
--     real snapshot or position data) and an identical seeded
--     protocol_fee_accrued amount. The epoch numbers themselves are
--     deliberately far from season2_epoch's real 8..30 range AND from
--     protocol_fee_accrued's own real epoch values (see the fixture
--     section below for why that second collision is real and was hit
--     during development). Because both fixtures see the same avg
--     TRUST/USD price and the same fee amount, and the fee code path is
--     untouched by the guard, their computed fee_iq_total must be exactly
--     equal by construction — this is a real equality check, not a
--     coincidence of live data.
--   - The "pre-seeded leaderboard_pnl row for epoch 21 is deleted, a
--     pre-seeded fee row for epoch 21 survives" assertion (Step C) seeds
--     directly into season2_iq_ledger against the REAL epoch 21 (bypassing
--     settle_season2_epoch entirely) and checks the migration's own
--     cleanup DELETE, run as part of applying the embedded migration below.
--   - The BOUNDARY test (Step F) — the single most important assertion in
--     this script — cannot reuse epoch 6, -7 or 90000041 as-is: none of
--     them sit exactly on the real cutoff (20), so an off-by-one in the
--     guard (`<` instead of `<=`) would pass every assertion above without
--     detection. Step F temporarily overrides season2_last_leaderboard_epoch()
--     to a synthetic value (-100) and seeds epochs exactly AT that value
--     and exactly one past it, reusing epoch 6's real-data window (per its
--     own header comment: seeding fresh position/PnL data for two more
--     synthetic epochs is impractical, so the boundary epochs inherit real
--     qualifying data instead) so both boundary epochs get real leaderboard
--     rows to test the guard against, before restoring the real constant.
--   - The p_force = TRUE test (Step G) reuses epoch 90000041 (already
--     settled once with p_force = FALSE in Step E) to prove force
--     re-settlement past the cutoff cannot resurrect leaderboard IQ.
--   - The down.sql smoke test (Step H) embeds down.sql verbatim and proves
--     the rollback actually restores the pre-fix ambiguous-column bug. It
--     is deliberately the LAST phase: down.sql leaves settle_season2_epoch
--     unable to settle ANY epoch for the rest of the transaction.
--
-- Fixture epoch numbers 6, -7, 90000041, -100 and -99 are all chosen to be
-- outside the real season2_epoch calendar (8..30 today), so none of them
-- can collide with real rows in season2_epoch, and their season2_iq_ledger
-- / season2_epoch_price rows are equally isolated (both FK on epoch). -7,
-- 90000041, -100 and -99 are additionally chosen to avoid
-- protocol_fee_accrued's own real epoch values — see the fixture section
-- below.

\pset pager off
\timing off
\set ON_ERROR_STOP on

BEGIN;

-- ========================================
-- Step 0. PIN THE ASSERT GUC (must run first)
-- ========================================
--
-- plpgsql ASSERT is a COMPLETE NO-OP when plpgsql.check_asserts is off —
-- the condition is not even evaluated, so every `RAISE NOTICE 'PASS: ...'`
-- in this script (which sits unconditionally right after its ASSERT) would
-- still print, and the script would still reach 'ALL ASSERTIONS PASSED' and
-- exit 0, having verified NOTHING. SET LOCAL scopes this to the current
-- transaction only, so it never leaks past the ROLLBACK at the end of this
-- script. Verified: this works as the first statement of a fresh session,
-- and the default is already `on` — but pinning it here means this script's
-- pass/fail no longer depends on an ambient session setting nobody is
-- watching.
SET LOCAL plpgsql.check_asserts = on;

-- ========================================
-- Step A. FIXTURE: account + pre-existing epoch-21 ledger rows
--    (seeded BEFORE the migration so its DELETE has something to act on)
-- ========================================

INSERT INTO account (id, label, type)
VALUES ('fixture-gwth4354-account', 'GWTH-4354 verification fixture', 'Default');

-- Pre-existing leaderboard_pnl row for the REAL epoch 21 (>20). The
-- migration's cleanup DELETE must remove this.
INSERT INTO season2_iq_ledger (epoch, account_id, entry_type, source_id, iq_points, metadata)
VALUES (
  21,
  'fixture-gwth4354-account',
  'leaderboard_pnl',
  'fixture-gwth4354-pnl-epoch21-preexisting',
  1000,
  '{"fixture": "gwth4354-delete-survival-test"}'::jsonb
);

-- Pre-existing fee row for the same REAL epoch 21. The migration's cleanup
-- DELETE only targets entry_type IN ('leaderboard_pnl','leaderboard_roi'),
-- so this must survive.
INSERT INTO season2_iq_ledger (epoch, account_id, entry_type, source_id, iq_points, metadata)
VALUES (
  21,
  'fixture-gwth4354-account',
  'fee',
  'fixture-gwth4354-fee-epoch21-preexisting',
  500,
  '{"fixture": "gwth4354-delete-survival-test"}'::jsonb
);

-- ========================================
-- Step B. APPLY THE MIGRATION (embedded verbatim — see header comment above)
--    Source: infrastructure/hasura/migrations/intuition/1771526444000_leaderboard_wind_down_epoch_20/up.sql
--    Keep this block byte-identical to that file. Checked by
--    scripts/season2_check_verify_script_sync.sh.
-- ========================================

-- BEGIN EMBEDDED up.sql (verbatim - keep byte-identical, see scripts/season2_check_verify_script_sync.sh)
-- GWTH-4354: Leaderboard wind-down — freeze leaderboard IQ at Epoch 20.
--
-- Season 2 leaderboard rewards ("Support the Trenches") are winding down.
-- Epoch 20 (2026-08-11 -> 2026-08-25) is the LAST epoch that pays out
-- leaderboard IQ (the `leaderboard_pnl` and `leaderboard_roi` entry types).
-- From Epoch 21 onward, `settle_season2_epoch` must skip those two INSERT
-- blocks entirely.
--
-- Fee IQ (`fee` entry type, protocol fees converted to IQ) is NOT part of
-- this wind-down and MUST keep accruing unchanged for every epoch,
-- including 21+. See docs/leaderboard-wind-down-epoch-20.md for the full
-- rationale and the live-DB evidence this was based on.
--
-- Three changes, plus two unrelated bug fixes discovered while validating them:
--   1. A greppable, auditable cutoff constant (season2_last_leaderboard_epoch()),
--      instead of a bare literal buried in the settlement function body.
--   2. CREATE OR REPLACE settle_season2_epoch(...) with the IDENTICAL
--      signature and RETURNS TABLE column list as the original definition
--      in migration 1771526400000_add_season2_iq_points_mvp (Postgres
--      cannot change a function's return type via CREATE OR REPLACE). The
--      body is copied verbatim except the `leaderboard_pnl` and
--      `leaderboard_roi` INSERT blocks are now wrapped in
--      `IF p_epoch <= season2_last_leaderboard_epoch() THEN ... END IF;`.
--      Everything else (epoch validation, snapshot averaging, the `fee`
--      INSERT block, `settled_at` update, final recompute-and-return) is
--      untouched, so settling epoch 21+ still succeeds and still awards
--      fee IQ.
--   3. An idempotent cleanup DELETE that reverses any leaderboard IQ that
--      may already have been settled past the cutoff. In practice this is
--      a no-op today: `season2_iq_ledger` is empty in every environment
--      (settlement has never been run — verified 2026-09-10), so there is
--      nothing to delete. It is kept so this migration is safe to apply
--      even after a manual settlement run, and so it remains idempotent
--      if applied more than once.
--   4. PRE-EXISTING BUG FIX (unrelated to the wind-down, required for
--      settle_season2_epoch to run at all): added `#variable_conflict
--      use_column` to the top of the function body. Without it, every
--      `ON CONFLICT (epoch, ...)` clause in the function raises "column
--      reference \"epoch\" is ambiguous", because the function's own
--      RETURNS TABLE OUT column is also named `epoch`. This bug has
--      existed since the function was first created in migration
--      1771526400000_add_season2_iq_points_mvp and was never caught
--      because settle_season2_epoch has never been successfully run
--      anywhere (verified 2026-09-10: season2_iq_ledger is empty and
--      settled_at is NULL for every epoch in every environment). It
--      surfaced when this migration's own verification script
--      (scripts/season2_verify_leaderboard_cutoff.sql) first called the
--      function for real. See docs/leaderboard-wind-down-epoch-20.md for
--      the full writeup. Fixed here rather than deferred, since an
--      unfixed settle_season2_epoch cannot award fee IQ for epoch 21+
--      either — the exact thing requirement 2 needs to keep working.
--   5. SECOND PRE-EXISTING BUG FIX (unrelated to the wind-down): added
--      `DROP TABLE IF EXISTS` for all seven of get_pnl_leaderboard_period's
--      `ON COMMIT DROP` temp tables, in TWO places — right before the pnl
--      INSERT block, and again between the pnl and roi INSERT blocks.
--      Those tables only drop at transaction commit, never between calls
--      within the same transaction/session, and this function calls
--      get_pnl_leaderboard_period() twice (pnl, then roi) every time it
--      settles an epoch <= the cutoff — so without both DROPs, either the
--      roi call within one execution, or the pnl call of a LATER
--      settle_season2_epoch invocation in the same session, fails with
--      'relation "..." already exists'. Also pre-existing, also never
--      triggered because settlement has never run. See both DROP
--      statements' own comments and docs/leaderboard-wind-down-epoch-20.md
--      for the full writeup.
--
-- season2_epoch is NOT touched: epochs 21-30 must keep existing so fee IQ
-- can still be settled for them.

-- ========================================
-- 1. CUTOFF CONSTANT
-- ========================================

-- GWTH-4354: last Season 2 epoch that pays out leaderboard IQ
-- (leaderboard_pnl / leaderboard_roi). Fee IQ is unaffected and keeps
-- accruing for every epoch after this one.
CREATE OR REPLACE FUNCTION season2_last_leaderboard_epoch()
RETURNS INTEGER AS $$
  SELECT 20;
$$ LANGUAGE sql IMMUTABLE;

COMMENT ON FUNCTION season2_last_leaderboard_epoch() IS
  'GWTH-4354: last Season 2 epoch that pays out leaderboard_pnl/leaderboard_roi IQ. Fee IQ is unaffected.';

-- ========================================
-- 2. SETTLEMENT FUNCTION — leaderboard cutoff guard
-- ========================================

CREATE OR REPLACE FUNCTION settle_season2_epoch(
  p_epoch INTEGER,
  p_force BOOLEAN DEFAULT FALSE
)
RETURNS TABLE (
  epoch INTEGER,
  snapshot_count INTEGER,
  average_trust_usd NUMERIC(20, 10),
  fee_entries_inserted BIGINT,
  pnl_entries_inserted BIGINT,
  roi_entries_inserted BIGINT,
  fee_iq_total NUMERIC(30, 0),
  pnl_iq_total NUMERIC(30, 0),
  roi_iq_total NUMERIC(30, 0),
  total_iq NUMERIC(30, 0)
) AS $$
#variable_conflict use_column
-- GWTH-4354: pre-existing bug fix, unrelated to the leaderboard cutoff.
-- This RETURNS TABLE's first OUT column is named `epoch`, which shadows
-- the `epoch` column of `season2_epoch_price` and `season2_iq_ledger`
-- everywhere a bare `epoch` reference appears in a DML statement below,
-- making Postgres raise "column reference \"epoch\" is ambiguous". There
-- are FIVE such sites, not four: the `ON CONFLICT (epoch)` clause in the
-- season2_epoch_price upsert, the three `ON CONFLICT (epoch, entry_type,
-- source_id)` clauses in the season2_iq_ledger inserts (fee, pnl, roi) —
-- and a fifth that is easy to miss because it is not an ON CONFLICT
-- clause at all: `DELETE FROM season2_iq_ledger WHERE epoch = p_epoch`
-- inside the `IF p_force THEN` branch a few lines below. That fifth site
-- is exactly as ambiguous as the other four, but it never surfaced during
-- development because p_force has never been exercised — every
-- verification and live call so far used p_force = FALSE (see
-- scripts/season2_verify_leaderboard_cutoff.sql's p_force test, added to
-- cover this). This was never caught because settle_season2_epoch has
-- never been successfully run in any environment (season2_iq_ledger is
-- empty and settled_at is NULL for every epoch everywhere, verified
-- 2026-09-10) — see docs/leaderboard-wind-down-epoch-20.md for how this
-- was found (this migration's own verification script,
-- scripts/season2_verify_leaderboard_cutoff.sql, failed against the
-- unpatched body on first run). `use_column` makes plpgsql prefer the
-- column interpretation for every such ambiguity in this function; the
-- function already never references its OUT columns by their bare names
-- as variables (it consistently uses v_-prefixed locals and p_-prefixed
-- params instead), so this cannot change any other behaviour.
--
-- CAUTION for future edits: `#variable_conflict use_column` disables
-- Postgres's 42702 ambiguity detection for every bare identifier in THIS
-- ENTIRE FUNCTION, not just the five sites above — it is a function-wide
-- pragma, not a per-statement one. If a future edit adds a bare
-- reference that collides with a column name but actually intends the
-- VARIABLE (not the column), it will silently resolve to the column
-- instead of raising 42702 — a correctness bug with no warning at
-- CREATE FUNCTION time or at call time. If this function is revisited,
-- prefer a more surgical fix over relying further on this function-wide
-- pragma: `ON CONFLICT ON CONSTRAINT <constraint_name>` (e.g.
-- `season2_iq_ledger_epoch_entry_source_unique` for the ledger inserts,
-- `season2_epoch_price_pkey` for the price upsert) resolves each site's
-- ambiguity individually, by naming the constraint instead of the column
-- list, without touching the function's global identifier resolution
-- rule.
DECLARE
  v_start_at TIMESTAMPTZ;
  v_end_at TIMESTAMPTZ;
  v_settle_after TIMESTAMPTZ;
  v_is_final BOOLEAN;
  v_snapshot_count INTEGER;
  v_avg_price NUMERIC(20, 10);

  v_fee_entries_inserted BIGINT := 0;
  v_pnl_entries_inserted BIGINT := 0;
  v_roi_entries_inserted BIGINT := 0;

  v_fee_iq_total NUMERIC(30, 0) := 0;
  v_pnl_iq_total NUMERIC(30, 0) := 0;
  v_roi_iq_total NUMERIC(30, 0) := 0;
BEGIN
  SELECT
    se.start_at,
    se.end_at,
    se.settle_after,
    se.is_final
  INTO
    v_start_at,
    v_end_at,
    v_settle_after,
    v_is_final
  FROM season2_epoch se
  WHERE se.epoch = p_epoch;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'Season 2 epoch % not found', p_epoch;
  END IF;

  IF v_is_final THEN
    RAISE EXCEPTION 'Season 2 epoch % is already finalized', p_epoch;
  END IF;

  IF NOW() < v_settle_after THEN
    RAISE EXCEPTION
      'Season 2 epoch % cannot be settled before settle_after (%)',
      p_epoch,
      v_settle_after;
  END IF;

  SELECT
    COUNT(*)::INTEGER,
    AVG(stps.price_usd)::NUMERIC(20, 10)
  INTO
    v_snapshot_count,
    v_avg_price
  FROM season2_trust_price_snapshot stps
  WHERE stps.snapshot_at >= v_start_at
    AND stps.snapshot_at < v_end_at;

  IF v_snapshot_count = 0 OR v_avg_price IS NULL THEN
    RAISE EXCEPTION 'No TRUST/USD snapshots found for Season 2 epoch %', p_epoch;
  END IF;

  INSERT INTO season2_epoch_price (
    epoch,
    snapshot_count,
    average_price_usd,
    computed_at,
    updated_at
  )
  VALUES (
    p_epoch,
    v_snapshot_count,
    v_avg_price,
    NOW(),
    NOW()
  )
  ON CONFLICT (epoch) DO UPDATE
  SET
    snapshot_count = EXCLUDED.snapshot_count,
    average_price_usd = EXCLUDED.average_price_usd,
    computed_at = EXCLUDED.computed_at,
    updated_at = NOW();

  IF p_force THEN
    DELETE FROM season2_iq_ledger
    WHERE epoch = p_epoch;
  END IF;

  -- Fee IQ:  amount(wei TRUST) -> TRUST -> USD via epoch avg -> IQ at 2000/$1
  -- GWTH-4354: unaffected by the leaderboard cutoff — fee IQ keeps accruing
  -- for every epoch, including 21+.
  WITH inserted_fee AS (
    INSERT INTO season2_iq_ledger (
      epoch,
      account_id,
      entry_type,
      source_id,
      iq_points,
      metadata
    )
    SELECT
      p_epoch,
      pfa.sender_id,
      'fee',
      pfa.id,
      ROUND((((pfa.amount / 1000000000000000000::NUMERIC) * v_avg_price) * 2000), 0)::NUMERIC(30, 0),
      jsonb_build_object(
        'fee_amount_raw', pfa.amount,
        'average_trust_usd', v_avg_price,
        'formula', '(amount_trust * average_trust_usd) * 2000'
      )
    FROM protocol_fee_accrued pfa
    WHERE pfa.epoch = p_epoch::NUMERIC
    ON CONFLICT (epoch, entry_type, source_id) DO NOTHING
    RETURNING iq_points
  )
  SELECT
    COUNT(*)::BIGINT,
    COALESCE(SUM(iq_points), 0)::NUMERIC(30, 0)
  INTO
    v_fee_entries_inserted,
    v_fee_iq_total
  FROM inserted_fee;

  -- GWTH-4354: leaderboard IQ (nominal PnL + ROI) is frozen after Epoch 20.
  -- No leaderboard_pnl/leaderboard_roi entries are inserted for epochs
  -- past season2_last_leaderboard_epoch(); v_pnl_entries_inserted and
  -- v_roi_entries_inserted stay at their initialised 0.
  IF p_epoch <= season2_last_leaderboard_epoch() THEN
    -- GWTH-4354: same pre-existing bug fix as below (see that DROP
    -- statement's comment), applied here too: a PRIOR settle_season2_epoch
    -- call earlier in the same session/transaction (e.g. someone settling
    -- several missed epochs back to back in one psql session) can leave
    -- get_pnl_leaderboard_period's temp tables behind from ITS roi call,
    -- since they are session-scoped and only ever dropped at actual
    -- transaction commit. Dropping them here too, before this call's own
    -- pnl block, makes each settle_season2_epoch execution self-cleaning
    -- regardless of what ran before it in the same session — discovered
    -- against intuition-testnet-next when a third settle_season2_epoch
    -- call in the same verification script collided with the previous
    -- call's leftover roi-call tables.
    DROP TABLE IF EXISTS
      _tmp_active_accounts,
      _tmp_position_data,
      _tmp_prices_at_start,
      _tmp_prices_at_end,
      _tmp_vault_state_at_start,
      _tmp_vault_state_at_end,
      _tmp_realized_fifo;

    -- Nominal PnL leaderboard IQ
    WITH inserted_pnl AS (
      INSERT INTO season2_iq_ledger (
        epoch,
        account_id,
        entry_type,
        source_id,
        iq_points,
        metadata
      )
      SELECT
        p_epoch,
        lb.account_id,
        'leaderboard_pnl',
        lb.account_id,
        slp.iq_points,
        jsonb_build_object(
          'rank', lb.rank,
          'leaderboard', 'pnl',
          'sort_by', 'total_pnl'
        )
      FROM get_pnl_leaderboard_period(
        v_start_at,
        v_end_at,
        25,
        0,
        'total_pnl',
        'DESC',
        TRUE,
        1,
        0,
        NULL
      ) lb
      JOIN season2_leaderboard_payout slp
        ON slp.rank = lb.rank::INTEGER
      ON CONFLICT (epoch, entry_type, source_id) DO NOTHING
      RETURNING iq_points
    )
    SELECT
      COUNT(*)::BIGINT,
      COALESCE(SUM(iq_points), 0)::NUMERIC(30, 0)
    INTO
      v_pnl_entries_inserted,
      v_pnl_iq_total
    FROM inserted_pnl;

    -- GWTH-4354: second pre-existing bug fix, also unrelated to the
    -- leaderboard cutoff. get_pnl_leaderboard_period() creates SEVEN
    -- `ON COMMIT DROP` temp tables (_tmp_active_accounts,
    -- _tmp_position_data, _tmp_prices_at_start, _tmp_prices_at_end,
    -- _tmp_vault_state_at_start, _tmp_vault_state_at_end,
    -- _tmp_realized_fifo), which are only dropped when the enclosing
    -- transaction actually commits — not between calls inside the same
    -- transaction. This function calls get_pnl_leaderboard_period() a
    -- second time immediately below (for the ROI board), so without
    -- these DROPs the second call's CREATE TEMP TABLE statements fail
    -- with 'relation "..." already exists' (discovered one table at a
    -- time against intuition-testnet-next: _tmp_active_accounts first,
    -- then _tmp_position_data). This means the ORIGINAL, unpatched
    -- settle_season2_epoch could never successfully settle any epoch that
    -- reached both the pnl and roi INSERT blocks — i.e. it could never
    -- successfully settle any epoch, period — which is consistent with
    -- settlement never having been run anywhere (see
    -- docs/leaderboard-wind-down-epoch-20.md). All seven are
    -- self-contained scratch tables used only inside
    -- get_pnl_leaderboard_period's own body (created, [ANALYZEd where
    -- applicable,] and joined against there, and nowhere else) — by the
    -- time it returns its result rows to this caller, none of them have
    -- any further purpose, so dropping them here is safe and does not
    -- touch get_pnl_leaderboard_period itself (a live, portal-facing
    -- function outside this migration's scope).
    DROP TABLE IF EXISTS
      _tmp_active_accounts,
      _tmp_position_data,
      _tmp_prices_at_start,
      _tmp_prices_at_end,
      _tmp_vault_state_at_start,
      _tmp_vault_state_at_end,
      _tmp_realized_fifo;

    -- ROI leaderboard IQ
    WITH inserted_roi AS (
      INSERT INTO season2_iq_ledger (
        epoch,
        account_id,
        entry_type,
        source_id,
        iq_points,
        metadata
      )
      SELECT
        p_epoch,
        lb.account_id,
        'leaderboard_roi',
        lb.account_id,
        slp.iq_points,
        jsonb_build_object(
          'rank', lb.rank,
          'leaderboard', 'roi',
          'sort_by', 'pnl_pct'
        )
      FROM get_pnl_leaderboard_period(
        v_start_at,
        v_end_at,
        25,
        0,
        'pnl_pct',
        'DESC',
        TRUE,
        1,
        0,
        NULL
      ) lb
      JOIN season2_leaderboard_payout slp
        ON slp.rank = lb.rank::INTEGER
      ON CONFLICT (epoch, entry_type, source_id) DO NOTHING
      RETURNING iq_points
    )
    SELECT
      COUNT(*)::BIGINT,
      COALESCE(SUM(iq_points), 0)::NUMERIC(30, 0)
    INTO
      v_roi_entries_inserted,
      v_roi_iq_total
    FROM inserted_roi;
  END IF;

  UPDATE season2_epoch
  SET
    settled_at = NOW(),
    last_settlement_force = p_force,
    updated_at = NOW()
  WHERE season2_epoch.epoch = p_epoch;

  SELECT
    COALESCE(SUM(sil.iq_points) FILTER (WHERE sil.entry_type = 'fee'), 0)::NUMERIC(30, 0),
    COALESCE(SUM(sil.iq_points) FILTER (WHERE sil.entry_type = 'leaderboard_pnl'), 0)::NUMERIC(30, 0),
    COALESCE(SUM(sil.iq_points) FILTER (WHERE sil.entry_type = 'leaderboard_roi'), 0)::NUMERIC(30, 0)
  INTO
    v_fee_iq_total,
    v_pnl_iq_total,
    v_roi_iq_total
  FROM season2_iq_ledger sil
  WHERE sil.epoch = p_epoch;

  RETURN QUERY
  SELECT
    p_epoch,
    v_snapshot_count,
    v_avg_price,
    v_fee_entries_inserted,
    v_pnl_entries_inserted,
    v_roi_entries_inserted,
    COALESCE(v_fee_iq_total, 0)::NUMERIC(30, 0),
    COALESCE(v_pnl_iq_total, 0)::NUMERIC(30, 0),
    COALESCE(v_roi_iq_total, 0)::NUMERIC(30, 0),
    (COALESCE(v_fee_iq_total, 0) + COALESCE(v_pnl_iq_total, 0) + COALESCE(v_roi_iq_total, 0))::NUMERIC(30, 0);
END;
$$ LANGUAGE plpgsql VOLATILE;

-- ========================================
-- 3. REVERSE ANY LEADERBOARD IQ ALREADY SETTLED PAST THE CUTOFF
-- ========================================

-- Idempotent: `season2_iq_ledger` is empty in every environment as of
-- 2026-09-10 (settlement has never been run), so this deletes 0 rows
-- today. It exists so this migration is correct even if a manual
-- settlement is run between the ticket being written and this migration
-- landing, and so re-applying it is always safe.
DELETE FROM season2_iq_ledger
WHERE epoch > season2_last_leaderboard_epoch()
  AND entry_type IN ('leaderboard_pnl', 'leaderboard_roi');
-- END EMBEDDED up.sql

-- ========================================
-- Step C. ASSERT: migration's cleanup DELETE removed the pre-existing
--    leaderboard_pnl row for epoch 21, and left the fee row alone
-- ========================================

DO $$
DECLARE
  v_pnl_gone_count INTEGER;
  v_fee_survives_count INTEGER;
BEGIN
  SELECT COUNT(*) INTO v_pnl_gone_count
  FROM season2_iq_ledger
  WHERE epoch = 21
    AND entry_type = 'leaderboard_pnl'
    AND source_id = 'fixture-gwth4354-pnl-epoch21-preexisting';

  SELECT COUNT(*) INTO v_fee_survives_count
  FROM season2_iq_ledger
  WHERE epoch = 21
    AND entry_type = 'fee'
    AND source_id = 'fixture-gwth4354-fee-epoch21-preexisting';

  ASSERT v_pnl_gone_count = 0,
    format('expected pre-existing epoch-21 leaderboard_pnl fixture row to be deleted by the migration, but %s row(s) remain', v_pnl_gone_count);

  ASSERT v_fee_survives_count = 1,
    format('expected pre-existing epoch-21 fee fixture row to survive the migration''s DELETE, but found %s row(s)', v_fee_survives_count);

  RAISE NOTICE 'PASS: migration DELETE removed the epoch-21 leaderboard_pnl fixture and left the fee fixture untouched';
END $$;

-- ========================================
-- Step D. FIXTURES for the settle_season2_epoch() behavioural assertions
-- ========================================

-- Epoch 6 (<=20): wide real-data window on intuition-testnet-next, used to
-- prove leaderboard entries are still inserted for epochs at/under the
-- cutoff. settle_after is far in the past so NOW() >= settle_after always
-- holds.
INSERT INTO season2_epoch (epoch, start_at, end_at, settle_after)
VALUES (6, TIMESTAMPTZ '2020-01-01 00:00:00+00', NOW(), TIMESTAMPTZ '2000-01-01 00:00:00+00');

-- Epochs -7 (<=20) and 90000041 (>20): fully synthetic, mutually isolated,
-- and share an IDENTICAL window so they see an identical average
-- TRUST/USD price — required for the byte-identical fee assertion below.
--
-- Epoch numbers deliberately avoid small integers: protocol_fee_accrued's
-- `epoch` column turns out NOT to follow season2_epoch's 8..30 calendar —
-- it is a separate, much finer-grained, densely-populated counter (real
-- intuition-testnet-next data has rows for nearly every integer from 1 to
-- 322+ as of 2026-09-10, unrelated to Season 2's 14-day epochs). A first
-- version of this script used epoch 7 and 41 here and the byte-identical
-- assertion failed — not because settle_season2_epoch was wrong, but
-- because real protocol_fee_accrued rows at epoch=7 (820 rows) and
-- epoch=41 (96 rows) leaked into the fee totals alongside the fixture
-- rows, and those real counts differ between 7 and 41. -7 (negative,
-- impossible for a counter that only ever counts up from a small positive
-- number) and 90000041 (many orders of magnitude past the real counter's
-- current ~322) are chosen so this fixture can never collide with real
-- protocol_fee_accrued data, regardless of how far that counter grows.
INSERT INTO season2_epoch (epoch, start_at, end_at, settle_after)
VALUES
  (-7, TIMESTAMPTZ '2099-01-01 00:00:00+00', TIMESTAMPTZ '2099-01-02 00:00:00+00', TIMESTAMPTZ '2000-01-01 00:00:00+00'),
  (90000041, TIMESTAMPTZ '2099-01-01 00:00:00+00', TIMESTAMPTZ '2099-01-02 00:00:00+00', TIMESTAMPTZ '2000-01-01 00:00:00+00');

-- Shared TRUST/USD snapshots inside the epoch -7 / epoch 90000041 window.
-- Two snapshots (the canonical 00:00 / 12:00 cadence) at a fixed price so
-- the computed average is deterministic and identical for both epochs.
INSERT INTO season2_trust_price_snapshot (snapshot_at, price_usd, source)
VALUES
  (TIMESTAMPTZ '2099-01-01 00:00:00+00', 1.2345000000, 'fixture-gwth4354'),
  (TIMESTAMPTZ '2099-01-01 12:00:00+00', 1.2345000000, 'fixture-gwth4354');

-- Identical protocol_fee_accrued amount for epoch -7 and epoch 90000041 —
-- same sender, same amount, different id/epoch/tx hash only. With the
-- identical snapshot price above, and with both epoch numbers collision-free
-- against real protocol_fee_accrued rows (see the comment above), this
-- makes the resulting fee_iq_total identical by construction for both
-- epochs.
INSERT INTO protocol_fee_accrued (id, epoch, sender_id, amount, block_number, created_at, transaction_hash)
VALUES
  ('fixture-gwth4354-fee-epoch-neg7', -7, 'fixture-gwth4354-account', 1000000000000000000, 1, NOW(), '0xfixturegwth4354epochneg7'),
  ('fixture-gwth4354-fee-epoch90000041', 90000041, 'fixture-gwth4354-account', 1000000000000000000, 1, NOW(), '0xfixturegwth4354epoch90000041');

-- ========================================
-- Step E. SETTLE all three fixture epochs and ASSERT the differential
--    behaviour
-- ========================================

DO $$
DECLARE
  -- epoch 6 (<=20, real data)
  r6_pnl_entries BIGINT;
  r6_roi_entries BIGINT;

  -- epoch 90000041 (>20, synthetic, isolated)
  rhi_pnl_entries BIGINT;
  rhi_roi_entries BIGINT;
  rhi_fee_entries BIGINT;
  rhi_fee_iq_total NUMERIC(30, 0);
  rhi_settled_at TIMESTAMPTZ;

  -- epoch -7 (<=20, synthetic, isolated — same fee inputs as epoch 90000041)
  rlo_fee_iq_total NUMERIC(30, 0);
BEGIN
  -- --- Epoch 6: <=20 must still insert leaderboard entries ---
  SELECT pnl_entries_inserted, roi_entries_inserted
  INTO r6_pnl_entries, r6_roi_entries
  FROM settle_season2_epoch(6, FALSE);

  -- This is the only assertion in this script coupled to live data volume
  -- (see the file header's FIXTURE DESIGN note). If it fails, first rule
  -- out that the environment simply lacks live PnL/trading activity in the
  -- 2020-01-01..now() window — e.g. a fresh/reset database, or a change to
  -- get_pnl_leaderboard_period's own eligibility filtering that legitimately
  -- shrank the qualifying set to 0 — BEFORE treating this as a cutoff-guard
  -- regression. The cutoff guard itself is exhaustively covered on its own
  -- terms by the boundary test in Step F below, which does not depend on
  -- any particular volume of live data.
  ASSERT r6_pnl_entries > 0,
    format('expected epoch 6 (<=20) to insert >0 leaderboard_pnl entries, got %s. Before treating this as a cutoff-guard regression, rule out that this environment simply has no live PnL activity in the 2020-01-01..now() window this fixture scans (e.g. a fresh/reset database) — this is the one assertion in this script that depends on live data volume rather than a synthetic, isolated fixture. See Step F below for a volume-independent boundary check.', r6_pnl_entries);
  ASSERT r6_roi_entries > 0,
    format('expected epoch 6 (<=20) to insert >0 leaderboard_roi entries, got %s. Before treating this as a cutoff-guard regression, rule out that this environment simply has no live PnL activity in the 2020-01-01..now() window this fixture scans (e.g. a fresh/reset database) — this is the one assertion in this script that depends on live data volume rather than a synthetic, isolated fixture. See Step F below for a volume-independent boundary check.', r6_roi_entries);

  RAISE NOTICE 'PASS: epoch 6 (<=20, real testnet-next data) inserted % leaderboard_pnl and % leaderboard_roi entries', r6_pnl_entries, r6_roi_entries;

  -- --- Epoch 90000041: >20 must insert exactly 0 leaderboard entries
  --     while still inserting fee entries and setting settled_at, without
  --     raising. (If settle_season2_epoch(90000041, FALSE) raised, this DO
  --     block would abort right here and -v ON_ERROR_STOP=1 would fail the
  --     whole script — so reaching the assertions below already proves it
  --     did not raise.) ---
  SELECT pnl_entries_inserted, roi_entries_inserted, fee_entries_inserted, fee_iq_total
  INTO rhi_pnl_entries, rhi_roi_entries, rhi_fee_entries, rhi_fee_iq_total
  FROM settle_season2_epoch(90000041, FALSE);

  SELECT settled_at INTO rhi_settled_at FROM season2_epoch WHERE epoch = 90000041;

  ASSERT rhi_pnl_entries = 0,
    format('expected epoch 90000041 (>20) to insert exactly 0 leaderboard_pnl entries, got %s', rhi_pnl_entries);
  ASSERT rhi_roi_entries = 0,
    format('expected epoch 90000041 (>20) to insert exactly 0 leaderboard_roi entries, got %s', rhi_roi_entries);
  ASSERT rhi_fee_entries = 1,
    format('expected epoch 90000041 (>20) to insert exactly the 1 fixture fee entry in the SAME run, got %s', rhi_fee_entries);
  ASSERT rhi_settled_at IS NOT NULL,
    'expected epoch 90000041 (>20) settled_at to be set after settlement';

  RAISE NOTICE 'PASS: epoch 90000041 (>20) settled without raising — 0 leaderboard entries, % fee entries, settled_at=%', rhi_fee_entries, rhi_settled_at;

  -- --- Epoch -7: <=20 counterpart with the SAME fee inputs as epoch
  --     90000041 ---
  SELECT fee_iq_total INTO rlo_fee_iq_total
  FROM settle_season2_epoch(-7, FALSE);

  ASSERT rlo_fee_iq_total = rhi_fee_iq_total,
    format('expected byte-identical fee_iq_total for the same fixture whether epoch is <=20 or >20: epoch -7 = %s, epoch 90000041 = %s', rlo_fee_iq_total, rhi_fee_iq_total);
  ASSERT rlo_fee_iq_total > 0,
    format('expected the shared fixture to produce a nonzero fee_iq_total, got %s', rlo_fee_iq_total);

  RAISE NOTICE 'PASS: fee_iq_total is byte-identical for the same fixture regardless of the leaderboard cutoff (epoch -7 = epoch 90000041 = %)', rlo_fee_iq_total;
END $$;

-- ========================================
-- Step F. BOUNDARY TEST: cutoff override, synthetic epochs pinned exactly
--    at the boundary (cutoff and cutoff+1) — the most important assertion
--    in this script
-- ========================================
--
-- Every fixture above is deliberately FAR from the real cutoff (6 and -7
-- for <=20, 90000041 for >20), so an off-by-one in the guard's `<=` (e.g.
-- a typo'd `<`) would still pass every assertion above without detection.
-- This phase closes that gap: it temporarily redefines
-- season2_last_leaderboard_epoch() to return -100 (still outside
-- season2_epoch's real 8..30 calendar, so it cannot collide with real
-- rows), seeds one epoch AT the new cutoff (-100) and one epoch ONE PAST
-- it (-99), and asserts the guard's boundary is exactly `<=`, not `<` or
-- off by any other amount.
--
-- Postgres invalidates any cached plan that reads
-- season2_last_leaderboard_epoch() when it is CREATE OR REPLACEd (a
-- pg_proc catalog change triggers a dependency invalidation for anything
-- that referenced the old definition), so settle_season2_epoch calls made
-- AFTER the override below see -100, not 20, without needing a new
-- session — this is the same CREATE OR REPLACE mechanism the migration
-- itself relies on.
--
-- Getting real leaderboard rows for a synthetic epoch requires
-- get_pnl_leaderboard_period to return rows for that window, and seeding
-- enough position/PnL data to make that happen from scratch is impractical
-- (see the file header). So -100 and -99 reuse the EXACT same
-- [start_at, end_at) window as the epoch-6 fixture above (2020-01-01 ..
-- now()), which is already exercised and known to yield real leaderboard
-- rows on intuition-testnet-next (Step E, epoch 6) — they inherit that
-- real qualifying data instead of needing their own. Each still gets its
-- own synthetic protocol_fee_accrued row (mirroring the epoch -7 / 90000041
-- pattern in Step D) so the fee-side assertion below doesn't depend on real
-- fee data lining up with this window.

CREATE OR REPLACE FUNCTION season2_last_leaderboard_epoch()
RETURNS INTEGER AS $$ SELECT -100; $$ LANGUAGE sql IMMUTABLE;

INSERT INTO season2_epoch (epoch, start_at, end_at, settle_after)
VALUES
  (-100, TIMESTAMPTZ '2020-01-01 00:00:00+00', NOW(), TIMESTAMPTZ '2000-01-01 00:00:00+00'),
  (-99,  TIMESTAMPTZ '2020-01-01 00:00:00+00', NOW(), TIMESTAMPTZ '2000-01-01 00:00:00+00');

INSERT INTO protocol_fee_accrued (id, epoch, sender_id, amount, block_number, created_at, transaction_hash)
VALUES
  ('fixture-gwth4354-fee-epoch-neg100', -100, 'fixture-gwth4354-account', 1000000000000000000, 1, NOW(), '0xfixturegwth4354epochneg100'),
  ('fixture-gwth4354-fee-epoch-neg99', -99, 'fixture-gwth4354-account', 1000000000000000000, 1, NOW(), '0xfixturegwth4354epochneg99');

DO $$
DECLARE
  bnd_at_pnl_entries BIGINT;
  bnd_at_roi_entries BIGINT;
  bnd_at_fee_entries BIGINT;
  bnd_past_pnl_entries BIGINT;
  bnd_past_roi_entries BIGINT;
  bnd_past_fee_entries BIGINT;
BEGIN
  -- --- Epoch -100 (== overridden cutoff): must behave like a <=cutoff epoch ---
  SELECT pnl_entries_inserted, roi_entries_inserted, fee_entries_inserted
  INTO bnd_at_pnl_entries, bnd_at_roi_entries, bnd_at_fee_entries
  FROM settle_season2_epoch(-100, FALSE);

  ASSERT bnd_at_pnl_entries > 0,
    format('boundary test: expected epoch -100 (== overridden cutoff -100) to insert >0 leaderboard_pnl entries, got %s — the guard may be using < instead of <=', bnd_at_pnl_entries);
  ASSERT bnd_at_roi_entries > 0,
    format('boundary test: expected epoch -100 (== overridden cutoff -100) to insert >0 leaderboard_roi entries, got %s — the guard may be using < instead of <=', bnd_at_roi_entries);
  ASSERT bnd_at_fee_entries > 0,
    format('boundary test: expected epoch -100 (== overridden cutoff -100) to insert >0 fee entries, got %s', bnd_at_fee_entries);

  -- --- Epoch -99 (== overridden cutoff + 1): must behave like a >cutoff epoch ---
  SELECT pnl_entries_inserted, roi_entries_inserted, fee_entries_inserted
  INTO bnd_past_pnl_entries, bnd_past_roi_entries, bnd_past_fee_entries
  FROM settle_season2_epoch(-99, FALSE);

  ASSERT bnd_past_pnl_entries = 0,
    format('boundary test: expected epoch -99 (== overridden cutoff -100 + 1) to insert exactly 0 leaderboard_pnl entries, got %s — the guard may be off by one', bnd_past_pnl_entries);
  ASSERT bnd_past_roi_entries = 0,
    format('boundary test: expected epoch -99 (== overridden cutoff -100 + 1) to insert exactly 0 leaderboard_roi entries, got %s — the guard may be off by one', bnd_past_roi_entries);
  ASSERT bnd_past_fee_entries > 0,
    format('boundary test: expected epoch -99 (== overridden cutoff -100 + 1) to insert >0 fee entries, got %s', bnd_past_fee_entries);

  RAISE NOTICE 'PASS: boundary test — epoch -100 (== cutoff) inserted % pnl / % roi entries, epoch -99 (== cutoff+1) inserted 0 / 0, both inserted fee entries (% / %)',
    bnd_at_pnl_entries, bnd_at_roi_entries, bnd_at_fee_entries, bnd_past_fee_entries;
END $$;

-- Restore the real cutoff constant immediately, so every phase below this
-- point tests the ACTUAL production guard (epoch 20), not the -100
-- override above.
CREATE OR REPLACE FUNCTION season2_last_leaderboard_epoch()
RETURNS INTEGER AS $$
  SELECT 20;
$$ LANGUAGE sql IMMUTABLE;

-- Not decorative: this is what catches a botched restore (e.g. a typo
-- left the override in place) instead of silently letting every phase
-- below test the WRONG cutoff.
DO $$
DECLARE
  v_restored_cutoff INTEGER;
BEGIN
  SELECT season2_last_leaderboard_epoch() INTO v_restored_cutoff;
  ASSERT v_restored_cutoff = 20,
    format('expected season2_last_leaderboard_epoch() to be restored to 20 after the Step F boundary override, got %s — every phase below this point would be testing the WRONG cutoff', v_restored_cutoff);
  RAISE NOTICE 'PASS: season2_last_leaderboard_epoch() restored to the real cutoff (20)';
END $$;

-- ========================================
-- Step G. p_force = TRUE PAST THE CUTOFF: force-resettling cannot
--    resurrect leaderboard IQ, but does restore fee IQ after its own
--    DELETE
-- ========================================
--
-- p_force = TRUE makes settle_season2_epoch DELETE every existing
-- season2_iq_ledger row for that epoch (ALL entry types, not just
-- leaderboard ones — see the `IF p_force THEN DELETE ...` block near the
-- top of the function, itself one of the five ambiguous-`epoch` sites
-- documented in up.sql), then re-run the fee INSERT unconditionally and
-- the leaderboard INSERTs only if the epoch is still <= the cutoff. This
-- re-settles epoch 90000041 (already settled once with p_force = FALSE in
-- Step E, which inserted its 1 fee row and 0 leaderboard rows) to prove
-- force mode is not a backdoor around the cutoff guard.

DO $$
DECLARE
  force_pnl_entries BIGINT;
  force_roi_entries BIGINT;
  force_fee_entries BIGINT;
BEGIN
  SELECT pnl_entries_inserted, roi_entries_inserted, fee_entries_inserted
  INTO force_pnl_entries, force_roi_entries, force_fee_entries
  FROM settle_season2_epoch(90000041, TRUE);

  ASSERT force_pnl_entries = 0,
    format('p_force test: expected force-resettling epoch 90000041 (>20) to insert exactly 0 leaderboard_pnl entries, got %s — p_force must not be able to resurrect leaderboard IQ past the cutoff', force_pnl_entries);
  ASSERT force_roi_entries = 0,
    format('p_force test: expected force-resettling epoch 90000041 (>20) to insert exactly 0 leaderboard_roi entries, got %s — p_force must not be able to resurrect leaderboard IQ past the cutoff', force_roi_entries);
  ASSERT force_fee_entries > 0,
    format('p_force test: expected force-resettling epoch 90000041 (>20) to insert >0 fee entries (restoring what its own DELETE just removed), got %s', force_fee_entries);

  RAISE NOTICE 'PASS: p_force=TRUE on epoch 90000041 (>20) re-inserted % fee entries and 0 leaderboard entries — force cannot resurrect leaderboard IQ past the cutoff', force_fee_entries;
END $$;

-- ========================================
-- Step H. DOWN.SQL SMOKE TEST (must run LAST — see warning below)
--    Source: infrastructure/hasura/migrations/intuition/1771526444000_leaderboard_wind_down_epoch_20/down.sql
--    Keep this block byte-identical to that file. Checked by
--    scripts/season2_check_verify_script_sync.sh.
-- ========================================
--
-- Embeds down.sql verbatim and asserts that, after it runs,
-- settle_season2_epoch is back to raising the pre-fix ambiguity error.
-- This is deliberately the LAST phase in this script: down.sql restores
-- settle_season2_epoch to a body that cannot successfully settle ANY
-- epoch (see down.sql's own header and its RAISE WARNING), so every
-- phase above this point would break if this ran first. Nothing here
-- persists — the whole script still rolls back at the end.

-- BEGIN EMBEDDED down.sql (verbatim - keep byte-identical, see scripts/season2_check_verify_script_sync.sh)
-- GWTH-4354: revert the leaderboard wind-down.
--
-- Restores settle_season2_epoch(...) to the verbatim pre-migration body
-- from migration 1771526400000_add_season2_iq_points_mvp (unconditional
-- leaderboard_pnl / leaderboard_roi inserts for every epoch), and drops
-- the season2_last_leaderboard_epoch() cutoff constant.
--
-- NOTE: this cannot resurrect any leaderboard_pnl / leaderboard_roi rows
-- that up.sql's DELETE removed (or that were never inserted because the
-- guard skipped them). If leaderboard IQ needs to exist for those epochs
-- again, it must be re-settled (e.g. `settle_season2_epoch(N, TRUE)`)
-- after this rollback — there is no way to recover the original values
-- from a DELETE.
--
-- NOTE: this also reintroduces two pre-existing bugs that up.sql happened
-- to fix as a side effect (see up.sql's changes 4 and 5, and
-- docs/leaderboard-wind-down-epoch-20.md):
--   - Without `#variable_conflict use_column`, every
--     `ON CONFLICT (epoch, ...)` clause in this function raises "column
--     reference \"epoch\" is ambiguous", because the function's
--     RETURNS TABLE OUT column is also named `epoch`.
--   - Without the two `DROP TABLE IF EXISTS` for get_pnl_leaderboard_period's
--     seven `ON COMMIT DROP` temp tables (one before the pnl INSERT
--     block, one before the roi INSERT block), a second call to
--     get_pnl_leaderboard_period() — whether the roi call within one
--     settle_season2_epoch execution, or the pnl call of a later
--     settle_season2_epoch invocation in the same session — fails with
--     'relation "..." already exists', because those tables only drop at
--     transaction commit, not between calls within the same
--     transaction/session.
-- Both predate this migration (present verbatim in
-- 1771526400000_add_season2_iq_points_mvp) and were never triggered in
-- production because settle_season2_epoch has never been successfully
-- run anywhere — between them, the original function could never
-- successfully settle any epoch at all. A true "restore pre-migration
-- state" rollback has to bring both back too — this is intentional, not
-- an oversight. If this rollback is ever actually run, re-apply both
-- fixes (or new ones) before calling settle_season2_epoch again.

CREATE OR REPLACE FUNCTION settle_season2_epoch(
  p_epoch INTEGER,
  p_force BOOLEAN DEFAULT FALSE
)
RETURNS TABLE (
  epoch INTEGER,
  snapshot_count INTEGER,
  average_trust_usd NUMERIC(20, 10),
  fee_entries_inserted BIGINT,
  pnl_entries_inserted BIGINT,
  roi_entries_inserted BIGINT,
  fee_iq_total NUMERIC(30, 0),
  pnl_iq_total NUMERIC(30, 0),
  roi_iq_total NUMERIC(30, 0),
  total_iq NUMERIC(30, 0)
) AS $$
DECLARE
  v_start_at TIMESTAMPTZ;
  v_end_at TIMESTAMPTZ;
  v_settle_after TIMESTAMPTZ;
  v_is_final BOOLEAN;
  v_snapshot_count INTEGER;
  v_avg_price NUMERIC(20, 10);

  v_fee_entries_inserted BIGINT := 0;
  v_pnl_entries_inserted BIGINT := 0;
  v_roi_entries_inserted BIGINT := 0;

  v_fee_iq_total NUMERIC(30, 0) := 0;
  v_pnl_iq_total NUMERIC(30, 0) := 0;
  v_roi_iq_total NUMERIC(30, 0) := 0;
BEGIN
  SELECT
    se.start_at,
    se.end_at,
    se.settle_after,
    se.is_final
  INTO
    v_start_at,
    v_end_at,
    v_settle_after,
    v_is_final
  FROM season2_epoch se
  WHERE se.epoch = p_epoch;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'Season 2 epoch % not found', p_epoch;
  END IF;

  IF v_is_final THEN
    RAISE EXCEPTION 'Season 2 epoch % is already finalized', p_epoch;
  END IF;

  IF NOW() < v_settle_after THEN
    RAISE EXCEPTION
      'Season 2 epoch % cannot be settled before settle_after (%)',
      p_epoch,
      v_settle_after;
  END IF;

  SELECT
    COUNT(*)::INTEGER,
    AVG(stps.price_usd)::NUMERIC(20, 10)
  INTO
    v_snapshot_count,
    v_avg_price
  FROM season2_trust_price_snapshot stps
  WHERE stps.snapshot_at >= v_start_at
    AND stps.snapshot_at < v_end_at;

  IF v_snapshot_count = 0 OR v_avg_price IS NULL THEN
    RAISE EXCEPTION 'No TRUST/USD snapshots found for Season 2 epoch %', p_epoch;
  END IF;

  INSERT INTO season2_epoch_price (
    epoch,
    snapshot_count,
    average_price_usd,
    computed_at,
    updated_at
  )
  VALUES (
    p_epoch,
    v_snapshot_count,
    v_avg_price,
    NOW(),
    NOW()
  )
  ON CONFLICT (epoch) DO UPDATE
  SET
    snapshot_count = EXCLUDED.snapshot_count,
    average_price_usd = EXCLUDED.average_price_usd,
    computed_at = EXCLUDED.computed_at,
    updated_at = NOW();

  IF p_force THEN
    DELETE FROM season2_iq_ledger
    WHERE epoch = p_epoch;
  END IF;

  -- Fee IQ:  amount(wei TRUST) -> TRUST -> USD via epoch avg -> IQ at 2000/$1
  WITH inserted_fee AS (
    INSERT INTO season2_iq_ledger (
      epoch,
      account_id,
      entry_type,
      source_id,
      iq_points,
      metadata
    )
    SELECT
      p_epoch,
      pfa.sender_id,
      'fee',
      pfa.id,
      ROUND((((pfa.amount / 1000000000000000000::NUMERIC) * v_avg_price) * 2000), 0)::NUMERIC(30, 0),
      jsonb_build_object(
        'fee_amount_raw', pfa.amount,
        'average_trust_usd', v_avg_price,
        'formula', '(amount_trust * average_trust_usd) * 2000'
      )
    FROM protocol_fee_accrued pfa
    WHERE pfa.epoch = p_epoch::NUMERIC
    ON CONFLICT (epoch, entry_type, source_id) DO NOTHING
    RETURNING iq_points
  )
  SELECT
    COUNT(*)::BIGINT,
    COALESCE(SUM(iq_points), 0)::NUMERIC(30, 0)
  INTO
    v_fee_entries_inserted,
    v_fee_iq_total
  FROM inserted_fee;

  -- Nominal PnL leaderboard IQ
  WITH inserted_pnl AS (
    INSERT INTO season2_iq_ledger (
      epoch,
      account_id,
      entry_type,
      source_id,
      iq_points,
      metadata
    )
    SELECT
      p_epoch,
      lb.account_id,
      'leaderboard_pnl',
      lb.account_id,
      slp.iq_points,
      jsonb_build_object(
        'rank', lb.rank,
        'leaderboard', 'pnl',
        'sort_by', 'total_pnl'
      )
    FROM get_pnl_leaderboard_period(
      v_start_at,
      v_end_at,
      25,
      0,
      'total_pnl',
      'DESC',
      TRUE,
      1,
      0,
      NULL
    ) lb
    JOIN season2_leaderboard_payout slp
      ON slp.rank = lb.rank::INTEGER
    ON CONFLICT (epoch, entry_type, source_id) DO NOTHING
    RETURNING iq_points
  )
  SELECT
    COUNT(*)::BIGINT,
    COALESCE(SUM(iq_points), 0)::NUMERIC(30, 0)
  INTO
    v_pnl_entries_inserted,
    v_pnl_iq_total
  FROM inserted_pnl;

  -- ROI leaderboard IQ
  WITH inserted_roi AS (
    INSERT INTO season2_iq_ledger (
      epoch,
      account_id,
      entry_type,
      source_id,
      iq_points,
      metadata
    )
    SELECT
      p_epoch,
      lb.account_id,
      'leaderboard_roi',
      lb.account_id,
      slp.iq_points,
      jsonb_build_object(
        'rank', lb.rank,
        'leaderboard', 'roi',
        'sort_by', 'pnl_pct'
      )
    FROM get_pnl_leaderboard_period(
      v_start_at,
      v_end_at,
      25,
      0,
      'pnl_pct',
      'DESC',
      TRUE,
      1,
      0,
      NULL
    ) lb
    JOIN season2_leaderboard_payout slp
      ON slp.rank = lb.rank::INTEGER
    ON CONFLICT (epoch, entry_type, source_id) DO NOTHING
    RETURNING iq_points
  )
  SELECT
    COUNT(*)::BIGINT,
    COALESCE(SUM(iq_points), 0)::NUMERIC(30, 0)
  INTO
    v_roi_entries_inserted,
    v_roi_iq_total
  FROM inserted_roi;

  UPDATE season2_epoch
  SET
    settled_at = NOW(),
    last_settlement_force = p_force,
    updated_at = NOW()
  WHERE season2_epoch.epoch = p_epoch;

  SELECT
    COALESCE(SUM(sil.iq_points) FILTER (WHERE sil.entry_type = 'fee'), 0)::NUMERIC(30, 0),
    COALESCE(SUM(sil.iq_points) FILTER (WHERE sil.entry_type = 'leaderboard_pnl'), 0)::NUMERIC(30, 0),
    COALESCE(SUM(sil.iq_points) FILTER (WHERE sil.entry_type = 'leaderboard_roi'), 0)::NUMERIC(30, 0)
  INTO
    v_fee_iq_total,
    v_pnl_iq_total,
    v_roi_iq_total
  FROM season2_iq_ledger sil
  WHERE sil.epoch = p_epoch;

  RETURN QUERY
  SELECT
    p_epoch,
    v_snapshot_count,
    v_avg_price,
    v_fee_entries_inserted,
    v_pnl_entries_inserted,
    v_roi_entries_inserted,
    COALESCE(v_fee_iq_total, 0)::NUMERIC(30, 0),
    COALESCE(v_pnl_iq_total, 0)::NUMERIC(30, 0),
    COALESCE(v_roi_iq_total, 0)::NUMERIC(30, 0),
    (COALESCE(v_fee_iq_total, 0) + COALESCE(v_pnl_iq_total, 0) + COALESCE(v_roi_iq_total, 0))::NUMERIC(30, 0);
END;
$$ LANGUAGE plpgsql VOLATILE;

DROP FUNCTION IF EXISTS season2_last_leaderboard_epoch();

DO $$ BEGIN
  RAISE WARNING 'GWTH-4354 rollback: settle_season2_epoch restored to its pre-migration body, which cannot successfully settle ANY epoch (ON CONFLICT "epoch" ambiguity + get_pnl_leaderboard_period temp-table collision). See down.sql header.';
END $$;
-- END EMBEDDED down.sql

-- Reuses epoch 6 rather than the real epoch 21: epoch 6's fixture (Step D)
-- already proved, earlier in this SAME transaction (Step E), that its
-- window has real TRUST/USD snapshot data, so the pre-fix function is
-- guaranteed to reach the ambiguous ON CONFLICT statement instead of
-- failing earlier with an unrelated "No TRUST/USD snapshots found" error
-- that would depend on real epoch 21 data lining up on whichever
-- environment this runs against.
--
-- NOTE on test design: the "unexpectedly succeeded" failure below is
-- raised OUTSIDE the BEGIN/EXCEPTION block below, via a flag, rather than
-- as a RAISE EXCEPTION *inside* that block's try section. Raising it
-- inside would have it caught by this same block's own `WHEN OTHERS`
-- handler — and if that raised message happened to mention "ambiguous"
-- (an easy word to reach for when describing this exact test), the
-- ASSERT SQLERRM ILIKE '%ambiguous%' check below would then pass against
-- OUR OWN synthetic message, letting a no-op/broken down.sql pass this
-- test. Setting a flag and checking it after the block closes avoids that
-- self-referential trap entirely.
DO $$
DECLARE
  v_unexpectedly_succeeded BOOLEAN := FALSE;
BEGIN
  BEGIN
    PERFORM * FROM settle_season2_epoch(6, FALSE);
    v_unexpectedly_succeeded := TRUE;
  EXCEPTION
    WHEN OTHERS THEN
      ASSERT SQLERRM ILIKE '%ambiguous%',
        format('down.sql smoke test: expected the post-rollback error to mention "ambiguous" (the pre-fix ON CONFLICT "epoch" bug), got: %s', SQLERRM);
      RAISE NOTICE 'PASS: down.sql restored the pre-fix ambiguous-column behaviour (error: %)', SQLERRM;
  END;

  IF v_unexpectedly_succeeded THEN
    RAISE EXCEPTION 'down.sql smoke test FAILED: settle_season2_epoch(6) completed successfully after down.sql was applied, but it should have failed with a column-resolution error on the ON CONFLICT (epoch) clause -- down.sql may not be a true revert of the pre-fix state';
  END IF;
END $$;

SELECT 'ALL ASSERTIONS PASSED for GWTH-4354 leaderboard cutoff at Epoch 20' AS result;

ROLLBACK;
