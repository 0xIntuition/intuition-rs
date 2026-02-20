-- Add supporter_count and opposer_count to predicate_object
ALTER TABLE predicate_object
  ADD COLUMN supporter_count BIGINT NOT NULL DEFAULT 0,
  ADD COLUMN opposer_count BIGINT NOT NULL DEFAULT 0;

-- Add supporter_count and opposer_count to subject_predicate
ALTER TABLE subject_predicate
  ADD COLUMN supporter_count BIGINT NOT NULL DEFAULT 0,
  ADD COLUMN opposer_count BIGINT NOT NULL DEFAULT 0;

-- Indexes for sorting
CREATE INDEX idx_predicate_object_supporter_count ON predicate_object (supporter_count DESC);
CREATE INDEX idx_predicate_object_opposer_count ON predicate_object (opposer_count DESC);
CREATE INDEX idx_subject_predicate_supporter_count ON subject_predicate (supporter_count DESC);
CREATE INDEX idx_subject_predicate_opposer_count ON subject_predicate (opposer_count DESC);

-- Update trigger function to also aggregate supporter_count and opposer_count
CREATE OR REPLACE FUNCTION update_predicate_object_aggregates()
RETURNS TRIGGER AS $$
DECLARE
    affected_term_id TEXT;
    affected_counter_term_id TEXT;
BEGIN
    -- Determine which term_id and counter_term_id were affected
    IF (TG_OP = 'DELETE') THEN
        affected_term_id := OLD.term_id;
        affected_counter_term_id := OLD.counter_term_id;
    ELSE
        affected_term_id := NEW.term_id;
        affected_counter_term_id := NEW.counter_term_id;
    END IF;

    -- Insert or update all predicate_object records for triples that match either term_id or counter_term_id
    -- Use CTE to avoid duplicate subquery execution
    WITH affected_triples AS (
        SELECT t.predicate_id, t.object_id, t.term_id
        FROM triple t
        WHERE t.term_id = affected_term_id
           OR t.term_id = affected_counter_term_id
    ),
    predicate_object_aggregates AS (
        SELECT
            at.predicate_id,
            at.object_id,
            COALESCE(SUM(tt.total_market_cap), 0) AS agg_market_cap,
            COALESCE(SUM(tt.total_position_count), 0) AS agg_position_count,
            COALESCE(SUM(tt.supporter_count), 0) AS agg_supporter_count,
            COALESCE(SUM(tt.opposer_count), 0) AS agg_opposer_count
        FROM affected_triples at
        LEFT JOIN triple t ON t.predicate_id = at.predicate_id AND t.object_id = at.object_id
        LEFT JOIN triple_term tt ON tt.term_id = t.term_id
        GROUP BY at.predicate_id, at.object_id
    )
    INSERT INTO predicate_object (predicate_id, object_id, triple_count, total_market_cap, total_position_count, supporter_count, opposer_count)
    SELECT
        poa.predicate_id,
        poa.object_id,
        0, -- Initial triple_count, managed exclusively by triple insert trigger
        poa.agg_market_cap,
        poa.agg_position_count,
        poa.agg_supporter_count,
        poa.agg_opposer_count
    FROM predicate_object_aggregates poa
    ON CONFLICT (predicate_id, object_id) DO UPDATE SET
        total_market_cap = EXCLUDED.total_market_cap,
        total_position_count = EXCLUDED.total_position_count,
        supporter_count = EXCLUDED.supporter_count,
        opposer_count = EXCLUDED.opposer_count;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Update trigger function to also aggregate supporter_count and opposer_count
CREATE OR REPLACE FUNCTION update_subject_predicate_aggregates()
RETURNS TRIGGER AS $$
DECLARE
    affected_term_id TEXT;
    affected_counter_term_id TEXT;
BEGIN
    -- Determine which term_id and counter_term_id were affected
    IF (TG_OP = 'DELETE') THEN
        affected_term_id := OLD.term_id;
        affected_counter_term_id := OLD.counter_term_id;
    ELSE
        affected_term_id := NEW.term_id;
        affected_counter_term_id := NEW.counter_term_id;
    END IF;

    -- Insert or update all subject_predicate records for triples that match either term_id or counter_term_id
    -- Use CTE to avoid duplicate subquery execution
    WITH affected_triples AS (
        SELECT t.subject_id, t.predicate_id, t.term_id
        FROM triple t
        WHERE t.term_id = affected_term_id
           OR t.term_id = affected_counter_term_id
    ),
    subject_predicate_aggregates AS (
        SELECT
            at.subject_id,
            at.predicate_id,
            COALESCE(SUM(tt.total_market_cap), 0) AS agg_market_cap,
            COALESCE(SUM(tt.total_position_count), 0) AS agg_position_count,
            COALESCE(SUM(tt.supporter_count), 0) AS agg_supporter_count,
            COALESCE(SUM(tt.opposer_count), 0) AS agg_opposer_count
        FROM affected_triples at
        LEFT JOIN triple t ON t.subject_id = at.subject_id AND t.predicate_id = at.predicate_id
        LEFT JOIN triple_term tt ON tt.term_id = t.term_id
        GROUP BY at.subject_id, at.predicate_id
    )
    INSERT INTO subject_predicate (subject_id, predicate_id, triple_count, total_market_cap, total_position_count, supporter_count, opposer_count)
    SELECT
        spa.subject_id,
        spa.predicate_id,
        0, -- Initial triple_count, managed exclusively by triple insert trigger
        spa.agg_market_cap,
        spa.agg_position_count,
        spa.agg_supporter_count,
        spa.agg_opposer_count
    FROM subject_predicate_aggregates spa
    ON CONFLICT (subject_id, predicate_id) DO UPDATE SET
        total_market_cap = EXCLUDED.total_market_cap,
        total_position_count = EXCLUDED.total_position_count,
        supporter_count = EXCLUDED.supporter_count,
        opposer_count = EXCLUDED.opposer_count;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Backfill predicate_object from triple_term
UPDATE predicate_object po
SET
  supporter_count = COALESCE(agg.sc, 0),
  opposer_count   = COALESCE(agg.oc, 0)
FROM (
  SELECT t.predicate_id, t.object_id,
         SUM(tt.supporter_count) AS sc,
         SUM(tt.opposer_count)   AS oc
  FROM triple t
  JOIN triple_term tt ON tt.term_id = t.term_id
  GROUP BY t.predicate_id, t.object_id
) agg
WHERE po.predicate_id = agg.predicate_id
  AND po.object_id = agg.object_id;

-- Backfill subject_predicate from triple_term
UPDATE subject_predicate sp
SET
  supporter_count = COALESCE(agg.sc, 0),
  opposer_count   = COALESCE(agg.oc, 0)
FROM (
  SELECT t.subject_id, t.predicate_id,
         SUM(tt.supporter_count) AS sc,
         SUM(tt.opposer_count)   AS oc
  FROM triple t
  JOIN triple_term tt ON tt.term_id = t.term_id
  GROUP BY t.subject_id, t.predicate_id
) agg
WHERE sp.subject_id = agg.subject_id
  AND sp.predicate_id = agg.predicate_id;
