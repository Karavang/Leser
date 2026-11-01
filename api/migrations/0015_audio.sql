create table audio_tracks (
    id         uuid primary key default gen_random_uuid(),
    book_id    uuid references books (id) on delete cascade,
    owner_id   uuid references users (id) on delete set null,
    idx        integer     not null default 0,
    title      text        not null,
    mime       text        not null,
    seconds    real        not null,
    size       bigint      not null,
    head_sha   bytea,
    chapters   text        not null default '[]',
    created_at timestamptz not null default now()
);
create index audio_tracks_book_idx on audio_tracks (book_id, idx);
create index audio_tracks_head_idx on audio_tracks (head_sha) where idx = 0;
-- ponytail: tracks with book_id null are uploads in progress; anything older than a day is swept on the next upload.
create index audio_tracks_staged_idx on audio_tracks (created_at) where book_id is null;

create table audio_chunks (
    track_id uuid   not null references audio_tracks (id) on delete cascade,
    start    bigint not null,
    data     bytea  not null,
    primary key (track_id, start)
);
-- ponytail: audio is already compressed, and uncompressed out-of-line storage lets substring() read a range without detoasting the whole chunk.
alter table audio_chunks alter column data set storage external;
