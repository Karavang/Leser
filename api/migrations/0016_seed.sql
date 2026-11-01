create table seeded (
    key text primary key,
    at  timestamptz not null default now()
);
