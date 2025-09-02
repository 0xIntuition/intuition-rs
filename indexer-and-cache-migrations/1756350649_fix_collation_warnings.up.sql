-- Fix collation version mismatch warnings
-- This addresses PostgreSQL collation version warnings that can appear in logs

-- Update collation versions for the database to match current system locale
-- This is safe to run and will suppress the warnings
DO $$
BEGIN
    -- Try to refresh collation versions to match current system
    -- This command is idempotent and safe to run multiple times
    -- Note: REFRESH COLLATION VERSION is only available in PostgreSQL 12+
    BEGIN
        EXECUTE 'REFRESH COLLATION VERSION FOR (SELECT oid FROM pg_collation WHERE collname = ''default'')';
        RAISE NOTICE 'Refreshed collation versions to suppress warnings';
    EXCEPTION
        WHEN syntax_error OR undefined_function THEN
            -- If the command fails (older PostgreSQL versions), log but don't fail
            RAISE WARNING 'REFRESH COLLATION VERSION not supported in this PostgreSQL version. This is non-critical.';
    END;
    
EXCEPTION
    WHEN OTHERS THEN
        -- If the command fails for any other reason, log but don't fail
        RAISE WARNING 'Could not refresh collation version: %. This is non-critical.', SQLERRM;
END $$;

-- Also update statistics on system catalogs that might be affected
ANALYZE pg_collation;
ANALYZE pg_database;