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
