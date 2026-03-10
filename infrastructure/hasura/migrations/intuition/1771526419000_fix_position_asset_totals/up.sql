-- Fix position.total_deposit_assets_after_total_fees and position.total_redeem_assets_for_receiver
-- These were being overwritten by the consumer's position upsert ON CONFLICT clause,
-- conflicting with the DB triggers that accumulate these values.
-- The consumer's ON CONFLICT no longer sets these fields, so triggers are the sole owner.
-- This migration recalculates correct values from the source deposit/redemption tables.

-- 1. Fix total_deposit_assets_after_total_fees
UPDATE position p
SET total_deposit_assets_after_total_fees = COALESCE(d.total, 0)
FROM (
    SELECT receiver_id AS account_id, term_id, curve_id,
           SUM(assets_after_fees) AS total
    FROM deposit
    GROUP BY receiver_id, term_id, curve_id
) d
WHERE p.account_id = d.account_id
  AND p.term_id = d.term_id
  AND p.curve_id = d.curve_id
  AND p.total_deposit_assets_after_total_fees != d.total;

-- 2. Fix total_redeem_assets_for_receiver
UPDATE position p
SET total_redeem_assets_for_receiver = COALESCE(r.total, 0)
FROM (
    SELECT receiver_id AS account_id, term_id, curve_id,
           SUM(assets) AS total
    FROM redemption
    GROUP BY receiver_id, term_id, curve_id
) r
WHERE p.account_id = r.account_id
  AND p.term_id = r.term_id
  AND p.curve_id = r.curve_id
  AND p.total_redeem_assets_for_receiver != r.total;

-- 3. Also fix the 9 vault position_count mismatches
SELECT fix_wrong_position_counts();
