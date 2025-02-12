-- NEW LOGIC: Update or insert stats_hour based on the hour part of last_processed_block_timestamp
CREATE OR REPLACE FUNCTION update_stats_hour_based_on_block() 
RETURNS TRIGGER AS $$
DECLARE
    new_hour TIMESTAMP;
    rows_affected INTEGER;
BEGIN
    -- Handle both INSERT and UPDATE events. (For UPDATE, only act if the timestamp changed.)
    IF TG_OP = 'UPDATE' THEN
        IF NEW.last_processed_block_timestamp IS DISTINCT FROM OLD.last_processed_block_timestamp THEN
            new_hour := date_trunc('hour', to_timestamp(NEW.last_processed_block_timestamp::double precision));
        ELSE
            RETURN NEW;
        END IF;
    ELSE  -- For INSERT (or other operations if needed)
        new_hour := date_trunc('hour', to_timestamp(NEW.last_processed_block_timestamp::double precision));
    END IF;
    
    -- Debug output: ensure the function is invoked as expected.
    RAISE NOTICE 'Updating stats_hour for hour: %', new_hour;
    
    -- Try to UPDATE an existing record in stats_hour
    UPDATE stats_hour
    SET total_accounts   = s.total_accounts,
        total_atoms      = s.total_atoms,
        total_triples    = s.total_triples,
        total_positions  = s.total_positions,
        total_signals    = s.total_signals,
        total_fees       = s.total_fees,
        contract_balance = s.contract_balance
    FROM stats s
    WHERE s.id = 0
      AND stats_hour.created_at = new_hour;
      
    GET DIAGNOSTICS rows_affected = ROW_COUNT;
    
    -- If no row was updated, try to INSERT a new record.
    IF rows_affected = 0 THEN
        INSERT INTO stats_hour (
            total_accounts, total_atoms, total_triples, 
            total_positions, total_signals, total_fees, 
            contract_balance, created_at
        )
        SELECT 
            s.total_accounts, s.total_atoms, s.total_triples, 
            s.total_positions, s.total_signals, s.total_fees, 
            s.contract_balance, new_hour
        FROM stats s
        WHERE s.id = 0;
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Drop the old trigger if needed, then create or replace with the updated version.
DROP TRIGGER IF EXISTS trigger_on_stats_change ON stats;

CREATE TRIGGER trigger_on_stats_change
AFTER INSERT OR UPDATE ON stats
FOR EACH ROW
EXECUTE FUNCTION update_stats_hour_based_on_block();
