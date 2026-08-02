-- Серия ("Гарри Поттер", "Плоский мир") — третье поле, по которому ищут книгу,
-- наравне с названием и автором. У большинства книг её нет, отсюда nullable.
alter table books add column series text;

-- ponytail: индекса под поиск нет. `ilike '%q%'` открыт с обеих сторон, btree
-- на нём бесполезен, а на нынешней библиотеке последовательный скан по трём
-- полям — это микросекунды. Начнёт тормозить (тысячи книг) — pg_trgm:
--   create extension pg_trgm;
--   create index books_search_idx on books using gin (
--       (title || ' ' || author || ' ' || coalesce(series,'')) gin_trgm_ops);
