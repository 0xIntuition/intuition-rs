CREATE TABLE IF NOT EXISTS public.share_price_aggregate (
    id BIGSERIAL PRIMARY KEY,
    vault_id NUMERIC NOT NULL,
    current_share_price NUMERIC NOT NULL,
    last_time_updated TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (vault_id) REFERENCES public.curve_vault(id)
);

-- Index for efficient time-series queries
CREATE INDEX IF NOT EXISTS share_price_aggregate_time_idx 
ON public.share_price_aggregate(last_time_updated DESC);

-- Index for looking up price history by vault
CREATE INDEX IF NOT EXISTS share_price_aggregate_vault_time_idx 
ON public.share_price_aggregate(vault_id, last_time_updated DESC);

-- Add comments for documentation
COMMENT ON TABLE public.share_price_aggregate IS 'Historical tracking of share prices for curve vaults';
COMMENT ON COLUMN public.share_price_aggregate.id IS 'Unique identifier for the price record';
COMMENT ON COLUMN public.share_price_aggregate.vault_id IS 'Reference to the curve vault';
COMMENT ON COLUMN public.share_price_aggregate.current_share_price IS 'Share price at this point in time';
COMMENT ON COLUMN public.share_price_aggregate.last_time_updated IS 'Timestamp when this price was recorded'; 