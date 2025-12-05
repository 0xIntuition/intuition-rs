-- Add Caip22 to atom_type enum for NFT asset identifier support (CAIP-22)
-- This enables ingestion of AgentRegistry entries and other NFTs via tokenURI resolution

ALTER TYPE atom_type ADD VALUE IF NOT EXISTS 'Caip22';
