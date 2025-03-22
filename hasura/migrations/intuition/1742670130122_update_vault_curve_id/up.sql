ALTER TABLE vault ADD COLUMN curve_id NUMERIC(78, 0) NOT NULL DEFAULT 1;
CREATE INDEX idx_vault_curve ON vault(curve_id);