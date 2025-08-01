-- Add updated_at column to term table
ALTER TABLE term ADD COLUMN updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(); 