use std::{collections::HashMap, sync::Arc, sync::Mutex, time::Duration};

use axum::{
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use sha2::Digest;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    parse::Meta,
    tags::{self, Chapter, Kind, Tags},
    AppError, AppState, Result,
};

pub const EXT: &str = "audio";
pub const CHUNK: usize = 8 * 1024 * 1024;
const MAX_TRACK: u64 = 4 << 30;
const MAX_TRACKS: usize = 2000;
const MAX_SECONDS: f64 = 1_000_000.0;
const HEAD: u64 = 4 * 1024 * 1024;
const MAX_TAG: u64 = 16 * 1024 * 1024;
const MAX_MOOV: u64 = 64 * 1024 * 1024;

pub const AUDIO_EXTS: [&str; 10] = [
    "mp3", "m4a", "m4b", "aac", "ogg", "oga", "opus", "flac", "wav", "webm",
];

fn mime(ext: &str) -> &'static str {
    match ext {
        "mp3" => "audio/mpeg",
        "m4a" | "m4b" => "audio/mp4",
        "aac" => "audio/aac",
        "ogg" | "oga" => "audio/ogg",
        "opus" => "audio/ogg; codecs=opus",
        "flac" => "audio/flac",
        "wav" => "audio/wav",
        _ => "audio/webm",
    }
}

fn not_found() -> AppError {
    AppError::new(StatusCode::NOT_FOUND, "Не найдено", "Not found")
}

fn stem(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let stem = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base);
    crate::parse::clean(&stem.replace('_', " "), 300)
}

async fn slice(db: &PgPool, track: Uuid, pos: u64, len: u64) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    while (out.len() as u64) < len {
        let at = pos + out.len() as u64;
        let want = (len - out.len() as u64).min(CHUNK as u64) as i32;
        let row: Option<(Vec<u8>,)> = sqlx::query_as(
            "select substring(data from ($2 - start + 1)::int for $3) from audio_chunks
             where track_id = $1 and start <= $2 order by start desc limit 1",
        )
        .bind(track)
        .bind(at as i64)
        .bind(want)
        .fetch_optional(db)
        .await?;
        match row {
            Some((bytes,)) if !bytes.is_empty() => out.extend_from_slice(&bytes),
            _ => break,
        }
    }
    Ok(out)
}

async fn read_tags(db: &PgPool, track: Uuid, size: u64) -> Result<Tags> {
    let head = slice(db, track, 0, 16).await?;
    let mut found = match tags::kind(&head) {
        Kind::Id3 => {
            let len = tags::id3v2_len(&head).unwrap_or(0) as u64;
            tags::id3v2(&slice(db, track, 0, len.min(MAX_TAG)).await?)
        }
        Kind::Mp4 => mp4_tags(db, track, size).await?,
        Kind::Flac => tags::flac(&slice(db, track, 0, HEAD).await?),
        Kind::Ogg => tags::ogg(&slice(db, track, 0, HEAD).await?),
        Kind::Unknown => Tags::default(),
    };
    if found.title.is_none() && found.album.is_none() && found.artist.is_none() && size > 128 {
        let v1 = tags::id3v1(&slice(db, track, size - 128, 128).await?);
        found = Tags {
            chapters: found.chapters,
            duration: found.duration,
            ..v1
        };
    }
    Ok(found)
}

async fn mp4_tags(db: &PgPool, track: Uuid, size: u64) -> Result<Tags> {
    let mut pos = 0;
    for _ in 0..64 {
        let head = slice(db, track, pos, 16).await?;
        let Some(atom) = tags::top_atom(&head, pos, size) else {
            break;
        };
        if &atom.kind == b"moov" {
            if atom.size > MAX_MOOV {
                break;
            }
            let moov = slice(db, track, pos + atom.head, atom.size - atom.head).await?;
            let (mut found, text) = tags::mp4(&moov);
            if let Some(text) = text {
                let named = chapter_titles(db, track, &text.samples).await?;
                if named.len() >= 2 {
                    found.chapters = named;
                }
            }
            return Ok(found);
        }
        pos += atom.size;
    }
    Ok(Tags::default())
}

