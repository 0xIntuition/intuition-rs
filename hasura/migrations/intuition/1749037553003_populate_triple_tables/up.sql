-- Migration to populate triple_term and triple_vault tables with historical data

-- First, populate triple_vault table for curve_id = 1 (default curve)
-- For each triple, sum the vault data for both term_id and counter_term_id
INSERT INTO triple_vault (term_id, counter_term_id, curve_id, total_shares, total_assets, position_count, market_cap, block_number, log_index, updated_at)
SELECT 
    t.term_id,
    t.counter_term_id,
    1 as curve_id,
    COALESCE(SUM(spc.total_shares), 0) as total_shares,
    COALESCE(SUM(spc.total_assets), 0) as total_assets,
    COALESCE(SUM(p.position_count), 0) as position_count,
    COALESCE(SUM(spc.total_shares * spc.share_price / POWER(10, 18)), 0) as market_cap,
    t.block_number,
    0 as log_index, -- Default log_index for historical data
    NOW() as updated_at
FROM triple t
LEFT JOIN LATERAL (
    SELECT DISTINCT ON (term_id, curve_id) 
        term_id,
        curve_id,
        total_shares,
        total_assets,
        share_price
    FROM share_price_change 
    WHERE (term_id = t.term_id OR term_id = t.counter_term_id) AND curve_id = 1
    ORDER BY term_id, curve_id, updated_at DESC
) spc ON true
LEFT JOIN LATERAL (
    SELECT 
        term_id,
        COUNT(*) as position_count
    FROM position 
    WHERE (term_id = t.term_id OR term_id = t.counter_term_id) AND curve_id = 1
    GROUP BY term_id
) p ON p.term_id = spc.term_id
GROUP BY t.term_id, t.counter_term_id, t.block_number
ON CONFLICT (term_id, curve_id) DO UPDATE SET
    counter_term_id = EXCLUDED.counter_term_id,
    total_shares = EXCLUDED.total_shares,
    total_assets = EXCLUDED.total_assets,
    position_count = EXCLUDED.position_count,
    market_cap = EXCLUDED.market_cap,
    block_number = EXCLUDED.block_number,
    log_index = EXCLUDED.log_index,
    updated_at = NOW();

-- Then, populate triple_term table by summing all triple_vault records for each term_id/counter_term_id combination
INSERT INTO triple_term (term_id, counter_term_id, total_assets, total_market_cap, updated_at)
SELECT 
    tv.term_id,
    tv.counter_term_id,
    COALESCE(SUM(tv.total_assets), 0) as total_assets,
    COALESCE(SUM(tv.market_cap), 0) as total_market_cap,
    NOW() as updated_at
FROM triple_vault tv
GROUP BY tv.term_id, tv.counter_term_id
ON CONFLICT (term_id) DO UPDATE SET
    counter_term_id = EXCLUDED.counter_term_id,
    total_assets = EXCLUDED.total_assets,
    total_market_cap = EXCLUDED.total_market_cap,
    updated_at = NOW(); 