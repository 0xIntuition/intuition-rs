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
