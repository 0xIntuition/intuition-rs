-- Consolidated Indexes
-- This file contains all indexes from all migrations

-- Basic schema indexes
CREATE INDEX atom_log_index_idx ON atom(log_index);
CREATE INDEX vault_block_number_idx ON vault(block_number);
CREATE INDEX vault_log_index_idx ON vault(log_index);
CREATE INDEX vault_transaction_hash_idx ON vault(transaction_hash);
CREATE INDEX vault_current_share_price_idx ON vault(current_share_price);
CREATE INDEX vault_position_count_idx ON vault(position_count);
CREATE INDEX vault_total_shares_idx ON vault(total_shares);
CREATE INDEX vault_created_at_idx ON vault(created_at);
CREATE INDEX vault_updated_at_idx ON vault(updated_at);
CREATE INDEX fee_transfer_block_number_idx ON fee_transfer(block_number);
CREATE INDEX fee_transfer_transaction_hash_idx ON fee_transfer(transaction_hash);
CREATE INDEX fee_transfer_created_at_idx ON fee_transfer(created_at);
CREATE INDEX deposit_block_number_idx ON deposit(block_number);
CREATE INDEX deposit_transaction_hash_idx ON deposit(transaction_hash);
CREATE INDEX deposit_log_index_idx ON deposit(log_index);
CREATE INDEX deposit_vault_type_idx ON deposit(vault_type);
CREATE INDEX deposit_created_at_idx ON deposit(created_at);
CREATE INDEX redemption_block_number_idx ON redemption(block_number);
CREATE INDEX redemption_transaction_hash_idx ON redemption(transaction_hash);
CREATE INDEX redemption_log_index_idx ON redemption(log_index);
CREATE INDEX redemption_created_at_idx ON redemption(created_at);
CREATE INDEX position_account_vault_idx ON position(account_id, term_id);
CREATE INDEX position_shares_idx ON position(shares);
CREATE INDEX position_block_number_idx ON position(block_number);
CREATE INDEX position_log_index_idx ON position(log_index);
CREATE INDEX position_transaction_hash_idx ON position(transaction_hash);
CREATE INDEX position_transaction_index_idx ON position(transaction_index);

-- Basic schema foreign key indexes
CREATE INDEX idx_atom_creator ON atom(creator_id);
CREATE INDEX idx_atom_vault ON atom(term_id);
CREATE INDEX idx_term_created_at ON term(created_at);
CREATE INDEX idx_triple_creator ON triple(creator_id);
CREATE INDEX idx_triple_subject ON triple(subject_id);
CREATE INDEX idx_triple_predicate ON triple(predicate_id);
CREATE INDEX idx_triple_object ON triple(object_id);
CREATE INDEX idx_vault_atom ON vault(term_id);
CREATE INDEX idx_vault_triple ON vault(term_id);
CREATE INDEX idx_fee_transfer_sender ON fee_transfer(sender_id);
CREATE INDEX idx_fee_transfer_receiver ON fee_transfer(receiver_id);
CREATE INDEX idx_deposit_sender ON deposit(sender_id);
CREATE INDEX idx_deposit_receiver ON deposit(receiver_id);
CREATE INDEX idx_deposit_vault ON deposit(term_id);
CREATE INDEX idx_redemption_sender ON redemption(sender_id);
CREATE INDEX idx_redemption_receiver ON redemption(receiver_id);
CREATE INDEX idx_redemption_vault ON redemption(term_id);
CREATE INDEX idx_position_account ON position(account_id);
CREATE INDEX idx_position_vault ON position(term_id);
CREATE INDEX idx_predicate_object_predicate ON predicate_object(predicate_id);
CREATE INDEX idx_predicate_object_object ON predicate_object(object_id);
CREATE INDEX idx_subject_predicate_subject ON subject_predicate(subject_id);
CREATE INDEX idx_subject_predicate_predicate ON subject_predicate(predicate_id);
CREATE INDEX idx_signal_account ON signal(account_id);
CREATE INDEX idx_signal_atom ON signal(atom_id);
CREATE INDEX idx_signal_triple ON signal(triple_id);
CREATE INDEX idx_signal_created_at ON signal(created_at);
CREATE INDEX idx_atom_value_atom ON atom_value(id);
CREATE INDEX idx_atom_value_thing ON atom_value(thing_id);
CREATE INDEX idx_atom_value_person ON atom_value(person_id);
CREATE INDEX idx_atom_value_organization ON atom_value(organization_id);
CREATE INDEX idx_atom_value_book ON atom_value(book_id);
CREATE INDEX idx_thing_name ON thing(name);
CREATE INDEX idx_thing_description ON thing(description);
CREATE INDEX idx_thing_url ON thing(url);
CREATE INDEX idx_person_name ON person(name);
CREATE INDEX idx_person_description ON person(description);
CREATE INDEX idx_person_url ON person(url);
CREATE INDEX idx_organization_name ON organization(name);
CREATE INDEX idx_organization_description ON organization(description);
CREATE INDEX idx_organization_url ON organization(url);
CREATE INDEX idx_event_type ON event(type);
CREATE INDEX idx_event_atom ON event(atom_id);
CREATE INDEX idx_event_triple ON event(triple_id);
CREATE INDEX idx_event_block_number ON event(block_number);
CREATE INDEX idx_event_created_at ON event(created_at);
CREATE INDEX idx_event_transaction_hash ON event(transaction_hash);

