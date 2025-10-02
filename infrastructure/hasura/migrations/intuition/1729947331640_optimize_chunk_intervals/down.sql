-- Rollback: Restore default chunk intervals

-- Restore 7-day chunk intervals (TimescaleDB default for TIMESTAMP columns)

SELECT set_chunk_time_interval('signal', INTERVAL '7 days');

SELECT set_chunk_time_interval('share_price_change', INTERVAL '7 days');

SELECT set_chunk_time_interval('term_total_state_change', INTERVAL '7 days');

-- Note: This only affects NEW chunks created after the rollback
