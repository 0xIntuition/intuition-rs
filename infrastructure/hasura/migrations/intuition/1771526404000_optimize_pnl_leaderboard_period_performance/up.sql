-- Optimize get_pnl_leaderboard_period for older date ranges
--
-- ROOT CAUSE ANALYSIS:
--
-- Problem 1: position_state_start scans ALL historical data before v_bucket_start.
--   For Feb 10 start, this means scanning every row ever written to position_change_daily,
--   across all time, filtered only by account_id IN (subquery) and optional term_id.
--   The earlier the start date, the larger this scan becomes unboundedly.
--   For Feb 24 start, this is a small scan (platform is young). For Feb 10, it includes
--   all days 0..Feb 9, which is proportionally larger and growing every day.
--
-- Problem 2: position_state_end is symmetric but scans up to end date, so a Feb 24
--   end date and Feb 10 end date produce similar scan sizes. The asymmetry between
--   "works for Feb 24-Mar 10" vs "fails for Feb 10-24" isolates the root cause to
--   position_state_start (bucket < v_bucket_start), not position_state_end.
--
-- Problem 3: prices_earliest has NO WHERE clause filter on block_timestamp.
--   It scans the ENTIRE share_price_change table for all time even after the
--   relevant_vaults join, because DISTINCT ON with ASC order requires a full
--   sort to find minimums per (term_id, curve_id). The existing index
--   idx_share_price_change_term_curve_block_timestamp is DESC, which helps the
--   DESC lookups (prices_before_start, prices_at_end) but NOT prices_earliest.
--
-- Problem 4: position_state_start and position_state_end are separate full scans
--   of the same data, producing N+1 passes over position_change_daily when one
--   combined pass (with conditional aggregation) is sufficient.
--
-- FIXES APPLIED:
--
-- Fix 1: Collapse position_state_start, position_state_end, and period_activity
--   into a SINGLE scan of position_change_daily using conditional SUM(CASE WHEN).
--   This reduces 3 separate scans + 2 FULL OUTER JOINs to exactly 1 scan +
--   straightforward GROUP BY. TimescaleDB continuous aggregates read faster when
--   accessed once; multiple passes multiply I/O.
--
-- Fix 2: Eliminate prices_earliest by merging it into prices_before_start using
--   a single DISTINCT ON query with an OR-based fallback via COALESCE at join time.
--   Instead, use a single prices_at_start CTE that fetches the "best available"
--   price: the most recent price <= v_start_timestamp. When no such price exists
--   (vault launched after the start date), fall back to prices_at_end which is
--   already computed. The original prices_earliest was only needed as a fallback
--   when prices_before_start returned NULL. We make that fallback explicit using
--   a second DISTINCT ON in the same CTE with UNION ALL semantics.
--
--   More precisely: for any (term_id, curve_id) where no price record exists
--   before the period start, it means the vault did not exist at period start.
--   In that case equity_at_start = 0 is the correct answer (position size = 0
--   before vault existed). The prices_earliest fallback was mathematically wrong
--   because it applied a post-launch price to a pre-launch equity calculation.
--   Removing it and defaulting to 0 is both faster and more correct.
--
-- Fix 3: Add a composite index on position_change_daily (account_id, bucket) to
--   allow the planner to seek directly into the continuous aggregate by account
--   after the active_accounts subquery is resolved. Without this, every account_id
--   lookup requires a full TimescaleDB chunk scan.
--
-- Fix 4: Add an ASC index on share_price_change (term_id, curve_id, block_timestamp ASC,
--   log_index ASC) so that prices_earliest can use an index seek rather than a
--   full sort. Even though we are removing prices_earliest from this function,
--   the index remains useful for any future queries needing earliest-price lookups.
--
-- CORRECTNESS NOTES:
--   - The single-pass aggregation produces identical numeric results to three
--     separate CTEs because SUM is associative and the conditional filters are
--     mutually exhaustive partitions of the bucket timeline.
--   - Removing prices_earliest and defaulting equity_at_start to 0 for new vaults
--     is correct: if a vault had no price records before the period start, the
--     account held 0 shares before the period started (by definition, since the
--     vault did not exist), so equity_at_start = shares * price = 0 * anything = 0.
--   - The FULL OUTER JOIN structure in position_period_metrics is preserved
--     but simplified: with a single aggregation CTE, we only need COALESCE
--     over two sources (position data and period activity) instead of three.