async fn chapter_titles(
    db: &PgPool,
    track: Uuid,
    samples: &[(u64, u32, f64)],
) -> Result<Vec<Chapter>> {
    let (Some(lo), Some(hi)) = (
        samples.iter().map(|s| s.0).min(),
        samples.iter().map(|s| s.0 + u64::from(s.1)).max(),
    ) else {
        return Ok(Vec::new());
    };
    let span = if hi - lo <= 1 << 20 {
        slice(db, track, lo, hi - lo).await?
    } else {
        Vec::new()
    };
    let mut out = Vec::new();
    for &(at, len, start) in samples.iter().take(2000) {
        let bytes = if span.is_empty() {
            slice(db, track, at, u64::from(len.min(4096))).await?
        } else {
            let from = (at - lo) as usize;
            span.get(from..from + len as usize)
                .unwrap_or_default()
                .to_vec()
        };
        out.push(Chapter {
            title: tags::text_sample(&bytes),
            start,
        });
    }
    Ok(out)
}

#[derive(Deserialize)]
pub struct NewTrack {
    name: String,
    size: u64,
    #[serde(default)]
    seconds: f64,
}

pub async fn new_track(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(body): Json<NewTrack>,
) -> Result<Json<serde_json::Value>> {
    let ext = body
        .name
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .unwrap_or_default();
    if !AUDIO_EXTS.contains(&ext.as_str()) {
        return Err(AppError::bad(
            format!("Аудиокнига принимается в {}", AUDIO_EXTS.join(", ")),
            format!("Audiobooks are accepted as {}", AUDIO_EXTS.join(", ")),
        ));
    }
    if body.size == 0 || body.size > MAX_TRACK {
        return Err(AppError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Файл пустой или больше 4 ГБ",
            "The file is empty or over 4 GB",
        ));
    }
    let seconds = if body.seconds.is_finite() && body.seconds > 0.0 && body.seconds < MAX_SECONDS {
        body.seconds
    } else {
        0.0
    };

    sqlx::query(
        "delete from audio_tracks where book_id is null and created_at < now() - interval '1 day'",
    )
    .execute(&state.db)
    .await?;
    let (id,): (Uuid,) = sqlx::query_as(
        "insert into audio_tracks (owner_id, title, mime, seconds, size)
         values ($1, $2, $3, $4, $5) returning id",
    )
    .bind(user.id)
    .bind(stem(&body.name))
    .bind(mime(&ext))
    .bind(seconds as f32)
    .bind(body.size as i64)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(serde_json::json!({ "id": id, "chunk": CHUNK })))
}

#[derive(Deserialize)]
pub struct ChunkAt {
    start: u64,
}

