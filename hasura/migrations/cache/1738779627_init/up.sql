CREATE SCHEMA IF NOT EXISTS cached_images;

CREATE TABLE IF NOT EXISTS cached_images.cached_image (
  -- id is the original name of the image in lowercase without the extension
  url TEXT PRIMARY KEY NOT NULL,
  original_url TEXT NOT NULL,
  score JSONB,
  model TEXT,
  safe BOOLEAN NOT NULL DEFAULT false,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_cached_image_original_url ON cached_images.cached_image(original_url);
CREATE INDEX IF NOT EXISTS idx_cached_image_url ON cached_images.cached_image(url);