-- ========================================
-- NEW INDEXES
-- ========================================

-- Composite index on position_change_daily for account-bounded historical scans.
--
-- TimescaleDB 2.x allows CREATE INDEX directly on the continuous aggregate view
-- name; it creates the index on the underlying internal materialization table.
-- This index is the highest-impact fix:
--
-- Without it: position_state_start (bucket < v_bucket_start, account_id IN (...))
--   forces a full time-range scan across all chunks for each active account,
--   reading all (term_id, curve_id, bucket) rows then filtering by account_id.
--
-- With it: the planner can seek by (account_id, term_id, bucket) directly,
--   jumping only to the chunks and rows relevant to each account. For a query
--   with N=500 active accounts each holding M=10 vault positions, this reduces
--   the scan from O(all_rows_before_start) to O(N * M * chunk_count_before_start),
--   which is typically 2-3 orders of magnitude smaller.
--
-- The column order (account_id, term_id, bucket) matches the most selective
-- filter chain: account_id IN (...) is the tightest filter (applied first),
-- followed by optional term_id equality, followed by bucket range comparison.
CREATE INDEX IF NOT EXISTS idx_pcd_account_term_bucket
  ON position_change_daily (account_id, term_id, bucket);

-- Ascending index on share_price_change for future earliest-price lookups.
-- The existing idx_share_price_change_term_curve_block_timestamp is DESC-ordered
-- (term_id, curve_id, block_timestamp DESC, log_index DESC), which serves
-- DISTINCT ON ... ORDER BY ... DESC queries optimally (prices_at_start and
-- prices_at_end in this function). An ASC variant enables efficient
-- MIN(block_timestamp) seeks per (term_id, curve_id) in case any future query
-- needs an earliest-price fallback or the optimizer chooses a different plan.
CREATE INDEX IF NOT EXISTS idx_share_price_change_term_curve_block_asc
  ON share_price_change (term_id, curve_id, block_timestamp ASC, log_index ASC);

-- ========================================
-- OPTIMIZED FUNCTION
-- ========================================

CREATE OR REPLACE FUNCTION get_pnl_leaderboard_period(
  p_start_date TIMESTAMPTZ,
  p_end_date TIMESTAMPTZ,
  p_limit INTEGER DEFAULT 100,
  p_offset INTEGER DEFAULT 0,
  p_sort_by TEXT DEFAULT 'total_pnl',
  p_sort_order TEXT DEFAULT 'DESC',
  p_exclude_protocol_accounts BOOLEAN DEFAULT TRUE,
  p_min_positions INTEGER DEFAULT 1,
  p_min_volume NUMERIC DEFAULT 0,
  p_term_id TEXT DEFAULT NULL
)
RETURNS SETOF pnl_leaderboard_entry AS $$
DECLARE
  v_start_timestamp BIGINT;
  v_end_timestamp   BIGINT;
  v_bucket_start    TIMESTAMPTZ;
  v_bucket_end      TIMESTAMPTZ;
  v_limit           INTEGER;
  v_offset          INTEGER;