pub async fn put_chunk(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Query(at): Query<ChunkAt>,
    body: Bytes,
) -> Result<StatusCode> {
    let size: Option<(i64,)> = sqlx::query_as(
        "select size from audio_tracks where id = $1 and owner_id = $2 and book_id is null",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?;
    let size = size.ok_or_else(not_found)?.0 as u64;
    let end = at.start + body.len() as u64;
    let aligned = at.start % CHUNK as u64 == 0;
    let whole = body.len() == CHUNK || end == size;
    if body.is_empty() || !aligned || !whole || end > size {
        return Err(AppError::bad("Неверный кусок файла", "Bad file chunk"));
    }

    sqlx::query(
        "insert into audio_chunks (track_id, start, data) values ($1, $2, $3)
         on conflict (track_id, start) do update set data = excluded.data",
    )
    .bind(id)
    .bind(at.start as i64)
    .bind(body.as_ref())
    .execute(&state.db)
    .await?;
    if at.start == 0 {
        sqlx::query("update audio_tracks set head_sha = $2 where id = $1")
            .bind(id)
            .bind(sha2::Sha256::digest(&body).to_vec())
            .execute(&state.db)
            .await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct Finish {
    tracks: Vec<Uuid>,
    title: Option<String>,
}

#[derive(Deserialize)]
pub struct Force {
    #[serde(default)]
    force: bool,
}

struct Staged {
    id: Uuid,
    title: String,
    seconds: f64,
    chapters: Vec<Chapter>,
}

pub async fn finish(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Query(force): Query<Force>,
    Json(body): Json<Finish>,
) -> Result<Response> {
    if body.tracks.is_empty() || body.tracks.len() > MAX_TRACKS {
        return Err(AppError::bad("Нет файлов", "No files"));
    }
    type Row = (Uuid, String, f32, i64, Option<Vec<u8>>, i64);
    let rows: Vec<Row> = sqlx::query_as(
        "select t.id, t.title, t.seconds, t.size, t.head_sha,
                coalesce((select sum(octet_length(c.data)) from audio_chunks c where c.track_id = t.id), 0)::int8
         from audio_tracks t
         where t.id = any($1) and t.owner_id = $2 and t.book_id is null",
    )
    .bind(&body.tracks)
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;
    let ordered: Vec<&Row> = body
        .tracks
        .iter()
        .filter_map(|id| rows.iter().find(|r| r.0 == *id))
        .collect();
    if ordered.len() != body.tracks.len() || ordered.iter().any(|r| r.3 != r.5) {
        return Err(AppError::bad(
            "Файлы загружены не до конца",
            "The files have not finished uploading",
        ));
    }

    let first = ordered[0];
    let same: Option<(Uuid,)> = sqlx::query_as(
        "select t.book_id from audio_tracks t
         where t.idx = 0 and t.head_sha = $1 and t.size = $2 and t.book_id is not null
           and (select (count(*), sum(u.size)::int8) from audio_tracks u where u.book_id = t.book_id)
               = ($3::int8, $4::int8)
         limit 1",
    )
    .bind(&first.4)
    .bind(first.3)
    .bind(ordered.len() as i64)
    .bind(ordered.iter().map(|r| r.3).sum::<i64>())
    .fetch_optional(&state.db)
    .await?;
    if let Some((id,)) = same {
        sqlx::query(
            "insert into shelf (user_id, book_id) values ($1, $2) on conflict (user_id, book_id) do nothing",
        )
        .bind(user.id)
        .bind(id)
        .execute(&state.db)
        .await?;
        sqlx::query("delete from audio_tracks where id = any($1)")
            .bind(&body.tracks)
            .execute(&state.db)
            .await?;
        return duplicate(&state, &user, id, "same").await;
    }

    let mut staged = Vec::with_capacity(ordered.len());
    let mut book_tags = Tags::default();
    for (i, row) in ordered.iter().enumerate() {
        let found = read_tags(&state.db, row.0, row.3 as u64).await?;
        let seconds = if row.2 > 0.0 {
            f64::from(row.2)
        } else {
            found.duration.unwrap_or(0.0)
        };
        if seconds <= 0.0 {
            return Err(AppError::bad(
                format!("Не удалось узнать длительность файла «{}»", row.1),
                format!("Could not tell how long «{}» is", row.1),
            ));
        }
        staged.push(Staged {
            id: row.0,
            title: found
                .title
                .as_deref()
                .map(|t| crate::parse::clean(t, 300))
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| row.1.clone()),
            seconds,
            chapters: tags::tidy(found.chapters.clone(), seconds),
        });
        if i == 0 {
            book_tags = found;
        }
    }

    let single = (staged.len() == 1)
        .then(|| book_tags.title.clone())
        .flatten();
    let title = book_tags.album.clone().or(single).or(body.title.clone());
    let meta = Meta {
        title: title.unwrap_or_else(|| ordered[0].1.clone()),
        author: book_tags.author().unwrap_or_default().to_string(),
        published: book_tags.year.clone(),
        lang: lang_code(book_tags.lang.as_deref()),
        description: book_tags.comment.clone(),
        series: None,
    }
    .sanitized();

    if !force.force {
        if let Some(id) =
            crate::books::similar_book(&state.db, user.id, &meta.title, &meta.author, true).await?
        {
            return duplicate(&state, &user, id, "similar").await;
        }
    }

    let id = create(&state, Some(user.id), &staged, meta, None, None, true).await?;
    let book = crate::books::one_book(&state.db, &user, id).await?;
    Ok(Json(serde_json::json!({ "book": book, "duplicate": null })).into_response())
}

async fn duplicate(state: &AppState, user: &AuthUser, id: Uuid, kind: &str) -> Result<Response> {
    let book = crate::books::one_book(&state.db, user, id).await?;
    Ok((
        StatusCode::CONFLICT,
        Json(serde_json::json!({ "book": book, "duplicate": kind })),
    )
        .into_response())
}

pub(crate) fn lang_code(tag: Option<&str>) -> Option<String> {
    let code = crate::legal::lang_code(tag);
    let two = match code.as_str() {
        "rus" => "ru",
        "ukr" => "uk",
        "eng" => "en",
        "pol" => "pl",
        "deu" | "ger" => "de",
        "fra" | "fre" => "fr",
        "spa" => "es",
        "ita" => "it",
        "nld" | "dut" => "nl",
        "por" => "pt",
        c if c.len() == 2 => c,
        _ => return None,
    };
    Some(two.to_string())
}

async fn create(
    state: &AppState,
    owner: Option<Uuid>,
    staged: &[Staged],
    meta: Meta,
    source_url: Option<&str>,
    basis: Option<&str>,
    checked: bool,
) -> Result<Uuid> {
    let found = crate::enrich::on_upload(
        &state.http,
        &meta.title,
        &meta.author,
        meta.lang.as_deref(),
        meta.description.is_none(),
    )
    .await;

    let mut tx = state.db.begin().await?;
    let (id,): (Uuid,) = sqlx::query_as(
        "insert into books
             (title, author, published, lang, description, ext, owner_id, source_url,
              public, public_basis, legal_checked_at)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9 is not null, $9, case when $10 then now() end)
         returning id",
    )
    .bind(&meta.title)
    .bind(&meta.author)
    .bind(&meta.published)
    .bind(&meta.lang)
    .bind(&meta.description)
    .bind(EXT)
    .bind(owner)
    .bind(source_url)
    .bind(basis)
    .bind(checked)
    .fetch_one(&mut *tx)
    .await?;
    if let Some(found) = &found {
        crate::enrich::apply(&mut *tx, id, found).await?;
    }
    for (i, t) in staged.iter().enumerate() {
        sqlx::query(
            "update audio_tracks set book_id = $2, idx = $3, title = $4, seconds = $5, chapters = $6
             where id = $1",
        )
        .bind(t.id)
        .bind(id)
        .bind(i as i32)
        .bind(&t.title)
        .bind(t.seconds as f32)
        .bind(serde_json::to_string(&t.chapters).map_err(AppError::internal)?)
        .execute(&mut *tx)
        .await?;
    }
    if let Some(owner) = owner {
        sqlx::query("insert into shelf (user_id, book_id) values ($1, $2) on conflict do nothing")
            .bind(owner)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(id)
}

pub async fn listen(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(filename): Path<String>,
) -> Result<impl IntoResponse> {
    let (id, _) = crate::books::parse_filename(&filename)?;
    crate::books::ensure_visible(&state.db, &user, id).await?;
    let head: Option<(String, String, Option<String>)> =
        sqlx::query_as("select title, author, lang from books where id = $1 and ext = $2")
            .bind(id)
            .bind(EXT)
            .fetch_optional(&state.db)
            .await?;
    let (title, author, lang) = head.ok_or_else(not_found)?;
    let tracks: Vec<(String, String, f32, i64, String)> = sqlx::query_as(
        "select title, mime, seconds, size, chapters from audio_tracks where book_id = $1 order by idx",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    let tracks: Vec<serde_json::Value> = tracks
        .into_iter()
        .map(|(title, mime, seconds, size, chapters)| {
            serde_json::json!({
                "title": title,
                "mime": mime,
                "seconds": seconds,
                "size": size,
                "chapters": serde_json::from_str::<serde_json::Value>(&chapters)
                    .unwrap_or_else(|_| serde_json::json!([])),
            })
        })
        .collect();
    Ok((
        [(header::CACHE_CONTROL, "private, no-cache")],
        Json(serde_json::json!({
            "title": title,
            "author": author,
            "lang": lang,
            "tracks": tracks,
        })),
    ))
}

#[derive(Deserialize)]
pub struct Signed {
    t: String,
}

pub fn range(h: Option<&str>, size: u64) -> Option<std::result::Result<(u64, u64), ()>> {
    let spec = h?.trim().strip_prefix("bytes=")?;
    let spec = spec.split(',').next()?.trim();
    let (a, b) = spec.split_once('-')?;
    let parsed = match (a.trim(), b.trim()) {
        ("", n) => n
            .parse::<u64>()
            .ok()
            .filter(|n| *n > 0)
            .map(|n| (size.saturating_sub(n), size - 1)),
        (a, "") => a.parse().ok().map(|a| (a, size - 1)),
        (a, b) => a
            .parse()
            .ok()
            .zip(b.parse::<u64>().ok())
            .map(|(a, b)| (a, b.min(size - 1))),
    };
    Some(match parsed {
        Some((a, b)) if a <= b && b < size => Ok((a, b)),
        _ => Err(()),
    })
}

pub async fn stream(
    State(state): State<Arc<AppState>>,
    Path((id, idx)): Path<(Uuid, i32)>,
    Query(signed): Query<Signed>,
    headers: HeaderMap,
) -> Result<Response> {
    if crate::auth::book_of_token(&state.jwt_secret, &signed.t) != Some(id) {
        return Err(AppError::new(
            StatusCode::UNAUTHORIZED,
            "Нужен вход",
            "Not authorized",
        ));
    }
    let row: Option<(Uuid, String, i64)> =
        sqlx::query_as("select id, mime, size from audio_tracks where book_id = $1 and idx = $2")
            .bind(id)
            .bind(idx)
            .fetch_optional(&state.db)
            .await?;
    let (track, mime, size) = row.ok_or_else(not_found)?;
    let size = size as u64;

    let asked = range(
        headers.get(header::RANGE).and_then(|v| v.to_str().ok()),
        size,
    );
    let (status, from, to) = match asked {
        None => (StatusCode::OK, 0, size - 1),
        Some(Ok((a, b))) => (StatusCode::PARTIAL_CONTENT, a, b),
        Some(Err(())) => {
            return Ok((
                StatusCode::RANGE_NOT_SATISFIABLE,
                [(header::CONTENT_RANGE, format!("bytes */{size}"))],
            )
                .into_response())
        }
    };

    let db = state.db.clone();
    let body = futures_util::stream::unfold(from, move |pos| {
        let db = db.clone();
        async move {
            if pos > to {
                return None;
            }
            let want = (to + 1 - pos).min(CHUNK as u64);
            match slice(&db, track, pos, want).await {
                Ok(bytes) if !bytes.is_empty() => {
                    let next = pos + bytes.len() as u64;
                    Some((Ok::<_, std::io::Error>(Bytes::from(bytes)), next))
                }
                Ok(_) => None,
                Err(e) => Some((Err(std::io::Error::other(e.to_string())), to + 1)),
            }
        }
    });

    let mut response = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CONTENT_LENGTH, to + 1 - from)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "private, max-age=86400");
    if status == StatusCode::PARTIAL_CONTENT {
        response = response.header(header::CONTENT_RANGE, format!("bytes {from}-{to}/{size}"));
    }
    response
        .body(Body::from_stream(body))
        .map_err(AppError::internal)
}

#[derive(Deserialize)]
pub struct Search {
    q: Option<String>,
}

pub async fn search(
    State(state): State<Arc<AppState>>,
    _user: AuthUser,
    Query(search): Query<Search>,
) -> Json<Vec<crate::sources::FoundAudio>> {
    let q = search.q.unwrap_or_default();
    let q = q.trim();
    if q.is_empty() {
        return Json(Vec::new());
    }
    Json(crate::sources::search_audio(&state.http, q).await)
}

// ponytail: LibriVox imports in flight (false) and failed ones (true) live in process memory;
// a restart forgets them, and abandoned staged tracks are swept a day later like any unfinished upload.
static IMPORTS: Mutex<Option<HashMap<String, bool>>> = Mutex::new(None);

fn imports<T>(f: impl FnOnce(&mut HashMap<String, bool>) -> T) -> T {
    f(IMPORTS.lock().unwrap().get_or_insert_with(HashMap::new))
}

#[derive(Deserialize)]
pub struct ImportAudio {
    id: String,
}

pub async fn import(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(body): Json<ImportAudio>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let Some((from, key)) = origin(&body.id) else {
        return Err(AppError::bad(
            "Неверный адрес книги",
            "Invalid book address",
        ));
    };
    let id = format!("{from}:{key}");
    let pending = || {
        Ok((
            StatusCode::ACCEPTED,
            Json(serde_json::json!({ "pending": true })),
        ))
    };
    match imports(|m| m.get(&id).cloned()) {
        Some(false) => return pending(),
        Some(true) => {
            imports(|m| m.remove(&id));
            return Err(AppError::new(
                StatusCode::BAD_GATEWAY,
                "Не удалось скачать книгу из источника, попробуйте ещё раз",
                "Could not download the book from its source, try again",
            ));
        }
        None => {}
    }

    let book = match from {
        "wl" => crate::open::wolnelektury_book(&state.http, key).await,
        _ => crate::sources::librivox_book(&state.http, key).await,
    }
    .map_err(|e| {
        AppError::bad(
            format!("Источник не ответил: {e}"),
            format!("The source did not respond: {e}"),
        )
    })?;

    let have: Option<(Uuid,)> =
        sqlx::query_as("select id from books where source_url = $1 and ext = $2")
            .bind(&book.page_url)
            .bind(EXT)
            .fetch_optional(&state.db)
            .await?;
    if let Some((found,)) = have {
        sqlx::query("insert into shelf (user_id, book_id) values ($1, $2) on conflict do nothing")
            .bind(user.id)
            .bind(found)
            .execute(&state.db)
            .await?;
        return Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "location": found })),
        ));
    }

    if book.sections.is_empty()
        || book.sections.len() > MAX_TRACKS
        || book.sections.iter().any(|s| {
            !s.url
                .parse()
                .is_ok_and(|u| crate::sources::host_allowed(&u))
        })
    {
        return Err(AppError::bad(
            "У этой книги нет файлов, которые можно забрать",
            "This book has no files that can be fetched",
        ));
    }
    if imports(|m| m.insert(id.clone(), false).is_none()) {
        tokio::spawn(async move {
            match fetch(&state, Some(user.id), &book).await {
                Ok(book_id) => {
                    imports(|m| m.remove(&id));
                    if book.basis.is_none() {
                        crate::legal::check(state.clone(), book_id).await;
                    }
                }
                Err(e) => {
                    tracing::warn!("audio import {id}: {e}");
                    imports(|m| m.insert(id, true));
                }
            };
        });
    }
    pending()
}