-- Text and JSON object indexes
CREATE INDEX IF NOT EXISTS idx_text_object_data_fts ON text_object USING GIN(to_tsvector('english', data));
CREATE INDEX IF NOT EXISTS idx_text_object_data ON text_object(data);
CREATE INDEX IF NOT EXISTS idx_json_object_data ON json_object USING GIN(data);

-- Atom search indexes
CREATE INDEX IF NOT EXISTS idx_atom_label ON atom(label);

-- Account search indexes
CREATE INDEX IF NOT EXISTS idx_account_label ON account(label);

-- Term table indexes
CREATE INDEX idx_term_id ON term(id);
CREATE INDEX idx_term_type ON term(type);
CREATE INDEX idx_term_atom_id ON term(atom_id);
CREATE INDEX idx_term_triple_id ON term(triple_id);
CREATE INDEX idx_total_market_cap ON term(total_market_cap);
CREATE INDEX idx_total_assets ON term(total_assets);

-- Vault structure indexes
CREATE INDEX idx_vault_term_id ON vault(term_id);
CREATE INDEX idx_curve_id ON vault(curve_id);
CREATE INDEX idx_vault_total_assets ON vault(total_assets);
CREATE INDEX idx_vault_market_cap ON vault(market_cap);

-- Triple vault indexes
CREATE INDEX idx_triple_vault_term_id ON triple_vault(term_id);
CREATE INDEX idx_triple_vault_counter_term_id ON triple_vault(counter_term_id);
CREATE INDEX idx_triple_vault_curve_id ON triple_vault(curve_id);
CREATE INDEX idx_triple_vault_total_shares ON triple_vault(total_shares);
CREATE INDEX idx_triple_vault_total_assets ON triple_vault(total_assets);
CREATE INDEX idx_triple_vault_position_count ON triple_vault(position_count);
CREATE INDEX idx_triple_vault_market_cap ON triple_vault(market_cap);
CREATE INDEX idx_triple_vault_block_number ON triple_vault(block_number);
CREATE INDEX idx_triple_vault_log_index ON triple_vault(log_index);
CREATE INDEX idx_triple_vault_updated_at ON triple_vault(updated_at);

-- Triple term indexes
CREATE INDEX idx_triple_term_term_id ON triple_term(term_id);
CREATE INDEX idx_triple_term_counter_term_id ON triple_term(counter_term_id);
CREATE INDEX idx_triple_term_total_assets ON triple_term(total_assets);
CREATE INDEX idx_triple_term_total_market_cap ON triple_term(total_market_cap);
CREATE INDEX idx_triple_term_updated_at ON triple_term(updated_at);

-- Share price change indexes
CREATE INDEX idx_share_price_change_curve_id ON share_price_change(curve_id);
CREATE INDEX idx_share_price_change_updated_at ON share_price_change(updated_at);
CREATE INDEX idx_share_price_change_term_updated_at ON share_price_change(updated_at);
CREATE INDEX idx_share_price_change_term_block_number ON share_price_change(block_number);
CREATE INDEX idx_share_price_change_term_transaction_hash ON share_price_change(transaction_hash);
CREATE INDEX idx_share_price_change_term_log_index ON share_price_change(log_index);

-- Initialize indexes
CREATE INDEX idx_initialize_block_number ON initialize(block_number);
CREATE INDEX idx_initialize_transaction_hash ON initialize(transaction_hash);

-- Failed logs indexes
CREATE INDEX idx_failed_logs_block_number ON failed_logs(block_number);
CREATE INDEX idx_failed_logs_transaction_hash ON failed_logs(transaction_hash);
CREATE INDEX idx_failed_logs_address ON failed_logs(address);
CREATE INDEX idx_failed_logs_block_timestamp ON failed_logs(block_timestamp);

