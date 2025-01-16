-- Ensure we have a record in the stats table
INSERT INTO base_sepolia_backend.stats (id, total_accounts, total_atoms, total_triples, total_positions, total_signals, total_fees, contract_balance)
VALUES (0, 0, 0, 0, 0, 0, 0, 0);


-- ACCOUNT STATS
-- Create a trigger on the accounts table for inserts
CREATE OR REPLACE FUNCTION update_account_stats()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE base_sepolia_backend.stats
    SET total_accounts = total_accounts + 1
    WHERE id = 0;  -- Assuming single row stats table with id 0
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ATOM STATS
-- Create a trigger on the atom table for inserts
CREATE OR REPLACE FUNCTION update_atom_stats()
RETURNS TRIGGER AS $$
BEGIN
    -- Update stats logic here
    UPDATE base_sepolia_backend.stats
    SET total_atoms = total_atoms + 1
    WHERE id = 0;  -- Assuming single row stats table with id 0
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- TRIPLE STATS
-- Create a trigger on the triple table for inserts
CREATE OR REPLACE FUNCTION update_triple_stats()
RETURNS TRIGGER AS $$
BEGIN
    -- Update stats logic here
    UPDATE base_sepolia_backend.stats
    SET total_triples = total_triples + 1
    WHERE id = 0;  -- Assuming single row stats table with id 0
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- POSITION STATS
-- Create a trigger on the position table for inserts
CREATE OR REPLACE FUNCTION update_position_stats()
RETURNS TRIGGER AS $$
BEGIN
    -- Update stats logic here
    UPDATE base_sepolia_backend.stats
    SET total_positions = total_positions + 1
    WHERE id = 0;  -- Assuming single row stats table with id 0
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- SIGNAL STATS
-- Create a trigger on the signal table for inserts
CREATE OR REPLACE FUNCTION update_signal_stats()
RETURNS TRIGGER AS $$
BEGIN
    -- Update stats logic here
    UPDATE base_sepolia_backend.stats
    SET total_signals = total_signals + 1
    WHERE id = 0;  -- Assuming single row stats table with id 0
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- FEE STATS
-- Create a trigger on the fee_transfer table for inserts
CREATE OR REPLACE FUNCTION update_fee_stats()
RETURNS TRIGGER AS $$
BEGIN
    -- Update stats logic here
    UPDATE base_sepolia_backend.stats
    SET total_fees = total_fees + NEW.amount
    WHERE id = 0;  -- Assuming single row stats table with id 0
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- DEPOSITED STATS
-- Create a trigger on the deposit table for inserts
CREATE OR REPLACE FUNCTION update_deposit_stats()
RETURNS TRIGGER AS $$
BEGIN
    -- Update stats logic here
    UPDATE base_sepolia_backend.stats
    SET contract_balance = contract_balance + NEW.sender_assets_after_total_fees
    WHERE id = 0;  -- Assuming single row stats table with id 0
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- REDEMPTION STATS
-- Create a trigger on the redemption table for inserts
CREATE OR REPLACE FUNCTION update_redemption_stats()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.sender_total_shares_in_vault = 0 THEN
        -- Full redemption - update both positions and balance
        UPDATE base_sepolia_backend.stats
        SET total_positions = total_positions - 1,
            contract_balance = contract_balance - NEW.assets_for_receiver
        WHERE id = 0;
    ELSE
        -- Partial redemption - only update balance
        UPDATE base_sepolia_backend.stats
        SET contract_balance = contract_balance - NEW.assets_for_receiver
        WHERE id = 0;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;


-- TRIGGERS
-- Create a trigger on the accounts table for inserts
CREATE TRIGGER account_insert_trigger
AFTER INSERT ON base_sepolia_backend.account
FOR EACH ROW
EXECUTE FUNCTION update_account_stats();

-- Create a trigger on the atom table for inserts
CREATE TRIGGER atom_insert_trigger
AFTER INSERT ON base_sepolia_backend.atom
FOR EACH ROW
EXECUTE FUNCTION update_atom_stats();

-- Create a trigger on the triple table for inserts
CREATE TRIGGER triple_insert_trigger
AFTER INSERT ON base_sepolia_backend.triple
FOR EACH ROW
EXECUTE FUNCTION update_triple_stats();

-- Create a trigger on the position table for inserts
CREATE TRIGGER position_insert_trigger
AFTER INSERT ON base_sepolia_backend.position
FOR EACH ROW
EXECUTE FUNCTION update_position_stats();

-- Create a trigger on the signal table for inserts
CREATE TRIGGER signal_insert_trigger
AFTER INSERT ON base_sepolia_backend.signal
FOR EACH ROW
EXECUTE FUNCTION update_signal_stats();

-- Create a trigger on the fee_transfer table for inserts
CREATE TRIGGER fee_insert_trigger
AFTER INSERT ON base_sepolia_backend.fee_transfer
FOR EACH ROW
EXECUTE FUNCTION update_fee_stats();

-- Create a trigger on the deposit table for inserts
CREATE TRIGGER deposit_insert_trigger
AFTER INSERT ON base_sepolia_backend.deposit
FOR EACH ROW
EXECUTE FUNCTION update_deposit_stats();

-- Create a trigger on the redemption table for inserts
CREATE TRIGGER redemption_insert_trigger
AFTER INSERT ON base_sepolia_backend.redemption
FOR EACH ROW
EXECUTE FUNCTION update_redemption_stats();

