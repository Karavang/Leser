alter table book_files add column sha256 bytea;
update book_files set sha256 = sha256(data);
alter table book_files alter column sha256 set not null;
-- ponytail: not unique, because libraries from before this migration may already hold copies.
-- Two identical uploads in the same second can both pass the check; make it unique once old copies are cleaned up.
create index book_files_sha256_idx on book_files (sha256);
