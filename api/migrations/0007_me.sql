-- Личный кабинет: то, что читатель накопил сам, а не то, что лежит
-- в библиотеке. Всё здесь принадлежит только ему и уезжает с ним
-- по `GET /export`.

-- Цитаты. Название книги хранится копией: книгу из библиотеки могут удалить,
-- а выписка читателя от этого пропадать не должна — она его, а не библиотеки.
-- book_id остаётся ссылкой, пока книга есть: по ней цитата открывается
-- в читалке на нужном месте.
create table quotes (
    id         uuid primary key default gen_random_uuid(),
    user_id    uuid        not null references users (id) on delete cascade,
    book_id    uuid                 references books (id) on delete set null,
    title      text        not null,
    text       text        not null,
    -- доля книги, как в reading_progress: страницы зависят от шрифта и окна
    position   text        not null default '0',
    created_at timestamptz not null default now()
);
create index quotes_user_idx on quotes (user_id, created_at desc);

-- Словарь: незнакомые слова из книг на чужом языке.
create table words (
    id         uuid primary key default gen_random_uuid(),
    user_id    uuid        not null references users (id) on delete cascade,
    book_id    uuid                 references books (id) on delete set null,
    word       text        not null,
    -- фраза, в которой слово встретилось: без неё карточка бесполезна
    context    text        not null default '',
    lang       text,
    created_at timestamptz not null default now()
);
-- Одно слово в словаре один раз. lower() — потому что слово в начале
-- предложения и оно же в середине это одно слово.
create unique index words_uniq on words (user_id, lower(word));
create index words_user_idx on words (user_id, created_at desc);

-- Дни, когда читали. reading_progress хранит только текущее место, а стрик
-- и «страниц за неделю» — это история, и её надо копить отдельно.
-- ponytail: одна строка на день, а не событие на переворот. Понадобится
-- «во сколько читаю» — тогда таблица событий и чистка старого.
create table reading_days (
    user_id uuid not null references users (id) on delete cascade,
    day     date not null,
    pages   int  not null default 0,
    primary key (user_id, day)
);
