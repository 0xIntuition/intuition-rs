-- Down migration to remove populated data from triple tables

-- Remove all data from triple_vault table
DELETE FROM triple_vault;

-- Remove all data from triple_term table  
DELETE FROM triple_term; 