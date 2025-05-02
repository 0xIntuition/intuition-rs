CREATE TABLE initialize (
    version BIGINT NOT NULL PRIMARY KEY,
    block_number NUMERIC(78,0) NOT NULL,
    block_timestamp BIGINT NOT NULL,
    transaction_hash TEXT NOT NULL,
    log_index INTEGER NOT NULL
);

CREATE INDEX idx_initialize_block_number ON initialize(block_number);
CREATE INDEX idx_initialize_transaction_hash ON initialize(transaction_hash);

ALTER TYPE event_type ADD VALUE 'Initialized'; 

CREATE OR REPLACE FUNCTION notify_version_change()
RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('version_change_channel', row_to_json(NEW)::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER version_change_trigger
    AFTER INSERT ON initialize
    FOR EACH ROW
    EXECUTE FUNCTION notify_version_change();