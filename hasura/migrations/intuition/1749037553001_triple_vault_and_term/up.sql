CREATE TABLE triple_vault (
  term_id NUMERIC(78, 0) REFERENCES term(id) NOT NULL,
  counter_term_id NUMERIC(78, 0) REFERENCES term(id) NOT NULL,
  curve_id NUMERIC(78, 0) NOT NULL,
  total_shares NUMERIC(78, 0) NOT NULL,
  total_assets NUMERIC(78, 0) NOT NULL,
  position_count BIGINT NOT NULL,
  market_cap NUMERIC(78, 0) NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  log_index BIGINT NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
  PRIMARY KEY (term_id, curve_id)
);

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

CREATE TABLE triple_term (
  term_id NUMERIC(78, 0) REFERENCES term(id) NOT NULL,
  counter_term_id NUMERIC(78, 0) REFERENCES term(id) NOT NULL,
  total_assets NUMERIC(78, 0) NOT NULL,
  total_market_cap NUMERIC(78, 0) NOT NULL,
  total_position_count BIGINT NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
  PRIMARY KEY (term_id)
);

CREATE INDEX idx_triple_term_term_id ON triple_term(term_id);
CREATE INDEX idx_triple_term_counter_term_id ON triple_term(counter_term_id);
CREATE INDEX idx_triple_term_total_assets ON triple_term(total_assets);
CREATE INDEX idx_triple_term_total_market_cap ON triple_term(total_market_cap);
CREATE INDEX idx_triple_term_updated_at ON triple_term(updated_at);


