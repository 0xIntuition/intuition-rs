-- First, create a temporary column to store the enum values as text
ALTER TABLE cursors.histoflux_cursor ADD COLUMN temp_environment TEXT;

-- Copy the enum values as strings to the temporary column
UPDATE cursors.histoflux_cursor SET temp_environment = environment::TEXT;

-- Drop the enum column
ALTER TABLE cursors.histoflux_cursor DROP COLUMN environment;

-- Rename the temporary column to the original name
ALTER TABLE cursors.histoflux_cursor RENAME COLUMN temp_environment TO environment;
