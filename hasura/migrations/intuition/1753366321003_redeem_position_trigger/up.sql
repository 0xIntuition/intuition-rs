-- Create function to update position total_redeem_assets_for_receiver on redemption insert
CREATE OR REPLACE FUNCTION update_position_redeem_assets()
RETURNS TRIGGER AS $$
BEGIN
    -- Update position.total_redeem_assets_for_receiver by adding redemption.assets_for_receiver
    -- PostgreSQL's UPDATE is atomic, so concurrent updates will be serialized
    UPDATE position 
    SET total_redeem_assets_for_receiver = 
        COALESCE(total_redeem_assets_for_receiver, 0) + NEW.assets_for_receiver
    WHERE account_id = NEW.sender_id 
      AND term_id = NEW.term_id 
      AND curve_id = NEW.curve_id;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger on redemption table
CREATE TRIGGER redemption_position_update_trigger
AFTER INSERT ON redemption
FOR EACH ROW
EXECUTE FUNCTION update_position_redeem_assets();