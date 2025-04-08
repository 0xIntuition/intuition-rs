ALTER TABLE vault ADD COLUMN curve_id NUMERIC(78, 0) NOT NULL DEFAULT 1;
ALTER TABLE vault ADD COLUMN total_assets NUMERIC(78, 0);
ALTER TABLE vault ADD COLUMN market_cap NUMERIC(78, 0);

CREATE INDEX idx_vault_curve ON vault(curve_id);
CREATE INDEX idx_vault_total_assets ON vault(total_assets);
CREATE INDEX idx_vault_market_cap ON vault(market_cap);