pub fn origin(id: &str) -> Option<(&'static str, &str)> {
    let (from, key) = id.split_once(':').unwrap_or(("lv", id));
    match from {
        "lv" if !key.is_empty() && key.len() <= 12 && key.chars().all(|c| c.is_ascii_digit()) => {
            Some(("lv", key))
        }
        "wl" if crate::open::wl_slug(key) => Some(("wl", key)),
        _ => None,
    }
}

pub(crate) async fn seed(state: &Arc<AppState>, key: &str) -> Result<()> {
    let Some(("wl", slug)) = origin(key) else {
        return Err(AppError::bad(
            "Неверный адрес книги",
            "Invalid book address",
        ));
    };
    let book = crate::open::wolnelektury_book(&state.http, slug).await?;
    let have: Option<(Uuid,)> =
        sqlx::query_as("select id from books where source_url = $1 and ext = $2")
            .bind(&book.page_url)
            .bind(EXT)
            .fetch_optional(&state.db)
            .await?;
    if have.is_none() {
        fetch(state, None, &book).await?;
    }
    Ok(())
}

async fn fetch(
    state: &AppState,
    owner: Option<Uuid>,
    book: &crate::sources::Audiobook,
) -> Result<Uuid> {
    let mut staged = Vec::new();
    let result = async {
        for s in &book.sections {
            staged.push(download(state, owner, s).await?);
        }
        let meta = Meta {
            title: book.title.clone(),
            author: book.author.clone(),
            lang: book.lang.clone(),
            description: book.description.clone(),
            ..Default::default()
        }
        .sanitized();
        create(
            state,
            owner,
            &staged,
            meta,
            Some(&book.page_url),
            book.basis.as_deref(),
            book.basis.is_some(),
        )
        .await
    }
    .await;
    if result.is_err() {
        let ids: Vec<Uuid> = staged.iter().map(|s| s.id).collect();
        sqlx::query("delete from audio_tracks where id = any($1) and book_id is null")
            .bind(&ids)
            .execute(&state.db)
            .await?;
    }
    result
}

