-- Add missing indexes for atoms query optimization

-- Index for atoms filtering and ordering
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_atoms_created_at ON atom(created_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_atoms_updated_at ON atom(updated_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_atoms_type ON atom(type);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_atoms_creator_id ON atom(creator_id);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_atoms_term_id ON atom(term_id);

-- Composite indexes for common query patterns
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_atoms_type_created_at ON atom(type, created_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_atoms_creator_created_at ON atom(creator_id, created_at DESC);

-- Index for triples aggregation (tags)
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_triples_subject_predicate ON triple(subject_id, predicate_id);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_triples_predicate_object ON triple(predicate_id, object_id);

-- Index for vault positions lookup
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_position_term_account ON position(term_id, account_id);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_position_account_term ON position(account_id, term_id);

-- Index for vault ordering
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_vault_term_curve ON vault(term_id, curve_id);

-- Index for atom_value lookups
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_atom_value_id ON atom_value(id);

-- Index for account lookups
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_account_id ON account(id); 

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_total_market_cap ON term(total_market_cap);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_total_assets ON term(total_assets);