-- Rollback: Remove compression policies and settings

-- ========================================
-- REMOVE COMPRESSION POLICIES
-- ========================================

SELECT remove_compression_policy('signal', if_exists => true);
SELECT remove_compression_policy('share_price_change', if_exists => true);
SELECT remove_compression_policy('term_total_state_change', if_exists => true);

-- ========================================
-- DECOMPRESS ANY COMPRESSED CHUNKS
-- ========================================

-- Decompress all chunks for signal table
SELECT decompress_chunk(chunk, if_compressed => true)
FROM show_chunks('signal');

-- Decompress all chunks for share_price_change table
SELECT decompress_chunk(chunk, if_compressed => true)
FROM show_chunks('share_price_change');

-- Decompress all chunks for term_total_state_change table
SELECT decompress_chunk(chunk, if_compressed => true)
FROM show_chunks('term_total_state_change');

-- ========================================
-- DISABLE COMPRESSION
-- ========================================

ALTER TABLE signal SET (
  timescaledb.compress = false
);

ALTER TABLE share_price_change SET (
  timescaledb.compress = false
);

ALTER TABLE term_total_state_change SET (
  timescaledb.compress = false
);

-- Note: Decompression may take some time for large datasets
-- During decompression, the data is still queryable