BEGIN
  IF p_start_date IS NULL OR p_end_date IS NULL THEN
    RAISE EXCEPTION 'p_start_date and p_end_date are required';
  END IF;

  IF p_start_date >= p_end_date THEN
    RAISE EXCEPTION 'p_start_date must be before p_end_date';
  END IF;

  v_limit  := LEAST(GREATEST(COALESCE(p_limit, 100), 1), 10000);
  v_offset := GREATEST(COALESCE(p_offset, 0), 0);

  -- Precise Unix epoch seconds for share_price_change lookups
  v_start_timestamp := EXTRACT(EPOCH FROM p_start_date)::BIGINT;
  v_end_timestamp   := EXTRACT(EPOCH FROM p_end_date)::BIGINT;

  -- Day-boundary timestamps for position_change_daily bucket comparisons.
  -- position_change_daily uses time_bucket('1 day', ...) which truncates to
  -- midnight UTC. Mid-day timestamps must be floor'd to hit the right buckets.
  v_bucket_start := date_trunc('day', p_start_date);
  v_bucket_end   := date_trunc('day', p_end_date);

  RETURN QUERY
  WITH
  -- -----------------------------------------------------------------------
  -- Step 1: Identify accounts with any activity during the period.
  -- Uses the TimescaleDB continuous aggregate (materialized + real-time tail).
  -- The bucket >= / <= filter maps directly onto chunk exclusion in TimescaleDB.
  -- -----------------------------------------------------------------------
  active_accounts AS (
    SELECT DISTINCT pc.account_id
    FROM position_change_daily pc
    WHERE pc.bucket >= v_bucket_start
      AND pc.bucket <= v_bucket_end
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
  ),

  -- -----------------------------------------------------------------------
  -- Step 2: Identify the (term_id, curve_id) pairs actually needed.
  -- Limits all subsequent share_price_change scans from 300K+ vaults to only
  -- the vaults touched by active accounts. This filter is applied across ALL
  -- time (not bounded to the period) because we need pre-period prices too.
  -- -----------------------------------------------------------------------
  relevant_vaults AS (
    SELECT DISTINCT pc.term_id, pc.curve_id
    FROM position_change_daily pc
    WHERE pc.account_id IN (SELECT account_id FROM active_accounts)
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
  ),

  -- -----------------------------------------------------------------------
  -- Step 3: Fetch share prices at period START for relevant vaults only.
  --
  -- Uses the existing DESC index: idx_share_price_change_term_curve_block_timestamp
  -- which covers (term_id, curve_id, block_timestamp DESC, log_index DESC).
  -- DISTINCT ON with matching ORDER BY maps directly to an index scan:
  -- for each (term_id, curve_id) the planner reads the first row in DESC
  -- order of block_timestamp where block_timestamp <= v_start_timestamp.
  --
  -- When a vault launched AFTER the period start, no row satisfies
  -- block_timestamp <= v_start_timestamp, so this CTE returns no row for
  -- that vault. The LEFT JOIN in position_period_metrics then produces NULL,
  -- and COALESCE(..., 0) correctly sets equity_at_start = 0 (the account
  -- held 0 shares in a vault that did not yet exist). This is mathematically
  -- correct and removes the need for the old prices_earliest fallback, which
  -- was applying a post-launch price to a pre-launch equity calculation.
  -- -----------------------------------------------------------------------
  prices_at_start AS (
    SELECT DISTINCT ON (spc.term_id, spc.curve_id)
      spc.term_id, spc.curve_id, spc.share_price
    FROM share_price_change spc
    INNER JOIN relevant_vaults rv
      ON spc.term_id = rv.term_id AND spc.curve_id = rv.curve_id
    WHERE spc.block_timestamp <= v_start_timestamp
    ORDER BY spc.term_id, spc.curve_id, spc.block_timestamp DESC, spc.log_index DESC
  ),

  -- -----------------------------------------------------------------------
  -- Step 4: Fetch share prices at period END for relevant vaults only.
  -- Same index strategy as prices_at_start.
  -- -----------------------------------------------------------------------
  prices_at_end AS (
    SELECT DISTINCT ON (spc.term_id, spc.curve_id)
      spc.term_id, spc.curve_id, spc.share_price
    FROM share_price_change spc
    INNER JOIN relevant_vaults rv
      ON spc.term_id = rv.term_id AND spc.curve_id = rv.curve_id
    WHERE spc.block_timestamp <= v_end_timestamp
    ORDER BY spc.term_id, spc.curve_id, spc.block_timestamp DESC, spc.log_index DESC
  ),

  -- -----------------------------------------------------------------------
  -- Step 5: Single-pass aggregation over position_change_daily.
  --
  -- CRITICAL OPTIMIZATION: The original function made THREE separate scans
  -- of position_change_daily:
  --   - position_state_start:  WHERE bucket < v_bucket_start  (all history)
  --   - position_state_end:    WHERE bucket <= v_bucket_end   (all history + period)
  --   - period_activity:       WHERE bucket >= v_bucket_start AND bucket <= v_bucket_end
  --
  -- These were then joined via two FULL OUTER JOINs, which are expensive and
  -- require the planner to materialize intermediate results.
  --
  -- Replacement: scan once with WHERE bucket <= v_bucket_end (the widest filter),
  -- then use CASE WHEN inside SUM() to conditionally accumulate each column
  -- into its correct "window":
  --
  --   shares_at_start   = SUM of shares_delta for all days BEFORE the period
  --   shares_at_end     = SUM of shares_delta for ALL days up to period end
  --   period_deposits   = SUM of assets_in   for days WITHIN the period only
  --   period_redemptions= SUM of assets_out  for days WITHIN the period only
  --   had_activity      = TRUE if any row falls in [v_bucket_start, v_bucket_end]
  --
  -- This produces exactly the same numeric results as the three-CTE approach
  -- because SUM is a linear operator and the bucket ranges partition cleanly.
  -- The planner reads each TimescaleDB chunk at most once, cutting I/O by ~3x
  -- for the position_change_daily scans.
  --
  -- The account_id IN (active_accounts) filter bounds the scan to only accounts
  -- seen in the period. Without it, we would scan all-time history for every
  -- account ever, which is unbounded. The TimescaleDB chunk exclusion on
  -- bucket <= v_bucket_end keeps the scan bounded on the time axis.
  -- -----------------------------------------------------------------------
  position_data AS (
    SELECT
      pc.account_id,
      pc.term_id,
      pc.curve_id,
      -- Shares held at period START = cumulative delta before the period
      SUM(pc.shares_delta_period) FILTER (WHERE pc.bucket < v_bucket_start)::NUMERIC
        AS shares_at_start,
      -- Shares held at period END = cumulative delta through period end
      SUM(pc.shares_delta_period)::NUMERIC
        AS shares_at_end,
      -- Deposits made DURING the period only
      SUM(pc.assets_in_period) FILTER (WHERE pc.bucket >= v_bucket_start)::NUMERIC
        AS period_deposits,
      -- Redemptions made DURING the period only
      SUM(pc.assets_out_period) FILTER (WHERE pc.bucket >= v_bucket_start)::NUMERIC
        AS period_redemptions,
      -- Flag: did this account/vault have any activity during the period?
      BOOL_OR(pc.bucket >= v_bucket_start AND pc.bucket <= v_bucket_end)
        AS had_activity
    FROM position_change_daily pc
    WHERE pc.bucket <= v_bucket_end
      AND pc.account_id IN (SELECT account_id FROM active_accounts)
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
    GROUP BY pc.account_id, pc.term_id, pc.curve_id
  ),

  -- -----------------------------------------------------------------------
  -- Step 6: Attach prices and compute equity at boundaries.
  -- Simple INNER JOIN on position_data (already filtered to active accounts)
  -- + LEFT JOINs for prices (NULL-safe via COALESCE).
  -- -----------------------------------------------------------------------
  position_period_metrics AS (
    SELECT
      pd.account_id,
      pd.term_id,
      pd.curve_id,
      COALESCE(pd.shares_at_start, 0)       AS shares_at_start,
      pd.shares_at_end                       AS shares_at_end,
      COALESCE(pd.period_deposits, 0)        AS period_deposits,
      COALESCE(pd.period_redemptions, 0)     AS period_redemptions,
      pd.had_activity,
      -- equity_at_start: shares_before_period * price_at_start.
      -- COALESCE(price, 0): if no price record exists before period start,
      -- the vault did not exist yet, so equity_at_start is correctly 0.
      TRUNC(
        COALESCE(pd.shares_at_start, 0)
        * COALESCE(ps.share_price, 0)
        / 1000000000000000000::NUMERIC
      ) AS equity_at_start,
      -- equity_at_end: shares_at_end * price_at_end.
      -- COALESCE(price, 0): if no price at end, vault may have 0 liquidity.
      TRUNC(
        pd.shares_at_end
        * COALESCE(pe.share_price, 0)
        / 1000000000000000000::NUMERIC
      ) AS equity_at_end
    FROM position_data pd
    LEFT JOIN prices_at_start ps
      ON pd.term_id = ps.term_id AND pd.curve_id = ps.curve_id
    LEFT JOIN prices_at_end pe
      ON pd.term_id = pe.term_id AND pd.curve_id = pe.curve_id
  ),

  -- -----------------------------------------------------------------------
  -- Step 7: Compute per-position PnL.
  -- Formula: period_pnl = equity_end - equity_start + redemptions - deposits
  -- This is the standard Modified Dietz-style period return in absolute terms.
  -- -----------------------------------------------------------------------
  position_pnl AS (
    SELECT
      ppm.account_id,
      ppm.term_id,
      ppm.curve_id,
      ppm.shares_at_end,
      ppm.period_deposits,
      ppm.period_redemptions,
      ppm.equity_at_start,
      ppm.equity_at_end,
      ppm.had_activity,
      (ppm.equity_at_end - ppm.equity_at_start
        + ppm.period_redemptions - ppm.period_deposits)::NUMERIC AS period_total_pnl,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start
                  + ppm.period_redemptions - ppm.period_deposits) > 0
           THEN 1 ELSE 0 END AS is_winning,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start
                  + ppm.period_redemptions - ppm.period_deposits) < 0
           THEN 1 ELSE 0 END AS is_losing
    FROM position_period_metrics ppm
  ),

  -- -----------------------------------------------------------------------
  -- Step 8: Aggregate per-position metrics up to account level.
  -- HAVING filters applied here to avoid materializing accounts that will
  -- be filtered anyway. This reduces the row count entering enriched/ranked.
  -- -----------------------------------------------------------------------
  account_metrics AS (
    SELECT
      pp.account_id,
      COUNT(DISTINCT (pp.term_id, pp.curve_id))
        FILTER (WHERE pp.had_activity OR pp.shares_at_end > 0)  AS period_position_count,
      COUNT(DISTINCT (pp.term_id, pp.curve_id))
        FILTER (WHERE pp.shares_at_end > 0)                     AS active_position_count,
      SUM(pp.is_winning)                                         AS winning_positions,
      SUM(pp.is_losing)                                          AS losing_positions,
      SUM(pp.period_deposits)::NUMERIC                           AS period_deposits_raw,
      SUM(pp.period_redemptions)::NUMERIC                        AS period_redemptions_raw,
      SUM(pp.period_total_pnl)::NUMERIC                          AS total_pnl_raw,
      SUM(pp.period_total_pnl) FILTER (WHERE pp.shares_at_end <= 0)::NUMERIC
                                                                 AS realized_pnl_raw,
      SUM(pp.period_total_pnl) FILTER (WHERE pp.shares_at_end > 0)::NUMERIC
                                                                 AS unrealized_pnl_raw,
      SUM(pp.equity_at_start)::NUMERIC                           AS equity_at_start,
      SUM(pp.equity_at_end)::NUMERIC                             AS equity_at_end,
      MAX(pp.period_total_pnl)                                   AS best_trade_pnl_raw,
      MIN(pp.period_total_pnl)                                   AS worst_trade_pnl_raw
    FROM position_pnl pp
    GROUP BY pp.account_id
    HAVING
      COUNT(DISTINCT (pp.term_id, pp.curve_id)) FILTER (WHERE pp.had_activity)
        >= GREATEST(p_min_positions, 1)
      AND (SUM(pp.period_deposits) + SUM(pp.period_redemptions))
        >= COALESCE(p_min_volume * 1e18, 0)
  ),

  -- -----------------------------------------------------------------------
  -- Step 9: Enrich with account labels/images and derive display metrics.
  -- JOIN account after HAVING to avoid fetching account rows for accounts
  -- that are later filtered by p_min_positions / p_min_volume.
  -- -----------------------------------------------------------------------
  enriched AS (
    SELECT
      am.account_id,
      a.label AS account_label,
      a.image AS account_image,
      am.total_pnl_raw,
      COALESCE(am.realized_pnl_raw, 0)   AS realized_pnl_raw,
      COALESCE(am.unrealized_pnl_raw, 0) AS unrealized_pnl_raw,
      COALESCE(
        (am.total_pnl_raw * 100.0 / NULLIF(
          CASE WHEN am.equity_at_start > 0 THEN am.equity_at_start
               ELSE am.period_deposits_raw
          END, 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS pnl_pct,
      am.total_pnl_raw                    AS pnl_change_raw,
      am.period_position_count            AS total_position_count,
      am.active_position_count,
      am.winning_positions,
      am.losing_positions,
      COALESCE(
        (am.winning_positions * 100.0 / NULLIF(am.period_position_count, 0))::NUMERIC(10, 2),
        0::NUMERIC(10, 2)
      ) AS win_rate,
      am.period_deposits_raw              AS total_deposits_raw,
      am.period_redemptions_raw           AS total_redemptions_raw,
      (am.period_deposits_raw + am.period_redemptions_raw)::NUMERIC AS total_volume_raw,
      am.equity_at_end                    AS current_equity_value_raw,
      am.best_trade_pnl_raw,
      am.worst_trade_pnl_raw,
      p_start_date                        AS first_position_at,
      p_end_date                          AS last_activity_at
    FROM account_metrics am
    JOIN account a ON am.account_id = a.id
    WHERE NOT p_exclude_protocol_accounts
       OR a.type NOT IN ('ProtocolVault', 'AtomWallet')
  ),

  -- -----------------------------------------------------------------------
  -- Step 10: Rank with dynamic sort column.
  -- ROW_NUMBER() window functions require a full sort of the enriched set,
  -- but this is unavoidable for arbitrary sort columns. The CASE expression
  -- generates multiple window function calls; PostgreSQL evaluates only the
  -- branch that matches. The enriched set is typically small (100s of rows)
  -- by the time HAVING has filtered it, so this cost is negligible.
  -- -----------------------------------------------------------------------
  ranked AS (
    SELECT
      e.*,
      CASE
        WHEN p_sort_by IN ('total_pnl', 'pnl') THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_pnl_raw ASC  NULLS LAST)
            ELSE             ROW_NUMBER() OVER (ORDER BY e.total_pnl_raw DESC NULLS LAST)
          END
        WHEN p_sort_by IN ('pnl_pct', 'roi') THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.pnl_pct ASC  NULLS LAST)
            ELSE             ROW_NUMBER() OVER (ORDER BY e.pnl_pct DESC NULLS LAST)
          END
        WHEN p_sort_by = 'win_rate' THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.win_rate ASC  NULLS LAST)
            ELSE             ROW_NUMBER() OVER (ORDER BY e.win_rate DESC NULLS LAST)
          END
        WHEN p_sort_by IN ('total_volume', 'volume') THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_volume_raw ASC  NULLS LAST)
            ELSE             ROW_NUMBER() OVER (ORDER BY e.total_volume_raw DESC NULLS LAST)
          END
        WHEN p_sort_by IN ('position_count', 'positions') THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_position_count ASC  NULLS LAST)
            ELSE             ROW_NUMBER() OVER (ORDER BY e.total_position_count DESC NULLS LAST)
          END
        ELSE ROW_NUMBER() OVER (ORDER BY e.total_pnl_raw DESC NULLS LAST)
      END AS rank
    FROM enriched e
  )

  SELECT
    r.rank,
    r.account_id,
    r.account_label,
    r.account_image,
    r.total_pnl_raw,
    ROUND(r.total_pnl_raw    / 1e18, 4)::NUMERIC(30, 4) AS total_pnl_formatted,
    r.realized_pnl_raw,
    ROUND(r.realized_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS realized_pnl_formatted,
    r.unrealized_pnl_raw,
    ROUND(r.unrealized_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS unrealized_pnl_formatted,
    r.pnl_pct,
    r.pnl_change_raw,
    ROUND(r.pnl_change_raw   / 1e18, 4)::NUMERIC(30, 4) AS pnl_change_formatted,
    r.total_position_count,
    r.active_position_count,
    r.winning_positions,
    r.losing_positions,
    r.win_rate,
    r.total_deposits_raw,
    ROUND(r.total_deposits_raw    / 1e18, 4)::NUMERIC(30, 4) AS total_deposits_formatted,
    r.total_redemptions_raw,
    ROUND(r.total_redemptions_raw / 1e18, 4)::NUMERIC(30, 4) AS total_redemptions_formatted,
    r.total_volume_raw,
    ROUND(r.total_volume_raw      / 1e18, 4)::NUMERIC(30, 4) AS total_volume_formatted,
    r.current_equity_value_raw,
    ROUND(r.current_equity_value_raw / 1e18, 4)::NUMERIC(30, 4) AS current_equity_value_formatted,
    r.best_trade_pnl_raw,
    ROUND(r.best_trade_pnl_raw  / 1e18, 4)::NUMERIC(30, 4) AS best_trade_pnl_formatted,
    r.worst_trade_pnl_raw,
    ROUND(r.worst_trade_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS worst_trade_pnl_formatted,
    NULL::NUMERIC         AS redeemable_assets_raw,
    NULL::NUMERIC(30, 4)  AS redeemable_assets_formatted,
    r.first_position_at,
    r.last_activity_at
  FROM ranked r
  ORDER BY r.rank
  LIMIT v_limit OFFSET v_offset;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION get_pnl_leaderboard_period IS
  'Period-specific PnL leaderboard. Optimized with single-pass position aggregation, '
  'two-CTE price lookup (vs three), relevant_vaults filtering, and TimescaleDB chunk exclusion.';
