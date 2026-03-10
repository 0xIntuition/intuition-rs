-- Revert: Redemption triggers back to using sender_id

CREATE OR REPLACE FUNCTION update_position_redeem_assets()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE position
    SET total_redeem_assets_for_receiver =
        COALESCE(total_redeem_assets_for_receiver, 0) + NEW.assets
    WHERE account_id = NEW.sender_id
      AND term_id = NEW.term_id
      AND curve_id = NEW.curve_id;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION insert_position_change_from_redemption()
RETURNS TRIGGER AS $$
BEGIN
  INSERT INTO position_change (
    created_at,
    account_id,
    term_id,
    curve_id,
    shares_delta,
    assets_in,
    assets_out,
    event_type,
    event_id,
    block_number,
    transaction_hash,
    log_index
  ) VALUES (
    NEW.created_at,
    NEW.sender_id,
    NEW.term_id,
    NEW.curve_id,
    -NEW.shares,
    0,
    NEW.assets,
    'redemption',
    NEW.id,
    NEW.block_number,
    NEW.transaction_hash,
    NEW.log_index
  );
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Revert historical position_change rows
UPDATE position_change pc
SET account_id = r.sender_id
FROM redemption r
WHERE pc.event_type = 'redemption'
  AND pc.event_id = r.id
  AND r.sender_id <> r.receiver_id;