async fn download(
    state: &AppState,
    owner: Option<Uuid>,
    s: &crate::sources::Section,
) -> Result<Staged> {
    let (id,): (Uuid,) = sqlx::query_as(
        "insert into audio_tracks (owner_id, title, mime, seconds, size)
         values ($1, $2, 'audio/mpeg', $3, 0) returning id",
    )
    .bind(owner)
    .bind(crate::parse::clean(&s.title, 300))
    .bind(s.seconds as f32)
    .fetch_one(&state.db)
    .await?;

    let mut attempt = 1;
    let size = loop {
        match pull(state, id, &s.url).await {
            Ok(size) => break size,
            Err(e) if attempt < 3 => {
                tracing::warn!("audio file {}, attempt {attempt}: {e}", s.url);
                sqlx::query("delete from audio_chunks where track_id = $1")
                    .bind(id)
                    .execute(&state.db)
                    .await?;
                tokio::time::sleep(Duration::from_secs(5 * attempt)).await;
                attempt += 1;
            }
            Err(e) => {
                sqlx::query("delete from audio_tracks where id = $1")
                    .bind(id)
                    .execute(&state.db)
                    .await?;
                return Err(e);
            }
        }
    };
    sqlx::query("update audio_tracks set size = $2 where id = $1")
        .bind(id)
        .bind(size as i64)
        .execute(&state.db)
        .await?;
    let found = read_tags(&state.db, id, size).await?;
    Ok(Staged {
        id,
        title: crate::parse::clean(&s.title, 300),
        seconds: s.seconds,
        chapters: tags::tidy(found.chapters, s.seconds),
    })
}

