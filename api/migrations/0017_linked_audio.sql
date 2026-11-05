alter table audio_tracks add column url text;
-- ponytail: a track with url is played straight from its source and has no chunks here; /store pulls it in and clears url.
