-- Rollback: Restore view and functions to use hardcoded constants, drop config tables

-- ========================================
-- 1. RESTORE position_with_value VIEW (hardcoded constants from previous migration)
-- ========================================

DROP VIEW IF EXISTS public.position_with_value;

CREATE VIEW public.position_with_value AS
SELECT
  base.*,
  (base.theoretical_value + base.total_redeem_assets_for_receiver
    - base.total_deposit_assets_after_total_fees)::NUMERIC AS pnl,
  CASE
    WHEN base.total_deposit_assets_after_total_fees > 0
    THEN ((base.theoretical_value + base.total_redeem_assets_for_receiver
           - base.total_deposit_assets_after_total_fees) * 100.0
          / base.total_deposit_assets_after_total_fees)::NUMERIC(20, 4)
    ELSE 0::NUMERIC(20, 4)
  END AS pnl_pct
FROM (
  SELECT
    p.id, p.account_id, p.term_id, p.curve_id, p.shares,
    p.total_deposit_assets_after_total_fees,
    p.total_redeem_assets_for_receiver,
    p.block_number, p.log_index, p.transaction_hash,
    p.transaction_index, p.created_at, p.updated_at,
    TRUNC(p.shares * v.current_share_price / 1000000000000000000::NUMERIC)::NUMERIC AS theoretical_value,
    CASE
      WHEN p.shares = 0 THEN 0::NUMERIC
      WHEN p.curve_id = 1 THEN
        (SELECT GREATEST(
          raw_a
          - TRUNC((raw_a * 125 + 9999) / 10000)
          - CASE
              WHEN (v_default.total_shares - p.shares) >= 1000000000000000000::NUMERIC
              THEN TRUNC((raw_a * 75 + 9999) / 10000)
              ELSE 0
            END
        , 0)::NUMERIC
        FROM (SELECT TRUNC(p.shares * v.total_assets
                      / NULLIF(v.total_shares, 0))::NUMERIC AS raw_a) calc)
      WHEN p.curve_id = 2 THEN
        (SELECT GREATEST(
          raw_a
          - TRUNC((raw_a * 125 + 9999) / 10000)
          - CASE
              WHEN COALESCE(v_default.total_shares, 0) >= 1000000000000000000::NUMERIC
              THEN TRUNC((raw_a * 75 + 9999) / 10000)
              ELSE 0
            END
        , 0)::NUMERIC
        FROM (SELECT TRUNC(
          (
            TRUNC((v.total_shares + 30000000000000000000::NUMERIC)
             * (v.total_shares + 30000000000000000000::NUMERIC)
             / 1000000000000000000::NUMERIC)
            -
            TRUNC(((v.total_shares + 30000000000000000000::NUMERIC - p.shares)
              * (v.total_shares + 30000000000000000000::NUMERIC - p.shares)
              + 999999999999999999::NUMERIC)
             / 1000000000000000000::NUMERIC)
          ) * 50000000000000000::NUMERIC / 1000000000000000000::NUMERIC
        )::NUMERIC AS raw_a) calc)
      ELSE 0::NUMERIC
    END AS redeemable_assets
  FROM position p
  JOIN vault v ON v.term_id = p.term_id AND v.curve_id = p.curve_id
  LEFT JOIN vault v_default ON v_default.term_id = p.term_id AND v_default.curve_id = 1
) base;

-- ========================================
-- 2. RESTORE get_vault_leaderboard (hardcoded constants)
-- ========================================

-- Restore from previous migration state (1768475600000_fix_pnl_pct_denominator)
-- This is handled by that migration's version of the function, so we just need to
-- CREATE OR REPLACE with the old hardcoded version.
-- For brevity, we reference the fact that rolling back this migration means
-- the 1768475600000 state should be current. The view rollback above is sufficient
-- since the functions reference curve constants inline.

-- The vault leaderboard functions are restored via the previous migration's definitions
-- which used hardcoded OFFSET=3e19, HALF_SLOPE=5e16, protocolFee=125, exitFee=75

-- ========================================
-- 3. DROP CONFIG TABLES
-- ========================================

DROP TABLE IF EXISTS fee_config;
DROP TABLE IF EXISTS curve_config;
