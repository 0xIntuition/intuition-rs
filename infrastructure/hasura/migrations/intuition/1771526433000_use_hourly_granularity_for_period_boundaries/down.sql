-- Revert to daily-granularity period boundaries (restore from migration 1771526432000)

DROP INDEX IF EXISTS idx_pch_hourly_account_term_bucket;

-- Restore get_pnl_leaderboard_period from 1771526432000
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
  p_term_id TEXT DEFAULT NULL,
  p_min_deposit NUMERIC DEFAULT 0
)
RETURNS SETOF pnl_leaderboard_entry AS $$
DECLARE
  v_start_timestamp      BIGINT;
  v_end_timestamp        BIGINT;
  v_bucket_start         TIMESTAMPTZ;
  v_bucket_end           TIMESTAMPTZ;
  v_limit                INTEGER;
  v_offset               INTEGER;
  v_cagg_cutoff          TIMESTAMPTZ;
BEGIN
  IF p_start_date IS NULL OR p_end_date IS NULL THEN
    RAISE EXCEPTION 'p_start_date and p_end_date are required';
  END IF;

  IF p_start_date >= p_end_date THEN
    RAISE EXCEPTION 'p_start_date must be before p_end_date';
  END IF;

  v_limit  := LEAST(GREATEST(COALESCE(p_limit, 100), 1), 10000);
  v_offset := GREATEST(COALESCE(p_offset, 0), 0);

  v_start_timestamp := EXTRACT(EPOCH FROM p_start_date)::BIGINT;
  v_end_timestamp   := EXTRACT(EPOCH FROM p_end_date)::BIGINT;

  v_bucket_start := date_trunc('day', p_start_date);
  v_bucket_end   := date_trunc('day', p_end_date);

  v_cagg_cutoff := date_trunc('hour', NOW());

  SET LOCAL work_mem = '64MB';

  CREATE TEMP TABLE _tmp_active_accounts ON COMMIT DROP AS
  SELECT DISTINCT pc.account_id
  FROM position_change_daily pc
  WHERE pc.bucket >= v_bucket_start
    AND pc.bucket <= v_bucket_end
    AND (p_term_id IS NULL OR pc.term_id = p_term_id)
    AND (NOT p_exclude_protocol_accounts
         OR pc.account_id NOT IN (
           SELECT id FROM account
           WHERE type IN ('ProtocolVault', 'AtomWallet')
         ));

  ANALYZE _tmp_active_accounts;

  CREATE TEMP TABLE _tmp_position_data ON COMMIT DROP AS
  SELECT
    snap_end.account_id,
    snap_end.term_id,
    snap_end.curve_id,
    COALESCE(snap_start.cumulative_shares, 0)::NUMERIC(78,0)     AS shares_at_start,
    snap_end.cumulative_shares::NUMERIC(78,0)                    AS shares_at_end,
    (snap_end.cumulative_assets_in  - COALESCE(snap_start.cumulative_assets_in,  0))::NUMERIC AS period_deposits,
    (snap_end.cumulative_assets_out - COALESCE(snap_start.cumulative_assets_out, 0))::NUMERIC AS period_redemptions,
    ((snap_end.cumulative_assets_in  - COALESCE(snap_start.cumulative_assets_in,  0)) > 0
     OR
     (snap_end.cumulative_assets_out - COALESCE(snap_start.cumulative_assets_out, 0)) > 0
    ) AS had_activity,
    snap_end.cumulative_assets_in::NUMERIC                       AS cumulative_deposits,
    (snap_end.cumulative_shares_in  - COALESCE(snap_start.cumulative_shares_in,  0))::NUMERIC AS shares_acquired_in_period,
    (snap_end.cumulative_shares_out - COALESCE(snap_start.cumulative_shares_out, 0))::NUMERIC AS shares_redeemed_in_period
  FROM (
    SELECT DISTINCT ON (pch.account_id, pch.term_id, pch.curve_id)
      pch.account_id, pch.term_id, pch.curve_id,
      pch.cumulative_shares,
      pch.cumulative_assets_in,
      pch.cumulative_assets_out,
      pch.cumulative_shares_in,
      pch.cumulative_shares_out
    FROM _tmp_active_accounts aa
    JOIN position_cumulative_hourly pch ON pch.account_id = aa.account_id
    WHERE pch.bucket <= v_bucket_end
      AND (p_term_id IS NULL OR pch.term_id = p_term_id)
    ORDER BY pch.account_id, pch.term_id, pch.curve_id, pch.bucket DESC
  ) snap_end
  LEFT JOIN LATERAL (
    SELECT
      cumulative_shares,
      cumulative_assets_in,
      cumulative_assets_out,
      cumulative_shares_in,
      cumulative_shares_out
    FROM position_cumulative_hourly pch
    WHERE pch.account_id = snap_end.account_id
      AND pch.term_id    = snap_end.term_id
      AND pch.curve_id   = snap_end.curve_id
      AND pch.bucket     < v_bucket_start
    ORDER BY pch.bucket DESC
    LIMIT 1
  ) snap_start ON true;

  ANALYZE _tmp_position_data;

  CREATE TEMP TABLE _tmp_prices_at_start ON COMMIT DROP AS
  SELECT DISTINCT ON (c.term_id, c.curve_id)
    c.term_id, c.curve_id, c.last_share_price AS share_price
  FROM share_price_change_stats_hourly c
  INNER JOIN (
    SELECT DISTINCT term_id, curve_id FROM _tmp_position_data
    WHERE COALESCE(shares_at_start, 0) > 0
  ) pd ON c.term_id = pd.term_id AND c.curve_id = pd.curve_id
  WHERE c.bucket <= date_trunc('hour', p_start_date)
  ORDER BY c.term_id, c.curve_id, c.bucket DESC;

  CREATE TEMP TABLE _tmp_prices_at_end ON COMMIT DROP AS
  SELECT DISTINCT ON (c.term_id, c.curve_id)
    c.term_id, c.curve_id, c.last_share_price AS share_price
  FROM share_price_change_stats_hourly c
  INNER JOIN (
    SELECT DISTINCT term_id, curve_id FROM _tmp_position_data
    WHERE shares_at_end > 0
  ) pd ON c.term_id = pd.term_id AND c.curve_id = pd.curve_id
  WHERE c.bucket <= date_trunc('hour', p_end_date)
  ORDER BY c.term_id, c.curve_id, c.bucket DESC;

  IF p_end_date > v_cagg_cutoff THEN
    WITH raw_current_hour AS (
      SELECT DISTINCT ON (spc.term_id, spc.curve_id)
        spc.term_id, spc.curve_id, spc.share_price
      FROM share_price_change spc
      WHERE spc.block_timestamp > EXTRACT(EPOCH FROM v_cagg_cutoff)::BIGINT
        AND spc.block_timestamp <= v_end_timestamp
        AND EXISTS (
          SELECT 1 FROM _tmp_prices_at_end pe
          WHERE pe.term_id = spc.term_id AND pe.curve_id = spc.curve_id
        )
      ORDER BY spc.term_id, spc.curve_id, spc.block_timestamp DESC, spc.log_index DESC
    )
    UPDATE _tmp_prices_at_end pe
    SET share_price = rch.share_price
    FROM raw_current_hour rch
    WHERE pe.term_id = rch.term_id AND pe.curve_id = rch.curve_id;

    INSERT INTO _tmp_prices_at_end (term_id, curve_id, share_price)
    SELECT DISTINCT ON (spc.term_id, spc.curve_id)
      spc.term_id, spc.curve_id, spc.share_price
    FROM share_price_change spc
    INNER JOIN (
      SELECT DISTINCT term_id, curve_id FROM _tmp_position_data
      WHERE shares_at_end > 0
    ) pd ON spc.term_id = pd.term_id AND spc.curve_id = pd.curve_id
    WHERE spc.block_timestamp <= v_end_timestamp
      AND NOT EXISTS (
        SELECT 1 FROM _tmp_prices_at_end pe
        WHERE pe.term_id = spc.term_id AND pe.curve_id = spc.curve_id
      )
    ORDER BY spc.term_id, spc.curve_id, spc.block_timestamp DESC, spc.log_index DESC;
  END IF;

  CREATE TEMP TABLE _tmp_vault_state_at_start ON COMMIT DROP AS
  SELECT DISTINCT ON (spc.term_id, spc.curve_id)
    spc.term_id,
    spc.curve_id,
    spc.total_shares AS vault_total_shares
  FROM share_price_change spc
  INNER JOIN (
    SELECT DISTINCT term_id, curve_id
    FROM _tmp_position_data
    WHERE COALESCE(shares_at_start, 0) > 0
      AND curve_id = 2
  ) pd ON spc.term_id = pd.term_id AND spc.curve_id = pd.curve_id
  WHERE spc.block_timestamp <= v_start_timestamp
  ORDER BY spc.term_id, spc.curve_id, spc.block_timestamp DESC, spc.log_index DESC;

  CREATE TEMP TABLE _tmp_vault_state_at_end ON COMMIT DROP AS
  SELECT DISTINCT ON (spc.term_id, spc.curve_id)
    spc.term_id,
    spc.curve_id,
    spc.total_shares AS vault_total_shares
  FROM share_price_change spc
  INNER JOIN (
    SELECT DISTINCT term_id, curve_id
    FROM _tmp_position_data
    WHERE shares_at_end > 0
      AND curve_id = 2
  ) pd ON spc.term_id = pd.term_id AND spc.curve_id = pd.curve_id
  WHERE spc.block_timestamp <= v_end_timestamp
  ORDER BY spc.term_id, spc.curve_id, spc.block_timestamp DESC, spc.log_index DESC;

  RETURN QUERY
  WITH
  position_period_metrics AS (
    SELECT
      pd.account_id,
      pd.term_id,
      pd.curve_id,
      COALESCE(pd.shares_at_start, 0)           AS shares_at_start,
      pd.shares_at_end                          AS shares_at_end,
      COALESCE(pd.period_deposits, 0)           AS period_deposits,
      COALESCE(pd.period_redemptions, 0)        AS period_redemptions,
      pd.had_activity,
      COALESCE(pd.cumulative_deposits, 0)       AS cumulative_deposits,
      COALESCE(pd.shares_acquired_in_period, 0) AS shares_acquired_in_period,
      COALESCE(pd.shares_redeemed_in_period, 0) AS shares_redeemed_in_period,
      CASE
        WHEN pd.curve_id = 2 AND COALESCE(pd.shares_at_start, 0) > 0 THEN
          TRUNC(
            (
              TRUNC(
                (vs.vault_total_shares + cc."offset")
                * (vs.vault_total_shares + cc."offset")
                / 1000000000000000000::NUMERIC
              )
              -
              TRUNC(
                (
                  (vs.vault_total_shares + cc."offset" - pd.shares_at_start)
                  * (vs.vault_total_shares + cc."offset" - pd.shares_at_start)
                  + 999999999999999999::NUMERIC
                )
                / 1000000000000000000::NUMERIC
              )
            ) * cc.half_slope / 1000000000000000000::NUMERIC
          )
        ELSE
          TRUNC(
            COALESCE(pd.shares_at_start, 0)
            * COALESCE(ps.share_price, 0)
            / 1000000000000000000::NUMERIC
          )
      END AS equity_at_start,
      CASE
        WHEN pd.curve_id = 2 AND pd.shares_at_end > 0 THEN
          TRUNC(
            (
              TRUNC(
                (ve.vault_total_shares + cc."offset")
                * (ve.vault_total_shares + cc."offset")
                / 1000000000000000000::NUMERIC
              )
              -
              TRUNC(
                (
                  (ve.vault_total_shares + cc."offset" - pd.shares_at_end)
                  * (ve.vault_total_shares + cc."offset" - pd.shares_at_end)
                  + 999999999999999999::NUMERIC
                )
                / 1000000000000000000::NUMERIC
              )
            ) * cc.half_slope / 1000000000000000000::NUMERIC
          )
        ELSE
          TRUNC(
            pd.shares_at_end
            * COALESCE(pe.share_price, 0)
            / 1000000000000000000::NUMERIC
          )
      END AS equity_at_end
    FROM _tmp_position_data pd
    LEFT JOIN _tmp_prices_at_start ps
      ON pd.term_id = ps.term_id AND pd.curve_id = ps.curve_id
    LEFT JOIN _tmp_prices_at_end pe
      ON pd.term_id = pe.term_id AND pd.curve_id = pe.curve_id
    LEFT JOIN curve_config cc
      ON pd.curve_id = cc.curve_id
    LEFT JOIN _tmp_vault_state_at_start vs
      ON pd.term_id = vs.term_id AND pd.curve_id = vs.curve_id
    LEFT JOIN _tmp_vault_state_at_end ve
      ON pd.term_id = ve.term_id AND pd.curve_id = ve.curve_id
  ),

  position_pnl AS (
    SELECT
      ppm.account_id,
      ppm.term_id,
      ppm.curve_id,
      ppm.shares_at_start,
      ppm.shares_at_end,
      ppm.period_deposits,
      ppm.period_redemptions,
      ppm.equity_at_start,
      ppm.equity_at_end,
      ppm.had_activity,
      ppm.cumulative_deposits,
      ppm.shares_acquired_in_period,
      ppm.shares_redeemed_in_period,
      (ppm.equity_at_end - ppm.equity_at_start
        + ppm.period_redemptions - ppm.period_deposits)::NUMERIC AS period_total_pnl,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start
                  + ppm.period_redemptions - ppm.period_deposits) > 0
           THEN 1 ELSE 0 END AS is_winning,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start
                  + ppm.period_redemptions - ppm.period_deposits) < 0
           THEN 1 ELSE 0 END AS is_losing,
      CASE
        WHEN ppm.shares_at_end <= 0 THEN
          (ppm.equity_at_end - ppm.equity_at_start
            + ppm.period_redemptions - ppm.period_deposits)::NUMERIC
        WHEN ppm.period_redemptions <= 0 THEN 0::NUMERIC
        ELSE (ppm.period_redemptions - TRUNC(
          (ppm.equity_at_start + ppm.period_deposits) * ppm.shares_redeemed_in_period
          / NULLIF(ppm.shares_at_start + ppm.shares_acquired_in_period, 0)
        ))::NUMERIC
      END AS realized_pnl
    FROM position_period_metrics ppm
  ),

  filtered_pnl AS (
    SELECT * FROM position_pnl pp
    WHERE p_min_deposit <= 0
       OR pp.cumulative_deposits >= p_min_deposit * 1e18
  ),

  account_metrics AS (
    SELECT
      fp.account_id,
      COUNT(DISTINCT (fp.term_id, fp.curve_id))
        FILTER (WHERE fp.had_activity)                           AS period_position_count,
      COUNT(DISTINCT (fp.term_id, fp.curve_id))
        FILTER (WHERE fp.shares_at_end > 0)                      AS active_position_count,
      SUM(fp.is_winning) FILTER (WHERE fp.had_activity)          AS winning_positions,
      SUM(fp.is_losing)  FILTER (WHERE fp.had_activity)          AS losing_positions,
      SUM(fp.period_deposits)::NUMERIC                           AS period_deposits_raw,
      SUM(fp.period_redemptions)::NUMERIC                        AS period_redemptions_raw,
      SUM(fp.period_total_pnl)::NUMERIC                          AS total_pnl_raw,
      SUM(fp.realized_pnl)::NUMERIC                              AS realized_pnl_raw,
      SUM(fp.period_total_pnl - fp.realized_pnl)::NUMERIC        AS unrealized_pnl_raw,
      SUM(fp.equity_at_start)::NUMERIC                           AS equity_at_start,
      SUM(fp.equity_at_end)::NUMERIC                             AS equity_at_end,
      MAX(fp.period_total_pnl)                                   AS best_trade_pnl_raw,
      MIN(fp.period_total_pnl)                                   AS worst_trade_pnl_raw,
      SUM(CASE
        WHEN fp.shares_at_end <= 0 THEN fp.equity_at_start + fp.period_deposits
        WHEN fp.period_redemptions <= 0 THEN 0
        ELSE TRUNC((fp.equity_at_start + fp.period_deposits) * fp.shares_redeemed_in_period
             / NULLIF(fp.shares_at_start + fp.shares_acquired_in_period, 0))
      END)::NUMERIC                                              AS denominator_closed,
      SUM(CASE
        WHEN fp.shares_at_end <= 0 THEN 0
        WHEN fp.period_redemptions <= 0 THEN fp.equity_at_start + fp.period_deposits
        ELSE TRUNC((fp.equity_at_start + fp.period_deposits) * fp.shares_at_end
             / NULLIF(fp.shares_at_start + fp.shares_acquired_in_period, 0))
      END)::NUMERIC                                              AS denominator_open
    FROM filtered_pnl fp
    GROUP BY fp.account_id
    HAVING
      COUNT(DISTINCT (fp.term_id, fp.curve_id)) FILTER (WHERE fp.had_activity)
        >= GREATEST(p_min_positions, 1)
      AND (SUM(fp.period_deposits) + SUM(fp.period_redemptions))
        >= COALESCE(p_min_volume * 1e18, 0)
  ),

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
          am.equity_at_start + am.period_deposits_raw
        , 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS pnl_pct,
      COALESCE(
        (COALESCE(am.realized_pnl_raw, 0) * 100.0 / NULLIF(am.denominator_closed, 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS realized_pnl_pct,
      COALESCE(
        (COALESCE(am.unrealized_pnl_raw, 0) * 100.0 / NULLIF(am.denominator_open, 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS unrealized_pnl_pct,
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
        WHEN p_sort_by = 'realized_pnl' THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.realized_pnl_raw ASC  NULLS LAST)
            ELSE             ROW_NUMBER() OVER (ORDER BY e.realized_pnl_raw DESC NULLS LAST)
          END
        WHEN p_sort_by = 'unrealized_pnl' THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.unrealized_pnl_raw ASC  NULLS LAST)
            ELSE             ROW_NUMBER() OVER (ORDER BY e.unrealized_pnl_raw DESC NULLS LAST)
          END
        WHEN p_sort_by = 'realized_pnl_pct' THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.realized_pnl_pct ASC  NULLS LAST)
            ELSE             ROW_NUMBER() OVER (ORDER BY e.realized_pnl_pct DESC NULLS LAST)
          END
        WHEN p_sort_by = 'unrealized_pnl_pct' THEN
          CASE p_sort_order
            WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.unrealized_pnl_pct ASC  NULLS LAST)
            ELSE             ROW_NUMBER() OVER (ORDER BY e.unrealized_pnl_pct DESC NULLS LAST)
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
    r.realized_pnl_pct,
    r.unrealized_pnl_pct,
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
$$ LANGUAGE plpgsql VOLATILE;

-- Restore get_pnl_leaderboard from 1771526432000
CREATE OR REPLACE FUNCTION get_pnl_leaderboard(
  p_limit INTEGER DEFAULT 100,
  p_offset INTEGER DEFAULT 0,
  p_time_filter TEXT DEFAULT 'all_time',
  p_start_time TIMESTAMPTZ DEFAULT NULL,
  p_end_time TIMESTAMPTZ DEFAULT NULL,
  p_sort_by TEXT DEFAULT 'total_pnl',
  p_sort_order TEXT DEFAULT 'DESC',
  p_exclude_protocol_accounts BOOLEAN DEFAULT TRUE,
  p_min_positions INTEGER DEFAULT 1,
  p_min_volume NUMERIC DEFAULT 0,
  p_term_id TEXT DEFAULT NULL
)
RETURNS SETOF pnl_leaderboard_entry AS $$
DECLARE
  v_start_time TIMESTAMPTZ;
  v_end_time TIMESTAMPTZ;
  v_bucket_start TIMESTAMPTZ;
  v_bucket_end TIMESTAMPTZ;
  v_limit INTEGER;
  v_offset INTEGER;
BEGIN
  v_limit := LEAST(GREATEST(COALESCE(p_limit, 100), 1), 10000);
  v_offset := GREATEST(COALESCE(p_offset, 0), 0);

  v_end_time := COALESCE(p_end_time, NOW());

  CASE p_time_filter
    WHEN '24h' THEN v_start_time := v_end_time - INTERVAL '24 hours';
    WHEN '7d' THEN v_start_time := v_end_time - INTERVAL '7 days';
    WHEN '30d' THEN v_start_time := v_end_time - INTERVAL '30 days';
    WHEN '90d' THEN v_start_time := v_end_time - INTERVAL '90 days';
    WHEN 'custom' THEN v_start_time := COALESCE(p_start_time, '1970-01-01'::TIMESTAMPTZ);
    ELSE v_start_time := '1970-01-01'::TIMESTAMPTZ;
  END CASE;

  v_bucket_start := date_trunc('day', v_start_time);
  v_bucket_end := date_trunc('day', v_end_time);

  RETURN QUERY
  WITH
  active_accounts AS (
    SELECT DISTINCT pc.account_id
    FROM position_change_daily pc
    WHERE pc.bucket >= v_bucket_start
      AND pc.bucket <= v_bucket_end
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
  ),
  period_pnl_change AS (
    SELECT
      pc.account_id,
      SUM(COALESCE(pc.assets_out_period, 0) - COALESCE(pc.assets_in_period, 0))::NUMERIC AS period_realized_change_raw
    FROM position_change_daily pc
    WHERE pc.bucket >= v_bucket_start
      AND pc.bucket <= v_bucket_end
      AND (p_term_id IS NULL OR pc.term_id = p_term_id)
    GROUP BY pc.account_id
  ),
  current_positions AS (
    SELECT
      p.account_id,
      p.term_id,
      p.curve_id,
      p.shares,
      p.total_deposit_assets_after_total_fees AS total_deposits_raw,
      p.total_redeem_assets_for_receiver AS total_redemptions_raw,
      p.created_at AS position_created_at,
      p.updated_at AS position_updated_at,
      v.current_share_price,
      CASE
        WHEN p.curve_id = 2 AND p.shares > 0 THEN
          TRUNC(
            (
              TRUNC(
                (v.total_shares + cc."offset")
                * (v.total_shares + cc."offset")
                / 1000000000000000000::NUMERIC
              )
              -
              TRUNC(
                (
                  (v.total_shares + cc."offset" - p.shares)
                  * (v.total_shares + cc."offset" - p.shares)
                  + 999999999999999999::NUMERIC
                )
                / 1000000000000000000::NUMERIC
              )
            ) * cc.half_slope / 1000000000000000000::NUMERIC
          )
        ELSE
          TRUNC(p.shares * v.current_share_price / 1000000000000000000::NUMERIC)
      END AS equity_value_raw,
      COALESCE(st.total_shares_acquired, 0) AS total_shares_acquired,
      COALESCE(st.total_shares_redeemed, 0) AS total_shares_redeemed
    FROM position p
    JOIN vault v ON p.term_id = v.term_id AND p.curve_id = v.curve_id
    LEFT JOIN curve_config cc ON p.curve_id = cc.curve_id
    LEFT JOIN LATERAL (
      SELECT
        SUM(pc.shares_in_period)::NUMERIC AS total_shares_acquired,
        SUM(pc.shares_out_period)::NUMERIC AS total_shares_redeemed
      FROM position_change_daily pc
      WHERE pc.account_id = p.account_id
        AND pc.term_id = p.term_id
        AND pc.curve_id = p.curve_id
    ) st ON TRUE
    WHERE (p_time_filter = 'all_time' OR p.account_id IN (SELECT account_id FROM active_accounts))
      AND (p_term_id IS NULL OR p.term_id = p_term_id)
  ),
  current_positions_valued AS (
    SELECT
      cp.*,
      (cp.equity_value_raw + cp.total_redemptions_raw - cp.total_deposits_raw)::NUMERIC AS position_pnl_raw,
      CASE
        WHEN cp.shares <= 0 THEN
          (cp.equity_value_raw + cp.total_redemptions_raw - cp.total_deposits_raw)::NUMERIC
        WHEN cp.total_redemptions_raw <= 0 THEN 0::NUMERIC
        ELSE (cp.total_redemptions_raw - TRUNC(
          cp.total_deposits_raw * COALESCE(cp.total_shares_redeemed, 0)
          / NULLIF(COALESCE(cp.total_shares_acquired, 0), 0)
        ))::NUMERIC
      END AS realized_pnl
    FROM current_positions cp
  ),
  account_metrics AS (
    SELECT
      cp.account_id,
      COUNT(DISTINCT (cp.term_id, cp.curve_id)) AS total_position_count,
      COUNT(DISTINCT (cp.term_id, cp.curve_id)) FILTER (WHERE cp.shares > 0) AS active_position_count,
      COUNT(*) FILTER (WHERE cp.position_pnl_raw > 0) AS winning_positions,
      COUNT(*) FILTER (WHERE cp.position_pnl_raw < 0) AS losing_positions,
      SUM(cp.total_deposits_raw)::NUMERIC AS total_deposits_raw,
      SUM(cp.total_redemptions_raw)::NUMERIC AS total_redemptions_raw,
      SUM(cp.equity_value_raw)::NUMERIC AS current_equity_value_raw,
      SUM(cp.position_pnl_raw)::NUMERIC AS total_pnl_raw,
      SUM(cp.realized_pnl)::NUMERIC AS realized_pnl_raw,
      SUM(cp.position_pnl_raw - cp.realized_pnl)::NUMERIC AS unrealized_pnl_raw,
      MAX(cp.position_pnl_raw) AS best_trade_pnl_raw,
      MIN(cp.position_pnl_raw) AS worst_trade_pnl_raw,
      MIN(cp.position_created_at) AS first_position_at,
      MAX(cp.position_updated_at) AS last_activity_at,
      SUM(CASE
        WHEN cp.shares <= 0 THEN cp.total_deposits_raw
        WHEN cp.total_redemptions_raw <= 0 THEN 0
        ELSE TRUNC(cp.total_deposits_raw * cp.total_shares_redeemed
             / NULLIF(cp.total_shares_acquired, 0))
      END)::NUMERIC AS denominator_closed,
      SUM(CASE
        WHEN cp.shares <= 0 THEN 0
        WHEN cp.total_redemptions_raw <= 0 THEN cp.total_deposits_raw
        ELSE TRUNC(cp.total_deposits_raw * cp.shares
             / NULLIF(cp.total_shares_acquired, 0))
      END)::NUMERIC AS denominator_open
    FROM current_positions_valued cp
    GROUP BY cp.account_id
    HAVING COUNT(DISTINCT (cp.term_id, cp.curve_id)) >= GREATEST(p_min_positions, 1)
      AND (SUM(cp.total_deposits_raw) + SUM(cp.total_redemptions_raw)) >= COALESCE(p_min_volume * 1e18, 0)
  ),
  enriched AS (
    SELECT
      am.account_id,
      a.label AS account_label,
      a.image AS account_image,
      am.total_pnl_raw,
      COALESCE(am.realized_pnl_raw, 0) AS realized_pnl_raw,
      COALESCE(am.unrealized_pnl_raw, 0) AS unrealized_pnl_raw,
      COALESCE(
        (am.total_pnl_raw * 100.0 / NULLIF(am.total_deposits_raw, 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS pnl_pct,
      COALESCE(
        (COALESCE(am.realized_pnl_raw, 0) * 100.0 / NULLIF(am.denominator_closed, 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS realized_pnl_pct,
      COALESCE(
        (COALESCE(am.unrealized_pnl_raw, 0) * 100.0 / NULLIF(am.denominator_open, 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS unrealized_pnl_pct,
      CASE
        WHEN p_time_filter = 'all_time' THEN am.total_pnl_raw
        ELSE COALESCE(ppc.period_realized_change_raw, 0)
      END AS pnl_change_raw,
      am.total_position_count,
      am.active_position_count,
      am.winning_positions,
      am.losing_positions,
      COALESCE(
        (am.winning_positions * 100.0 / NULLIF(am.total_position_count, 0))::NUMERIC(10, 2),
        0::NUMERIC(10, 2)
      ) AS win_rate,
      am.total_deposits_raw,
      am.total_redemptions_raw,
      (am.total_deposits_raw + am.total_redemptions_raw)::NUMERIC AS total_volume_raw,
      am.current_equity_value_raw,
      am.best_trade_pnl_raw,
      am.worst_trade_pnl_raw,
      am.first_position_at,
      am.last_activity_at
    FROM account_metrics am
    JOIN account a ON am.account_id = a.id
    LEFT JOIN period_pnl_change ppc ON am.account_id = ppc.account_id
    WHERE NOT p_exclude_protocol_accounts OR a.type NOT IN ('ProtocolVault', 'AtomWallet')
  ),
  ranked AS (
    SELECT e.*,
      CASE
        WHEN p_sort_by IN ('total_pnl', 'pnl') THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_pnl_raw ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_pnl_raw DESC NULLS LAST) END
        WHEN p_sort_by IN ('pnl_pct', 'roi') THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.pnl_pct ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.pnl_pct DESC NULLS LAST) END
        WHEN p_sort_by = 'win_rate' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.win_rate ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.win_rate DESC NULLS LAST) END
        WHEN p_sort_by IN ('total_volume', 'volume') THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_volume_raw ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_volume_raw DESC NULLS LAST) END
        WHEN p_sort_by IN ('position_count', 'positions') THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_position_count ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_position_count DESC NULLS LAST) END
        WHEN p_sort_by = 'newest' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.first_position_at ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.first_position_at DESC NULLS LAST) END
        WHEN p_sort_by = 'most_improved' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.pnl_change_raw ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.pnl_change_raw DESC NULLS LAST) END
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
    ROUND(r.total_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS total_pnl_formatted,
    r.realized_pnl_raw,
    ROUND(r.realized_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS realized_pnl_formatted,
    r.unrealized_pnl_raw,
    ROUND(r.unrealized_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS unrealized_pnl_formatted,
    r.pnl_pct,
    r.realized_pnl_pct,
    r.unrealized_pnl_pct,
    r.pnl_change_raw,
    ROUND(r.pnl_change_raw / 1e18, 4)::NUMERIC(30, 4) AS pnl_change_formatted,
    r.total_position_count,
    r.active_position_count,
    r.winning_positions,
    r.losing_positions,
    r.win_rate,
    r.total_deposits_raw,
    ROUND(r.total_deposits_raw / 1e18, 4)::NUMERIC(30, 4) AS total_deposits_formatted,
    r.total_redemptions_raw,
    ROUND(r.total_redemptions_raw / 1e18, 4)::NUMERIC(30, 4) AS total_redemptions_formatted,
    r.total_volume_raw,
    ROUND(r.total_volume_raw / 1e18, 4)::NUMERIC(30, 4) AS total_volume_formatted,
    r.current_equity_value_raw,
    ROUND(r.current_equity_value_raw / 1e18, 4)::NUMERIC(30, 4) AS current_equity_value_formatted,
    r.best_trade_pnl_raw,
    ROUND(r.best_trade_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS best_trade_pnl_formatted,
    r.worst_trade_pnl_raw,
    ROUND(r.worst_trade_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS worst_trade_pnl_formatted,
    NULL::NUMERIC AS redeemable_assets_raw,
    NULL::NUMERIC(30, 4) AS redeemable_assets_formatted,
    r.first_position_at,
    r.last_activity_at
  FROM ranked r
  ORDER BY r.rank
  LIMIT v_limit OFFSET v_offset;
END;
$$ LANGUAGE plpgsql VOLATILE;

-- Restore get_vault_leaderboard_period from 1771526432000
CREATE OR REPLACE FUNCTION get_vault_leaderboard_period(
  p_term_id TEXT,
  p_start_date TIMESTAMPTZ,
  p_end_date TIMESTAMPTZ,
  p_curve_id NUMERIC DEFAULT NULL,
  p_limit INTEGER DEFAULT 100,
  p_offset INTEGER DEFAULT 0,
  p_sort_by TEXT DEFAULT 'total_pnl',
  p_sort_order TEXT DEFAULT 'DESC'
)
RETURNS SETOF pnl_leaderboard_entry AS $$
DECLARE
  v_start_timestamp BIGINT;
  v_end_timestamp BIGINT;
  v_bucket_start TIMESTAMPTZ;
  v_bucket_end TIMESTAMPTZ;
  v_limit INTEGER;
  v_offset INTEGER;
BEGIN
  IF p_term_id IS NULL OR p_term_id = '' THEN
    RAISE EXCEPTION 'p_term_id is required';
  END IF;

  IF p_start_date IS NULL OR p_end_date IS NULL THEN
    RAISE EXCEPTION 'p_start_date and p_end_date are required';
  END IF;

  IF p_start_date >= p_end_date THEN
    RAISE EXCEPTION 'p_start_date must be before p_end_date';
  END IF;

  v_limit := LEAST(GREATEST(COALESCE(p_limit, 100), 1), 10000);
  v_offset := GREATEST(COALESCE(p_offset, 0), 0);

  v_start_timestamp := EXTRACT(EPOCH FROM p_start_date)::BIGINT;
  v_end_timestamp := EXTRACT(EPOCH FROM p_end_date)::BIGINT;

  v_bucket_start := date_trunc('day', p_start_date);
  v_bucket_end := date_trunc('day', p_end_date);

  RETURN QUERY
  WITH
  fc AS (SELECT * FROM fee_config LIMIT 1),

  active_accounts AS (
    SELECT DISTINCT pc.account_id
    FROM position_change_daily pc
    WHERE pc.bucket >= v_bucket_start
      AND pc.bucket <= v_bucket_end
      AND pc.term_id = p_term_id
      AND (p_curve_id IS NULL OR pc.curve_id = p_curve_id)
  ),

  prices_before_start AS (
    SELECT DISTINCT ON (term_id, curve_id)
      term_id, curve_id, share_price, total_assets, total_shares
    FROM share_price_change
    WHERE term_id = p_term_id
      AND (p_curve_id IS NULL OR curve_id = p_curve_id)
      AND block_timestamp <= v_start_timestamp
    ORDER BY term_id, curve_id, block_timestamp DESC, log_index DESC
  ),
  prices_earliest AS (
    SELECT DISTINCT ON (term_id, curve_id)
      term_id, curve_id, share_price, total_assets, total_shares
    FROM share_price_change
    WHERE term_id = p_term_id
      AND (p_curve_id IS NULL OR curve_id = p_curve_id)
    ORDER BY term_id, curve_id, block_timestamp ASC, log_index ASC
  ),

  vault_state_end AS (
    SELECT DISTINCT ON (term_id, curve_id)
      term_id, curve_id, share_price, total_assets, total_shares
    FROM share_price_change
    WHERE term_id = p_term_id
      AND (p_curve_id IS NULL OR curve_id = p_curve_id)
      AND block_timestamp <= v_end_timestamp
    ORDER BY term_id, curve_id, block_timestamp DESC, log_index DESC
  ),

  position_state_start AS (
    SELECT
      pc.account_id, pc.term_id, pc.curve_id,
      SUM(pc.shares_delta_period)::NUMERIC AS shares_at_start,
      SUM(pc.assets_in_period)::NUMERIC AS cumulative_deposits_before,
      SUM(pc.assets_out_period)::NUMERIC AS cumulative_redemptions_before
    FROM position_change_daily pc
    WHERE pc.bucket < v_bucket_start
      AND pc.term_id = p_term_id
      AND (p_curve_id IS NULL OR pc.curve_id = p_curve_id)
    GROUP BY pc.account_id, pc.term_id, pc.curve_id
  ),

  position_state_end AS (
    SELECT
      pc.account_id, pc.term_id, pc.curve_id,
      SUM(pc.shares_delta_period)::NUMERIC AS shares_at_end,
      SUM(pc.assets_in_period)::NUMERIC AS cumulative_deposits_through,
      SUM(pc.assets_out_period)::NUMERIC AS cumulative_redemptions_through
    FROM position_change_daily pc
    WHERE pc.bucket <= v_bucket_end
      AND pc.term_id = p_term_id
      AND (p_curve_id IS NULL OR pc.curve_id = p_curve_id)
    GROUP BY pc.account_id, pc.term_id, pc.curve_id
  ),

  period_activity AS (
    SELECT
      pc.account_id, pc.term_id, pc.curve_id,
      SUM(pc.assets_in_period)::NUMERIC AS period_deposits,
      SUM(pc.assets_out_period)::NUMERIC AS period_redemptions,
      SUM(pc.shares_delta_period)::NUMERIC AS period_shares_delta
    FROM position_change_daily pc
    WHERE pc.bucket >= v_bucket_start
      AND pc.bucket <= v_bucket_end
      AND pc.term_id = p_term_id
      AND (p_curve_id IS NULL OR pc.curve_id = p_curve_id)
    GROUP BY pc.account_id, pc.term_id, pc.curve_id
  ),

  position_period_metrics AS (
    SELECT
      COALESCE(pss.account_id, pse.account_id, pa.account_id) AS account_id,
      COALESCE(pss.term_id, pse.term_id, pa.term_id) AS term_id,
      COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) AS curve_id,
      COALESCE(pss.shares_at_start, 0) AS shares_at_start,
      COALESCE(pse.shares_at_end, 0) AS shares_at_end,
      COALESCE(pa.period_deposits, 0) AS period_deposits,
      COALESCE(pa.period_redemptions, 0) AS period_redemptions,
      COALESCE(pbs.share_price, pe_earliest.share_price) AS price_at_start,
      COALESCE(ve.share_price, 0) AS price_at_end,
      COALESCE(ve.total_assets, 0) AS vault_total_assets_end,
      COALESCE(ve.total_shares, 0) AS vault_total_shares_end,
      CASE
        WHEN COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) = 2
             AND COALESCE(pss.shares_at_start, 0) > 0 THEN
          TRUNC(
            (
              TRUNC(
                (COALESCE(pbs.total_shares, pe_earliest.total_shares, 0) + cc."offset")
                * (COALESCE(pbs.total_shares, pe_earliest.total_shares, 0) + cc."offset")
                / 1000000000000000000::NUMERIC
              )
              -
              TRUNC(
                (
                  (COALESCE(pbs.total_shares, pe_earliest.total_shares, 0) + cc."offset" - COALESCE(pss.shares_at_start, 0))
                  * (COALESCE(pbs.total_shares, pe_earliest.total_shares, 0) + cc."offset" - COALESCE(pss.shares_at_start, 0))
                  + 999999999999999999::NUMERIC
                )
                / 1000000000000000000::NUMERIC
              )
            ) * cc.half_slope / 1000000000000000000::NUMERIC
          )
        ELSE
          TRUNC(COALESCE(pss.shares_at_start, 0) * COALESCE(pbs.share_price, pe_earliest.share_price) / 1000000000000000000::NUMERIC)
      END AS equity_at_start,
      CASE
        WHEN COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) = 2
             AND COALESCE(pse.shares_at_end, 0) > 0 THEN
          TRUNC(
            (
              TRUNC((COALESCE(ve.total_shares, 0) + cc."offset") * (COALESCE(ve.total_shares, 0) + cc."offset") / 1000000000000000000::NUMERIC)
              -
              TRUNC(((COALESCE(ve.total_shares, 0) + cc."offset" - COALESCE(pse.shares_at_end, 0)) * (COALESCE(ve.total_shares, 0) + cc."offset" - COALESCE(pse.shares_at_end, 0)) + 999999999999999999::NUMERIC) / 1000000000000000000::NUMERIC)
            )
            * cc.half_slope / 1000000000000000000::NUMERIC
          )
        ELSE
          TRUNC(COALESCE(pse.shares_at_end, 0) * COALESCE(ve.share_price, 0) / 1000000000000000000::NUMERIC)
      END AS equity_at_end,
      CASE WHEN pa.account_id IS NOT NULL THEN TRUE ELSE FALSE END AS had_activity
    FROM position_state_start pss
    FULL OUTER JOIN position_state_end pse
      ON pss.account_id = pse.account_id AND pss.term_id = pse.term_id AND pss.curve_id = pse.curve_id
    FULL OUTER JOIN period_activity pa
      ON COALESCE(pss.account_id, pse.account_id) = pa.account_id
      AND COALESCE(pss.term_id, pse.term_id) = pa.term_id
      AND COALESCE(pss.curve_id, pse.curve_id) = pa.curve_id
    LEFT JOIN prices_before_start pbs
      ON COALESCE(pss.term_id, pse.term_id, pa.term_id) = pbs.term_id
      AND COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) = pbs.curve_id
    LEFT JOIN prices_earliest pe_earliest
      ON COALESCE(pss.term_id, pse.term_id, pa.term_id) = pe_earliest.term_id
      AND COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) = pe_earliest.curve_id
    LEFT JOIN vault_state_end ve
      ON COALESCE(pss.term_id, pse.term_id, pa.term_id) = ve.term_id
      AND COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) = ve.curve_id
    LEFT JOIN curve_config cc
      ON COALESCE(pss.curve_id, pse.curve_id, pa.curve_id) = cc.curve_id
    WHERE COALESCE(pss.account_id, pse.account_id, pa.account_id) IN (SELECT account_id FROM active_accounts)
  ),

  position_pnl AS (
    SELECT
      ppm.*,
      (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits)::NUMERIC AS period_total_pnl,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits) > 0 THEN 1 ELSE 0 END AS is_winning,
      CASE WHEN (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits) < 0 THEN 1 ELSE 0 END AS is_losing,
      CASE
        WHEN ppm.shares_at_end <= 0 THEN
          (ppm.equity_at_end - ppm.equity_at_start + ppm.period_redemptions - ppm.period_deposits)::NUMERIC
        WHEN ppm.period_redemptions <= 0 THEN 0::NUMERIC
        ELSE (ppm.period_redemptions - TRUNC(
          (ppm.equity_at_start + ppm.period_deposits) * ppm.period_redemptions
          / NULLIF(ppm.period_redemptions + ppm.equity_at_end, 0)
        ))::NUMERIC
      END AS realized_pnl,
      CASE
        WHEN ppm.shares_at_end = 0 THEN 0::NUMERIC
        WHEN cc.curve_type = 'linear' THEN
          CASE WHEN ppm.vault_total_shares_end > 0
            THEN TRUNC(ppm.shares_at_end * ppm.vault_total_assets_end / ppm.vault_total_shares_end)
            ELSE 0::NUMERIC
          END
        WHEN cc.curve_type = 'offset_progressive' THEN
          TRUNC(
            (
              TRUNC((ppm.vault_total_shares_end + cc."offset") * (ppm.vault_total_shares_end + cc."offset") / 1000000000000000000::NUMERIC)
              -
              TRUNC(((ppm.vault_total_shares_end + cc."offset" - ppm.shares_at_end) * (ppm.vault_total_shares_end + cc."offset" - ppm.shares_at_end) + 999999999999999999::NUMERIC) / 1000000000000000000::NUMERIC)
            )
            * cc.half_slope / 1000000000000000000::NUMERIC
          )
        ELSE 0::NUMERIC
      END AS raw_assets_from_curve
    FROM position_period_metrics ppm
    LEFT JOIN curve_config cc ON ppm.curve_id = cc.curve_id
  ),

  position_with_fees AS (
    SELECT
      pp.*,
      TRUNC((pp.raw_assets_from_curve * f.protocol_fee + f.fee_denominator - 1) / f.fee_denominator)::NUMERIC AS protocol_fee,
      TRUNC((pp.raw_assets_from_curve * f.exit_fee + f.fee_denominator - 1) / f.fee_denominator)::NUMERIC AS exit_fee
    FROM position_pnl pp
    CROSS JOIN fc f
  ),

  position_final AS (
    SELECT
      pwf.*,
      GREATEST(pwf.raw_assets_from_curve - pwf.protocol_fee - pwf.exit_fee, 0)::NUMERIC AS redeemable_assets_raw
    FROM position_with_fees pwf
  ),

  account_metrics AS (
    SELECT
      pf.account_id,
      COUNT(DISTINCT (pf.term_id, pf.curve_id)) FILTER (WHERE pf.had_activity OR pf.shares_at_end > 0) AS period_position_count,
      COUNT(DISTINCT (pf.term_id, pf.curve_id)) FILTER (WHERE pf.shares_at_end > 0) AS active_position_count,
      SUM(pf.is_winning) AS winning_positions,
      SUM(pf.is_losing) AS losing_positions,
      SUM(pf.period_deposits)::NUMERIC AS period_deposits_raw,
      SUM(pf.period_redemptions)::NUMERIC AS period_redemptions_raw,
      SUM(pf.period_total_pnl)::NUMERIC AS total_pnl_raw,
      SUM(pf.realized_pnl)::NUMERIC AS realized_pnl_raw,
      SUM(pf.period_total_pnl - pf.realized_pnl)::NUMERIC AS unrealized_pnl_raw,
      SUM(pf.equity_at_start)::NUMERIC AS equity_at_start,
      SUM(pf.equity_at_end)::NUMERIC AS equity_at_end,
      MAX(pf.period_total_pnl) AS best_trade_pnl_raw,
      MIN(pf.period_total_pnl) AS worst_trade_pnl_raw,
      SUM(pf.redeemable_assets_raw) FILTER (WHERE pf.shares_at_end > 0)::NUMERIC AS redeemable_assets_raw,
      SUM(CASE
        WHEN pf.shares_at_end <= 0 THEN pf.equity_at_start + pf.period_deposits
        WHEN pf.period_redemptions <= 0 THEN 0
        ELSE TRUNC((pf.equity_at_start + pf.period_deposits) * pf.period_redemptions
             / NULLIF(pf.period_redemptions + pf.equity_at_end, 0))
      END)::NUMERIC AS denominator_closed,
      SUM(CASE
        WHEN pf.shares_at_end <= 0 THEN 0
        WHEN pf.period_redemptions <= 0 THEN pf.equity_at_start + pf.period_deposits
        ELSE (pf.equity_at_start + pf.period_deposits) - TRUNC((pf.equity_at_start + pf.period_deposits) * pf.period_redemptions
             / NULLIF(pf.period_redemptions + pf.equity_at_end, 0))
      END)::NUMERIC AS denominator_open
    FROM position_final pf
    GROUP BY pf.account_id
  ),

  enriched AS (
    SELECT
      am.account_id,
      a.label AS account_label,
      a.image AS account_image,
      am.total_pnl_raw,
      COALESCE(am.realized_pnl_raw, 0) AS realized_pnl_raw,
      COALESCE(am.unrealized_pnl_raw, 0) AS unrealized_pnl_raw,
      COALESCE(
        (am.total_pnl_raw * 100.0 / NULLIF(
          am.equity_at_start + am.period_deposits_raw
        , 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS pnl_pct,
      COALESCE(
        (COALESCE(am.realized_pnl_raw, 0) * 100.0 / NULLIF(am.denominator_closed, 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS realized_pnl_pct,
      COALESCE(
        (COALESCE(am.unrealized_pnl_raw, 0) * 100.0 / NULLIF(am.denominator_open, 0))::NUMERIC(20, 4),
        0::NUMERIC(20, 4)
      ) AS unrealized_pnl_pct,
      am.total_pnl_raw AS pnl_change_raw,
      am.period_position_count AS total_position_count,
      am.active_position_count,
      am.winning_positions,
      am.losing_positions,
      COALESCE(
        (am.winning_positions * 100.0 / NULLIF(am.period_position_count, 0))::NUMERIC(10, 2),
        0::NUMERIC(10, 2)
      ) AS win_rate,
      am.period_deposits_raw AS total_deposits_raw,
      am.period_redemptions_raw AS total_redemptions_raw,
      (am.period_deposits_raw + am.period_redemptions_raw)::NUMERIC AS total_volume_raw,
      am.equity_at_end AS current_equity_value_raw,
      am.best_trade_pnl_raw,
      am.worst_trade_pnl_raw,
      COALESCE(am.redeemable_assets_raw, 0) AS redeemable_assets_raw,
      p_start_date AS first_position_at,
      p_end_date AS last_activity_at
    FROM account_metrics am
    JOIN account a ON am.account_id = a.id
    WHERE a.type NOT IN ('ProtocolVault', 'AtomWallet')
  ),

  ranked AS (
    SELECT
      e.*,
      CASE p_sort_by
        WHEN 'total_pnl' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_pnl_raw ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_pnl_raw DESC NULLS LAST) END
        WHEN 'pnl_pct' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.pnl_pct ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.pnl_pct DESC NULLS LAST) END
        WHEN 'win_rate' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.win_rate ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.win_rate DESC NULLS LAST) END
        WHEN 'total_volume' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_volume_raw ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_volume_raw DESC NULLS LAST) END
        WHEN 'position_count' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.total_position_count ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.total_position_count DESC NULLS LAST) END
        WHEN 'redeemable_assets' THEN CASE p_sort_order WHEN 'ASC' THEN ROW_NUMBER() OVER (ORDER BY e.redeemable_assets_raw ASC NULLS LAST) ELSE ROW_NUMBER() OVER (ORDER BY e.redeemable_assets_raw DESC NULLS LAST) END
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
    ROUND(r.total_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS total_pnl_formatted,
    r.realized_pnl_raw,
    ROUND(r.realized_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS realized_pnl_formatted,
    r.unrealized_pnl_raw,
    ROUND(r.unrealized_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS unrealized_pnl_formatted,
    r.pnl_pct,
    r.realized_pnl_pct,
    r.unrealized_pnl_pct,
    r.pnl_change_raw,
    ROUND(r.pnl_change_raw / 1e18, 4)::NUMERIC(30, 4) AS pnl_change_formatted,
    r.total_position_count,
    r.active_position_count,
    r.winning_positions,
    r.losing_positions,
    r.win_rate,
    r.total_deposits_raw,
    ROUND(r.total_deposits_raw / 1e18, 4)::NUMERIC(30, 4) AS total_deposits_formatted,
    r.total_redemptions_raw,
    ROUND(r.total_redemptions_raw / 1e18, 4)::NUMERIC(30, 4) AS total_redemptions_formatted,
    r.total_volume_raw,
    ROUND(r.total_volume_raw / 1e18, 4)::NUMERIC(30, 4) AS total_volume_formatted,
    r.current_equity_value_raw,
    ROUND(r.current_equity_value_raw / 1e18, 4)::NUMERIC(30, 4) AS current_equity_value_formatted,
    r.best_trade_pnl_raw,
    ROUND(r.best_trade_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS best_trade_pnl_formatted,
    r.worst_trade_pnl_raw,
    ROUND(r.worst_trade_pnl_raw / 1e18, 4)::NUMERIC(30, 4) AS worst_trade_pnl_formatted,
    r.redeemable_assets_raw,
    ROUND(r.redeemable_assets_raw / 1e18, 4)::NUMERIC(30, 4) AS redeemable_assets_formatted,
    r.first_position_at,
    r.last_activity_at
  FROM ranked r
  ORDER BY r.rank
  LIMIT v_limit OFFSET v_offset;
END;
$$ LANGUAGE plpgsql VOLATILE;
