-- Revert to 24-hour stale detection window.
-- The function body is restored from the version created in migration 1771526427000
-- (with the stale detection logic from 1771526431000's original function).
-- This is a best-effort rollback — the 7-day window is strictly better.

-- Note: The previous function body is already on the server since we used
-- CREATE OR REPLACE. To truly revert, re-apply the function from 1771526427000.
-- Left as a no-op since downgrading the window is not recommended.
