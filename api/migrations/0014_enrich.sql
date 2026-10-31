alter table books add column enriched_at timestamptz;
alter table books add column series_index real;
alter table books add column genres text[] not null default '{}';
alter table books drop column series_checked_at;
