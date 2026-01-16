-- Backfill position_change from existing deposit/redemption records.
-- Run once after migration if needed.

INSERT INTO position_change (
  created_at,
  account_id,
  term_id,
  curve_id,
  shares_delta,
  assets_in,
  assets_out,
  event_type,
  event_id,
  block_number,
  transaction_hash,
  log_index
)
SELECT
  d.created_at,
  d.receiver_id AS account_id,
  d.term_id,
  d.curve_id,
  d.shares AS shares_delta,
  d.assets_after_fees AS assets_in,
  0 AS assets_out,
  'deposit' AS event_type,
  d.id AS event_id,
  d.block_number,
  d.transaction_hash,
  d.log_index
FROM deposit d

UNION ALL

SELECT
  r.created_at,
  r.sender_id AS account_id,
  r.term_id,
  r.curve_id,
  -r.shares AS shares_delta,
  0 AS assets_in,
  r.assets AS assets_out,
  'redemption' AS event_type,
  r.id AS event_id,
  r.block_number,
  r.transaction_hash,
  r.log_index
FROM redemption r

ORDER BY created_at, log_index;
