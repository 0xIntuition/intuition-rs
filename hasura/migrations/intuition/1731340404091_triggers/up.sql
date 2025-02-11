-- Ensure we have a record in the stats table
INSERT INTO stats (id, total_accounts, total_atoms, total_triples, total_positions, total_signals, total_fees, contract_balance, last_processed_block_number, last_processed_block_timestamp)
VALUES (0, 0, 0, 0, 0, 0, 0, 0, 0, 0);


-- ACCOUNT STATS
-- Create a trigger on the accounts table for inserts
CREATE OR REPLACE FUNCTION update_account_stats()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE stats
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
    UPDATE stats
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
    UPDATE stats
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
    UPDATE stats
    SET total_positions = total_positions + 1
    WHERE id = 0;  -- Assuming single row stats table with id 0
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION delete_position_stats()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE stats
    SET total_positions = total_positions - 1
    WHERE id = 0;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;
-- SIGNAL STATS
-- Create a trigger on the signal table for inserts
CREATE OR REPLACE FUNCTION update_signal_stats()
RETURNS TRIGGER AS $$
BEGIN
    -- Update stats logic here
    UPDATE stats
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
    UPDATE stats
    SET total_fees = total_fees + NEW.amount
    WHERE id = 0;  -- Assuming single row stats table with id 0
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- TRIGGERS
-- Create a trigger on the accounts table for inserts
CREATE TRIGGER account_insert_trigger
AFTER INSERT ON account
FOR EACH ROW
EXECUTE FUNCTION update_account_stats();

-- Create a trigger on the atom table for inserts
CREATE TRIGGER atom_insert_trigger
AFTER INSERT ON atom
FOR EACH ROW
EXECUTE FUNCTION update_atom_stats();

-- Create a trigger on the triple table for inserts
CREATE TRIGGER triple_insert_trigger
AFTER INSERT ON triple
FOR EACH ROW
EXECUTE FUNCTION update_triple_stats();

-- Create a trigger on the position table for inserts
CREATE TRIGGER position_insert_trigger
AFTER INSERT ON position
FOR EACH ROW
EXECUTE FUNCTION update_position_stats();

CREATE TRIGGER position_delete_trigger
AFTER DELETE ON position
FOR EACH ROW
EXECUTE FUNCTION delete_position_stats();

-- Create a trigger on the signal table for inserts
CREATE TRIGGER signal_insert_trigger
AFTER INSERT ON signal
FOR EACH ROW
EXECUTE FUNCTION update_signal_stats();

-- Create a trigger on the fee_transfer table for inserts
CREATE TRIGGER fee_insert_trigger
AFTER INSERT ON fee_transfer
FOR EACH ROW
EXECUTE FUNCTION update_fee_stats();


