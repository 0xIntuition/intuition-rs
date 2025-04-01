CREATE OR REPLACE FUNCTION public.get_curve_id(term_id text)
RETURNS numeric(78, 0)
AS $$
  SELECT curve_id FROM public.vault WHERE id::text = term_id;
$$ LANGUAGE sql STABLE;

CREATE OR REPLACE FUNCTION public.share_price_changed_by_term_curve_id(curve_id numeric(78, 0))
RETURNS SETOF public.share_price_changed
AS $$
  SELECT spc.*
  FROM public.share_price_changed spc
  JOIN public.vault v ON v.id::text = spc.term_id
  WHERE v.curve_id = $1
  ORDER BY spc.block_timestamp DESC;
$$ LANGUAGE sql STABLE; 