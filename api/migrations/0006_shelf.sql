-- «Мои книги» — полка читателя, а не список того, кто что загрузил.
--
-- Раньше «моей» книга считалась, если читатель её принёс (books.owner_id).
-- Но библиотека общая: чужую книгу хочется отложить себе, а свою — убрать
-- с полки, не удаляя из библиотеки. Это разные вещи, поэтому и таблицы разные:
-- owner_id остаётся и продолжает решать права на удаление, полка — только
-- про то, что читатель хочет видеть в своём разделе.
create table shelf (
    user_id  uuid        not null references users (id) on delete cascade,
    book_id  uuid        not null references books (id) on delete cascade,
    added_at timestamptz not null default now(),
    -- ponytail: отдельного индекса по book_id нет — все запросы идут от
    -- читателя (`where user_id = $1`), а для этого хватает первичного ключа.
    primary key (user_id, book_id)
);

-- Кто книгу принёс, у того она в «моих» и была: полка не должна опустеть
-- от появления самой полки.
insert into shelf (user_id, book_id, added_at)
select owner_id, id, created_at from books where owner_id is not null;