-- Missing indexes for atoms query optimization
CREATE INDEX IF NOT EXISTS idx_atoms_created_at ON atom(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_atoms_updated_at ON atom(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_atoms_type ON atom(type);
CREATE INDEX IF NOT EXISTS idx_atoms_creator_id ON atom(creator_id);
CREATE INDEX IF NOT EXISTS idx_atoms_term_id ON atom(term_id);

-- Composite indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_atoms_type_created_at ON atom(type, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_atoms_creator_created_at ON atom(creator_id, created_at DESC);

-- Index for triples aggregation (tags)
CREATE INDEX IF NOT EXISTS idx_triples_subject_predicate ON triple(subject_id, predicate_id);
CREATE INDEX IF NOT EXISTS idx_triples_predicate_object ON triple(predicate_id, object_id);

-- Index for vault positions lookup
CREATE INDEX IF NOT EXISTS idx_position_term_account ON position(term_id, account_id);
CREATE INDEX IF NOT EXISTS idx_position_account_term ON position(account_id, term_id);

-- Index for vault ordering
CREATE INDEX IF NOT EXISTS idx_vault_term_curve ON vault(term_id, curve_id);

-- Index for atom_value lookups
CREATE INDEX IF NOT EXISTS idx_atom_value_id ON atom_value(id);

-- Index for account lookups
CREATE INDEX IF NOT EXISTS idx_account_id ON account(id);

-- ========================================
-- CRITICAL INDEXES FOR CONTINUOUS AGGREGATES
-- ========================================
-- These indexes are essential for efficient continuous aggregate refreshes
-- Without these, continuous aggregates perform full table scans on every refresh
-- All continuous aggregates GROUP BY (term_id, curve_id, time) or (term_id, time)

-- Signal table indexes for continuous aggregates
CREATE INDEX IF NOT EXISTS idx_signal_term_curve_time
ON signal(term_id, curve_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_signal_time_term_curve
ON signal(created_at DESC, term_id, curve_id);

CREATE INDEX IF NOT EXISTS idx_signal_term_id
ON signal(term_id);

CREATE INDEX IF NOT EXISTS idx_signal_curve_id
ON signal(curve_id);

-- Share price change table indexes for continuous aggregates
CREATE INDEX IF NOT EXISTS idx_share_price_change_term_curve_time
ON share_price_change(term_id, curve_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_share_price_change_time_term_curve
ON share_price_change(updated_at DESC, term_id, curve_id);

CREATE INDEX IF NOT EXISTS idx_share_price_change_term_id
ON share_price_change(term_id);

-- Term total state change table indexes for continuous aggregates
CREATE INDEX IF NOT EXISTS idx_term_total_state_change_term_time
ON term_total_state_change(term_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_term_total_state_change_time_term
ON term_total_state_change(created_at DESC, term_id);

CREATE INDEX IF NOT EXISTS idx_term_total_state_change_term_id
ON term_total_state_change(term_id);

-- ========================================
-- INDEXES FOR PREDICATE_OBJECT AND SUBJECT_PREDICATE TRIGGERS
-- ========================================
-- These indexes are critical for the performance of:
-- - update_predicate_object_aggregates() trigger
-- - update_subject_predicate_aggregates() trigger
-- - update_triple_vault_from_vault() trigger

-- Critical: triple.term_id is used in WHERE clauses of both aggregate triggers
CREATE INDEX IF NOT EXISTS idx_triple_term_id ON triple(term_id);

-- Optimization: Covering indexes for predicate_object aggregate queries
CREATE INDEX IF NOT EXISTS idx_triple_predicate_object_term ON triple(predicate_id, object_id, term_id);

-- Optimization: Covering indexes for subject_predicate aggregate queries
CREATE INDEX IF NOT EXISTS idx_triple_subject_predicate_term ON triple(subject_id, predicate_id, term_id);

-- Optimization: Composite indexes for triple_vault lookups in update_triple_vault_from_vault trigger
CREATE INDEX IF NOT EXISTS idx_triple_vault_term_curve ON triple_vault(term_id, curve_id);
CREATE INDEX IF NOT EXISTS idx_triple_vault_counter_curve ON triple_vault(counter_term_id, curve_id);

-- ========================================
-- CRITICAL INDEXES FOR POSITION UPDATE TRIGGERS
-- ========================================
-- These indexes are essential for deposit and redemption position update triggers
-- The update_position_deposit_assets() and update_position_redeem_assets() triggers
-- fire on every deposit/redemption INSERT and need to efficiently locate positions
-- by the composite key (account_id, term_id, curve_id)

-- Critical: 3-column composite index for position lookups
-- Used by: update_position_deposit_assets() and update_position_redeem_assets() triggers
-- Query pattern: UPDATE position WHERE account_id = X AND term_id = Y AND curve_id = Z
CREATE INDEX IF NOT EXISTS idx_position_account_term_curve ON position(account_id, term_id, curve_id);

-- ========================================
-- INDEXES FOR SEARCH FUNCTIONS
-- ========================================
-- These indexes optimize the search_positions_on_subject() function

-- Index for atom data filtering in search queries
-- Used by: search_positions_on_subject() function
-- Query pattern: WHERE (predicate_atom.data, object_atom.data) IN (...)
CREATE INDEX IF NOT EXISTS idx_atom_data ON atom(data);

-- ========================================
-- OPTIMIZATION INDEXES FOR COMPLEX AGGREGATES
-- ========================================
-- These indexes optimize complex aggregate queries in triple_vault updates

-- Composite index for triple_term lookups
-- Used by: update_triple_vault_from_vault() function
-- Query pattern: WHERE term_id = X AND counter_term_id = Y (or vice versa)
CREATE INDEX IF NOT EXISTS idx_triple_term_composite ON triple_term(term_id, counter_term_id);