async fn pull(state: &AppState, id: Uuid, url: &str) -> Result<u64> {
    let mut response = state
        .http
        .get(url)
        .timeout(Duration::from_secs(1800))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(AppError::internal)?;
    let mut buf = Vec::with_capacity(CHUNK);
    let mut start = 0u64;
    loop {
        let piece = response.chunk().await.map_err(AppError::internal)?;
        if let Some(p) = &piece {
            buf.extend_from_slice(p);
        }
        while buf.len() >= CHUNK || (piece.is_none() && !buf.is_empty()) {
            let take = buf.len().min(CHUNK);
            let part: Vec<u8> = buf.drain(..take).collect();
            if start == 0 {
                sqlx::query("update audio_tracks set head_sha = $2 where id = $1")
                    .bind(id)
                    .bind(sha2::Sha256::digest(&part).to_vec())
                    .execute(&state.db)
                    .await?;
            }
            sqlx::query("insert into audio_chunks (track_id, start, data) values ($1, $2, $3)")
                .bind(id)
                .bind(start as i64)
                .bind(&part)
                .execute(&state.db)
                .await?;
            start += part.len() as u64;
            if start > MAX_TRACK {
                return Err(AppError::bad("Файл больше 4 ГБ", "The file is over 4 GB"));
            }
        }
        if piece.is_none() {
            break;
        }
    }
    Ok(start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_follow_rfc_7233() {
        assert_eq!(range(None, 100), None);
        assert_eq!(range(Some("bytes=0-1"), 100), Some(Ok((0, 1))));
        assert_eq!(range(Some("bytes=10-"), 100), Some(Ok((10, 99))));
        assert_eq!(range(Some("bytes=-10"), 100), Some(Ok((90, 99))));
        assert_eq!(range(Some("bytes=90-500"), 100), Some(Ok((90, 99))));
        assert_eq!(range(Some("bytes=0-0,5-9"), 100), Some(Ok((0, 0))));
        assert_eq!(range(Some("bytes=100-"), 100), Some(Err(())));
        assert_eq!(range(Some("bytes=5-1"), 100), Some(Err(())));
        assert_eq!(range(Some("bytes=-0"), 100), Some(Err(())));
        assert_eq!(range(Some("items=0-1"), 100), None);
        assert_eq!(range(Some("bytes=x-1"), 100), Some(Err(())));
    }

    #[test]
    fn import_ids_name_their_source() {
        assert_eq!(origin("271"), Some(("lv", "271")));
        assert_eq!(origin("lv:271"), Some(("lv", "271")));
        assert_eq!(origin("wl:pan-tadeusz"), Some(("wl", "pan-tadeusz")));
        for bad in [
            "",
            "lv:",
            "lv:12a",
            "wl:../x",
            "wl:",
            "xx:1",
            "1234567890123",
        ] {
            assert_eq!(origin(bad), None, "{bad}");
        }
    }

    #[test]
    fn three_letter_languages_become_two() {
        assert_eq!(lang_code(Some("rus")).as_deref(), Some("ru"));
        assert_eq!(lang_code(Some("en-US")).as_deref(), Some("en"));
        assert_eq!(lang_code(Some("xxx")), None);
        assert_eq!(lang_code(None), None);
    }

    #[test]
    fn file_names_become_titles() {
        assert_eq!(stem("Books/01_Глава_первая.mp3"), "01 Глава первая");
        assert_eq!(stem("C:\\a\\b.m4b"), "b");
    }

    #[tokio::test]
    #[ignore = "needs DATABASE_URL pointing at a throwaway Postgres"]
    async fn chunks_are_read_back_across_their_borders() {
        let db = PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();
        sqlx::migrate!().run(&db).await.unwrap();
        let (track,): (Uuid,) = sqlx::query_as(
            "insert into audio_tracks (title, mime, seconds, size) values ('t', 'audio/mpeg', 1, 25) returning id",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        for (start, data) in [
            (0i64, &b"0123456789"[..]),
            (10, b"abcdefghij"),
            (20, b"KLMNO"),
        ] {
            sqlx::query("insert into audio_chunks (track_id, start, data) values ($1, $2, $3)")
                .bind(track)
                .bind(start)
                .bind(data)
                .execute(&db)
                .await
                .unwrap();
        }
        assert_eq!(slice(&db, track, 0, 3).await.unwrap(), b"012");
        assert_eq!(slice(&db, track, 8, 14).await.unwrap(), b"89abcdefghijKL");
        assert_eq!(slice(&db, track, 22, 100).await.unwrap(), b"MNO");
        assert!(slice(&db, track, 25, 5).await.unwrap().is_empty());
        sqlx::query("delete from audio_tracks where id = $1")
            .bind(track)
            .execute(&db)
            .await
            .unwrap();
    }
}
