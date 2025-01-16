-- Unfortunately, PostgreSQL doesn't allow removing enum values directly. The down migration requires recreating the type.
CREATE TYPE atom_type_new AS ENUM (
  'Unknown', 'Account', 'Thing', 'ThingPredicate', 'Person', 'PersonPredicate',
  'Organization', 'OrganizationPredicate', 'Book', 'LikeAction', 'FollowAction', 'Keywords'
);

ALTER TABLE ${BACKEND_SCHEMA_NAME}.atoms 
  ALTER COLUMN type TYPE atom_type_new 
  USING (type::text::atom_type_new);

DROP TYPE atom_type;
ALTER TYPE atom_type_new RENAME TO atom_type;

DROP TABLE ${BACKEND_SCHEMA_NAME}.caip10;