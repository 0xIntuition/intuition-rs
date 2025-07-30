-- Create function to update position total_deposit_assets_after_total_fees on deposit insert
CREATE OR REPLACE FUNCTION update_position_deposit_assets()
RETURNS TRIGGER AS $$
BEGIN
    -- Update position.total_deposit_assets_after_total_fees by adding deposit.sender_assets_after_total_fees
    -- PostgreSQL's UPDATE is atomic, so concurrent updates will be serialized
    UPDATE position 
    SET total_deposit_assets_after_total_fees = 
        COALESCE(total_deposit_assets_after_total_fees, 0) + NEW.sender_assets_after_total_fees
    WHERE account_id = NEW.receiver_id 
      AND term_id = NEW.term_id 
      AND curve_id = NEW.curve_id;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger on deposit table
CREATE TRIGGER deposit_position_update_trigger
AFTER INSERT ON deposit
FOR EACH ROW
EXECUTE FUNCTION update_position_deposit_assets();