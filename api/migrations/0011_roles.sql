alter table users add column role text not null default 'user'
    check (role in ('admin', 'full', 'user'));
-- ponytail: everyone registered before roles existed came in as a friend, so they keep the whole library.
update users set role = case when is_admin then 'admin' else 'full' end;
alter table users drop column is_admin;

alter table books add column public boolean not null default false;

create table invites (
    token      text primary key,
    created_by uuid references users (id) on delete set null,
    created_at timestamptz not null default now(),
    used_by    uuid references users (id) on delete set null,
    used_at    timestamptz
);
