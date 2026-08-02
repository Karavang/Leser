create table users (
    id            uuid primary key default gen_random_uuid(),
    username      text        not null,
    email         text        not null unique,
    password_hash text        not null,
    is_admin      boolean     not null default false,
    created_at    timestamptz not null default now()
);

create table books (
    id          uuid primary key default gen_random_uuid(),
    title       text        not null,
    author      text        not null default '',
    published   text,
    lang        text,
    description text,
    ext         text        not null,
    owner_id    uuid references users (id) on delete set null,
    created_at  timestamptz not null default now()
);

create index books_owner_idx on books (owner_id);

-- Файл лежит отдельной таблицей, чтобы select по books никогда не тащил байты книги.
-- Каскад = удаление книги и файла в одной транзакции, осиротеть нечему.
-- ponytail: bytea целиком читается в память, потолок — MAX_UPLOAD (64 МБ).
-- Понадобятся книги крупнее или отдача тысячам читателей сразу — тогда
-- pg_largeobject с чтением по кускам или отдельный том, раздаваемый nginx'ом.
create table book_files (
    book_id uuid  primary key references books (id) on delete cascade,
    data    bytea not null
);

-- заменяет users.pages[] из Mongo: один upsert вместо перезаписи всего массива
create table reading_progress (
    user_id    uuid        not null references users (id) on delete cascade,
    book_id    uuid        not null references books (id) on delete cascade,
    position   text        not null,
    updated_at timestamptz not null default now(),
    primary key (user_id, book_id)
);
