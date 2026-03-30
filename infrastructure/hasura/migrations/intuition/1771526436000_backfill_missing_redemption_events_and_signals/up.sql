-- Backfill 64 missing rows in `event` and `signal` for existing redemptions.
--
-- Problem: 64 redemption records exist in the `redemption` table but have no
-- corresponding row in `event` or `signal`. The application indexer that
-- normally creates these rows failed to run (or was skipped) for this set of
-- transactions.
--
-- Root cause: These 64 redemptions span block range ~2361268-2377637.
-- Both `event` and `signal` are missing the same 64 rows, confirming the
-- indexer never processed the Redeemed events for these transactions.
--
-- Fix: Re-derive the rows from `redemption` using the same logic the
-- application applies:
--
--   event
--     id               = redemption.id  (= tx_hash || '-' || log_index)
--     type             = 'Redeemed'
--     atom_id          = redemption.term_id  (when term_id is an atom)
--     triple_id        = redemption.term_id  (when term_id is a triple)
--     redemption_id    = redemption.id
--     block_number     = redemption.block_number
--     created_at       = redemption.created_at
--     transaction_hash = redemption.transaction_hash
--     fee_transfer_id, protocol_fee_accrued_id, deposit_id = NULL
--
--   signal
--     id               = redemption.id
--     delta            = redemption.assets  (positive; assets returned to redeemer)
--     account_id       = redemption.sender_id
--     atom_id          = redemption.term_id  (when term_id is an atom)
--     triple_id        = redemption.term_id  (when term_id is a triple)
--     term_id          = redemption.term_id
--     curve_id         = redemption.curve_id
--     redemption_id    = redemption.id
--     block_number     = redemption.block_number
--     created_at       = redemption.created_at
--     transaction_hash = redemption.transaction_hash
--     deposit_id       = NULL
--
-- All field derivations verified against 7,783 existing redemption event/signal
-- rows in this database.
--
-- Breakdown of 64 missing records: 8 atom redemptions, 56 triple redemptions.
--
-- DEPENDS ON: (none — operates on pre-existing stable tables)
--
-- SCOPE:
--   Inserted:  64 rows into event
--   Inserted:  64 rows into signal

-- ────────────────────────────────────────────────────────────────────────────
-- Step 1: Insert missing rows into `event`
-- ────────────────────────────────────────────────────────────────────────────

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
    r.id                                          AS id,
    'Redeemed'                                    AS type,
    CASE WHEN a.term_id IS NOT NULL
         THEN r.term_id END                       AS atom_id,
    CASE WHEN t.term_id IS NOT NULL
         THEN r.term_id END                       AS triple_id,
    NULL                                          AS fee_transfer_id,
    NULL                                          AS deposit_id,
    r.id                                          AS redemption_id,
    r.block_number                                AS block_number,
    r.created_at                                  AS created_at,
    r.transaction_hash                            AS transaction_hash,
    NULL                                          AS protocol_fee_accrued_id
FROM redemption r
LEFT JOIN atom   a ON r.term_id = a.term_id
LEFT JOIN triple t ON r.term_id = t.term_id
WHERE NOT EXISTS (
    SELECT 1 FROM event e WHERE e.redemption_id = r.id
);

-- ────────────────────────────────────────────────────────────────────────────
-- Step 2: Insert missing rows into `signal`
-- ────────────────────────────────────────────────────────────────────────────

INSERT INTO signal (
    id,
    delta,
    account_id,
    atom_id,
    triple_id,
    term_id,
    curve_id,
    deposit_id,
    redemption_id,
    block_number,
    created_at,
    transaction_hash
)
SELECT
    r.id                                          AS id,
    r.assets                                      AS delta,
    r.sender_id                                   AS account_id,
    CASE WHEN a.term_id IS NOT NULL
         THEN r.term_id END                       AS atom_id,
    CASE WHEN t.term_id IS NOT NULL
         THEN r.term_id END                       AS triple_id,
    r.term_id                                     AS term_id,
    r.curve_id                                    AS curve_id,
    NULL                                          AS deposit_id,
    r.id                                          AS redemption_id,
    r.block_number                                AS block_number,
    r.created_at                                  AS created_at,
    r.transaction_hash                            AS transaction_hash
FROM redemption r
LEFT JOIN atom   a ON r.term_id = a.term_id
LEFT JOIN triple t ON r.term_id = t.term_id
WHERE NOT EXISTS (
    SELECT 1 FROM signal s WHERE s.redemption_id = r.id
);
