-- STATS HOUR
CREATE OR REPLACE FUNCTION update_stats_hour() 
RETURNS VOID AS $$ 
BEGIN 
    INSERT INTO base_sepolia_backend.stats_hour (total_accounts, total_atoms, total_triples, total_positions, total_signals, total_fees, contract_balance, created_at) 
    SELECT total_accounts, total_atoms, total_triples, total_positions, total_signals, total_fees, contract_balance, now() FROM base_sepolia_backend.stats WHERE id = 0; 
END; 
$$ LANGUAGE plpgsql;   

-- Table to track the last update time 
CREATE TABLE base_sepolia_backend.stats_hour_tracker ( id SERIAL PRIMARY KEY, last_updated TIMESTAMP ); 
-- Insert initial row 
INSERT INTO base_sepolia_backend.stats_hour_tracker (last_updated) VALUES (CURRENT_TIMESTAMP); 

-- Function to update stats_hour and stats_hour_tracker
CREATE OR REPLACE FUNCTION update_stats_hour_if_needed() RETURNS VOID AS $$ 
DECLARE last_update_time TIMESTAMP; 
BEGIN 
    -- Get the last update time 
    SELECT last_updated INTO last_update_time FROM base_sepolia_backend.stats_hour_tracker WHERE id = 1; 
    -- Check if an hour has passed 
    IF (CURRENT_TIMESTAMP - last_update_time) >= INTERVAL '5 minute' THEN 
        -- Update the stats_hour table 
        INSERT INTO base_sepolia_backend.stats_hour (total_accounts, total_atoms, total_triples, total_positions, total_signals, total_fees, contract_balance, created_at) 
          SELECT total_accounts, total_atoms, total_triples, total_positions, total_signals, total_fees, contract_balance, now() FROM base_sepolia_backend.stats WHERE id = 0; 
        -- Update the tracker 
        UPDATE base_sepolia_backend.stats_hour_tracker SET last_updated = CURRENT_TIMESTAMP WHERE id = 1; 
    END IF; 
END; 
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION trigger_update_stats_hour() 
RETURNS TRIGGER AS $$ 
BEGIN 
    PERFORM update_stats_hour_if_needed(); 
    RETURN NEW; 
END; 
$$ LANGUAGE plpgsql; 

CREATE TRIGGER trigger_on_stats_update
AFTER UPDATE ON base_sepolia_backend.stats
FOR EACH ROW
EXECUTE FUNCTION trigger_update_stats_hour();
