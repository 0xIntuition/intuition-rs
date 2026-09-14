\t on
\a
select term_id from (
 select term_id, created_at, case
   when resolving_status='Resolved' and type='Unknown' then 1
   when resolving_status='Pending' and exists (select 1 from atom_value v where v.id=a.term_id) then 2
   when resolving_status='Pending' then 3
   when resolving_status='Failed' then 4 end cls
 from atom a
 where data is not null and data <> ''
   and ((resolving_status='Resolved' and type='Unknown') or resolving_status in ('Pending','Failed'))
) s order by cls, created_at desc;
