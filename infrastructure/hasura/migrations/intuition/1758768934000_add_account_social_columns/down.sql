-- Remove social media and profile columns from account table
ALTER TABLE account 
DROP COLUMN IF EXISTS twitter,
DROP COLUMN IF EXISTS discord,
DROP COLUMN IF EXISTS github,
DROP COLUMN IF EXISTS telegram,
DROP COLUMN IF EXISTS email,
DROP COLUMN IF EXISTS description,
DROP COLUMN IF EXISTS url,
DROP COLUMN IF EXISTS location;
