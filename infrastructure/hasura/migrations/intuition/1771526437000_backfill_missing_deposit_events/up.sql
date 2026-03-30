-- Backfill 48 missing rows in `event` for existing deposits.
--
-- Same root cause as 1771526436000 (redemptions): the indexer failed to emit
-- event rows for these transactions. Scattered across Nov 2025 - Feb 2026.
--
-- Idempotent: WHERE NOT EXISTS guard — safe to run on any environment.

INSERT INTO event (
    id,
    type,
    atom_id,
    triple_id,
    fee_transfer_id,
    deposit_id,
    redemption_id,
    block_number,
    created_at,
    transaction_hash,
    protocol_fee_accrued_id
)
SELECT
    d.id                                          AS id,
    'Deposited'                                   AS type,
    CASE WHEN a.term_id IS NOT NULL
         THEN d.term_id END                       AS atom_id,
    CASE WHEN t.term_id IS NOT NULL
         THEN d.term_id END                       AS triple_id,
    NULL                                          AS fee_transfer_id,
    d.id                                          AS deposit_id,
    NULL                                          AS redemption_id,
    d.block_number                                AS block_number,
    d.created_at                                  AS created_at,
    d.transaction_hash                            AS transaction_hash,
    NULL                                          AS protocol_fee_accrued_id
FROM deposit d
LEFT JOIN atom   a ON d.term_id = a.term_id
LEFT JOIN triple t ON d.term_id = t.term_id
WHERE NOT EXISTS (
    SELECT 1 FROM event e WHERE e.deposit_id = d.id
);
