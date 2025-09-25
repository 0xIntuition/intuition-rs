-- Add social media and profile columns to account table
ALTER TABLE account 
ADD COLUMN twitter TEXT,
ADD COLUMN discord TEXT,
ADD COLUMN github TEXT,
ADD COLUMN telegram TEXT,
ADD COLUMN email TEXT,
ADD COLUMN description TEXT,
ADD COLUMN url TEXT,
ADD COLUMN location TEXT,
ADD COLUMN real_name TEXT;

