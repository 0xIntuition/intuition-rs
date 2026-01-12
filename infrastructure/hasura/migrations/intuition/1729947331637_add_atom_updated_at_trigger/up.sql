-- Add trigger to automatically update updated_at field on atom table

-- Function to update the updated_at timestamp
CREATE OR REPLACE FUNCTION update_atom_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to automatically update updated_at on any UPDATE to atom table
CREATE TRIGGER atom_updated_at_trigger
    BEFORE UPDATE ON atom
    FOR EACH ROW
    EXECUTE FUNCTION update_atom_updated_at();

-- ========================================
-- FUNCTION COMMENTS
-- ========================================

COMMENT ON FUNCTION update_atom_updated_at() IS 'Trigger function that automatically sets updated_at to current timestamp when atom records are modified.';
