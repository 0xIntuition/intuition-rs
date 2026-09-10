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
-- script silently.
--
-- WHY THE MIGRATION IS EMBEDDED VERBATIM BELOW (not `\i`-included):
-- psql is invoked here with `-f -` reading the script over `kubectl exec -i`
-- stdin, so a `\i`/`\ir` meta-command would try to open a path on the POD's
-- filesystem, where this repo does not exist. Copy-pasting the migration's
-- up.sql content into this script (byte-for-byte — see the marked block
-- below) is the reliable way to make this script runnable via that exact
-- invocation while staying self-contained. If the migration changes, this
-- block must be re-synced from
-- infrastructure/hasura/migrations/intuition/1771526444000_leaderboard_wind_down_epoch_20/up.sql.
--
-- FIXTURE DESIGN (why real data for some assertions, synthetic for others):
-- `get_pnl_leaderboard_period` depends on deep, interdependent machinery
-- (hourly position snapshots, continuous aggregates, bonding-curve equity
-- valuation) that is impractical to seed realistically from scratch. So:
--   - The "an epoch <=20 inserts leaderboard entries" assertion uses REAL
--     intuition-testnet-next trading data, via a synthetic season2_epoch
--     fixture (epoch 6) whose window is deliberately wide (2020-01-01 ..
--     now()) to be virtually guaranteed to contain real PnL activity —
--     empirically confirmed against intuition-testnet-next on 2026-09-10
--     (25 rows for both the total_pnl and pnl_pct sorts over that window;
--     0 rows for the real epoch 20 window specifically, which is why epoch
--     20 itself isn't reused directly here).
--   - The "fee IQ totals are byte-identical whether the epoch is <=20 or
--     >20" assertion instead uses two fully synthetic, mutually isolated
--     season2_epoch fixtures (epoch -7 <=20, epoch 90000041 >20) that
--     share an IDENTICAL synthetic time window (2099-01-01 .. 2099-01-02,
--     far outside any real epoch, so it cannot collide with real snapshot
--     or position data) and an identical seeded protocol_fee_accrued
--     amount. The epoch numbers themselves are deliberately far from
--     season2_epoch's real 8..30 range AND from protocol_fee_accrued's
--     own real epoch values (see the fixture section below for why that
--     second collision is real and was hit during development). Because
--     both fixtures see the same avg TRUST/USD price and the same fee
--     amount, and the fee code path is untouched by the guard, their
--     computed fee_iq_total must be exactly equal by construction — this
--     is a real equality check, not a coincidence of live data.
--   - The "pre-seeded leaderboard_pnl row for epoch 21 is deleted, a
--     pre-seeded fee row for epoch 21 survives" assertion seeds directly
--     into season2_iq_ledger against the REAL epoch 21 (bypassing
--     settle_season2_epoch entirely) and checks the migration's own
--     cleanup DELETE, run as part of applying the embedded migration below.
--
-- Fixture epoch numbers 6, -7 and 90000041 are chosen to be outside the
-- real season2_epoch calendar (8..30 today), so they cannot collide with
-- real rows in season2_epoch, and their season2_iq_ledger /
-- season2_epoch_price rows are equally isolated (both FK on epoch). -7
-- and 90000041 are additionally chosen to avoid protocol_fee_accrued's
-- own real epoch values — see the fixture section below.

\pset pager off
\timing off
\set ON_ERROR_STOP on

BEGIN;

-- ========================================
-- 0. FIXTURE: account + pre-existing epoch-21 ledger rows
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
-- 1. APPLY THE MIGRATION (embedded verbatim — see header comment above)
--    Source: infrastructure/hasura/migrations/intuition/1771526444000_leaderboard_wind_down_epoch_20/up.sql
-- ========================================

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
-- inside every `ON CONFLICT (epoch, ...)` clause below, making Postgres
-- raise "column reference \"epoch\" is ambiguous". This was never caught
-- because settle_season2_epoch has never been successfully run in any
-- environment (season2_iq_ledger is empty and settled_at is NULL for
-- every epoch everywhere, verified 2026-09-10) — see
-- docs/leaderboard-wind-down-epoch-20.md for how this was found (this
-- migration's own verification script, scripts/season2_verify_leaderboard_cutoff.sql,
-- failed against the unpatched body on first run). `use_column` makes
-- plpgsql prefer the column interpretation for every such ambiguity in
-- this function; the function already never references its OUT columns
-- by their bare names as variables (it consistently uses v_-prefixed
-- locals and p_-prefixed params instead), so this cannot change any
-- other behaviour.
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

-- ========================================
-- 2. ASSERT: migration's cleanup DELETE removed the pre-existing
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
-- 3. FIXTURES for the settle_season2_epoch() behavioural assertions
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
-- 4. SETTLE all three fixture epochs and ASSERT the differential behaviour
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

  ASSERT r6_pnl_entries > 0,
    format('expected epoch 6 (<=20) to insert >0 leaderboard_pnl entries, got %s', r6_pnl_entries);
  ASSERT r6_roi_entries > 0,
    format('expected epoch 6 (<=20) to insert >0 leaderboard_roi entries, got %s', r6_roi_entries);

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

SELECT 'ALL ASSERTIONS PASSED for GWTH-4354 leaderboard cutoff at Epoch 20' AS result;

ROLLBACK;
