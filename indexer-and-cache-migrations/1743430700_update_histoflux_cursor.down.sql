
-- Create the enum type if it doesn't exist (include all your current enum values)
DO $$ BEGIN
    CREATE TYPE cursors.environment_type AS ENUM (
        'base_mainnet',
        'base_sepolia',
        'linea_mainnet',
        'linea_sepolia'
        -- Add any other values your enum currently has
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Reverse the process
ALTER TABLE cursors.histoflux_cursor ALTER COLUMN environment TYPE cursors.environment_type USING environment::cursors.environment_type; 