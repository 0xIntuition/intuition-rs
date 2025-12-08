-- Note: PostgreSQL does not support removing values from an enum type.
-- To fully rollback, you would need to:
-- 1. Create a new enum type without Caip22
-- 2. Update all columns using the old type to use the new type
-- 3. Drop the old type and rename the new one
--
-- Since this is destructive and complex, this down migration is left as a no-op.
-- If you need to rollback, manually ensure no atoms have type 'Caip22' before
-- performing a full enum type migration.

SELECT 1; -- no-op
