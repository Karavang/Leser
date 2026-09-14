//! Личный кабинет: цитаты, словарь, статистика и выгрузка всего своего.
//!
//! Всё, что здесь лежит, читатель накопил сам. Отсюда два правила, которые
//! стоит держать в голове при правках:
//!
//! * Записи переживают книгу. Библиотека общая, книгу может удалить владелец
//!   или админ — выписка читателя при этом остаётся (`on delete set null`
//!   и копия названия).
//! * Всё это выгружается одним `GET /export`. Появится новая таблица про
//!   читателя — её место в `export`, иначе выгрузка перестанет быть полной.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use sqlx::types::chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

use crate::{auth::AuthUser, books::parse_filename, AppError, AppState, Result};

/// Потолки на то, что приходит от клиента. Цитата — это абзац-другой, слово —
/// это слово; всё, что больше, к делу не относится и место в базе занимать
/// не должно. Обрезаем молча: длина выделения — не та ошибка, из-за которой
/// стоит терять уже сделанную выписку.
const MAX_QUOTE: usize = 4096;
const MAX_WORD: usize = 64;
const MAX_CONTEXT: usize = 512;

/// Обрезка по символам, а не по байтам: `String::truncate` на границе
/// многобайтной буквы паникует, а книги здесь чаще не английские.
fn cut(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars()
        .take(max)
        .collect::<String>()
        .trim_end()
        .to_string()
}

// ── цитаты ────────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct QuoteRow {
    id: Uuid,
    title: String,
    text: String,
    position: String,
    created_at: DateTime<Utc>,
    /// null, если книгу успели удалить из библиотеки — цитата остаётся
    book_id: Option<Uuid>,
    ext: Option<String>,
}

impl QuoteRow {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "title": self.title,
            "text": self.text,
            "position": self.position,
            "createdAt": self.created_at.to_rfc3339(),
            // есть только пока книга в библиотеке: без неё открывать нечего
            "filename": match (&self.book_id, &self.ext) {
                (Some(id), Some(ext)) => Some(format!("{id}.{ext}")),
                _ => None,
            },
        })
    }
}

const QUOTE_SELECT: &str = "select q.id, q.title, q.text, q.position, q.created_at,
            q.book_id, b.ext
     from quotes q left join books b on b.id = q.book_id
     where q.user_id = $1 order by q.created_at desc";

pub async fn quotes(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
) -> Result<Json<Vec<serde_json::Value>>> {
    let rows: Vec<QuoteRow> = sqlx::query_as(QUOTE_SELECT)
        .bind(user.id)
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows.iter().map(QuoteRow::to_json).collect()))
}

#[derive(Deserialize)]
pub struct QuoteBody {
    filename: String,
    text: String,
    /// доля книги; не прислали — цитата просто откроет книгу с начала
    position: Option<String>,
}

pub async fn add_quote(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(body): Json<QuoteBody>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let (book_id, _) = parse_filename(&body.filename)?;
    let text = cut(&body.text, MAX_QUOTE);
    if text.is_empty() {
        return Err(AppError::bad("Пустая цитата", "Empty quote"));
    }

    // Название копией, а не джойном: книгу могут удалить, цитату — нет.
    let title: Option<(String,)> = sqlx::query_as("select title from books where id = $1")
        .bind(book_id)
        .fetch_optional(&state.db)
        .await?;
    let (title,) =
        title.ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, "Не найдено", "Not found"))?;

    let row: QuoteRow = sqlx::query_as(
        "with saved as (
             insert into quotes (user_id, book_id, title, text, position)
             values ($1, $2, $3, $4, coalesce($5, '0')) returning *
         )
         select q.id, q.title, q.text, q.position, q.created_at, q.book_id, b.ext
         from saved q left join books b on b.id = q.book_id",
    )
    .bind(user.id)
    .bind(book_id)
    .bind(&title)
    .bind(&text)
    .bind(body.position.as_deref())
    .fetch_one(&state.db)
    .await?;

    Ok((StatusCode::CREATED, Json(row.to_json())))
}

pub async fn delete_quote(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    // user_id в условии — чужую цитату не удалить, даже зная её id
    sqlx::query("delete from quotes where id = $1 and user_id = $2")
        .bind(id)
        .bind(user.id)
        .execute(&state.db)
        .await?;
    // Нет — значит уже нет: повторное удаление это тот же результат.
    Ok(StatusCode::NO_CONTENT)
}

// ── словарь ───────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct WordRow {
    id: Uuid,
    word: String,
    context: String,
    lang: Option<String>,
    created_at: DateTime<Utc>,
}

impl WordRow {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "word": self.word,
            "context": self.context,
            "lang": self.lang,
            "createdAt": self.created_at.to_rfc3339(),
        })
    }
}

const WORD_SELECT: &str = "select id, word, context, lang, created_at
     from words where user_id = $1 order by created_at desc";

pub async fn words(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
) -> Result<Json<Vec<serde_json::Value>>> {
    let rows: Vec<WordRow> = sqlx::query_as(WORD_SELECT)
        .bind(user.id)
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows.iter().map(WordRow::to_json).collect()))
}

#[derive(Deserialize)]
pub struct WordBody {
    filename: String,
    word: String,
    context: Option<String>,
}

pub async fn add_word(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(body): Json<WordBody>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let (book_id, _) = parse_filename(&body.filename)?;
    let word = cut(&body.word, MAX_WORD);
    if word.is_empty() {
        return Err(AppError::bad("Пустое слово", "Empty word"));
    }

    // Язык слова — это язык книги, из которой его выписали: сам по себе
    // словарь языка не знает, а карточке он нужен.
    let lang: Option<(Option<String>,)> = sqlx::query_as("select lang from books where id = $1")
        .bind(book_id)
        .fetch_optional(&state.db)
        .await?;
    let (lang,) =
        lang.ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, "Не найдено", "Not found"))?;

    // Слово уже в словаре — обновляем пример: второй раз его встретили
    // в более понятном месте, чем первый.
    let row: WordRow = sqlx::query_as(
        "insert into words (user_id, book_id, word, context, lang)
         values ($1, $2, $3, $4, $5)
         on conflict (user_id, lower(word))
         do update set context = excluded.context, created_at = now()
         returning id, word, context, lang, created_at",
    )
    .bind(user.id)
    .bind(book_id)
    .bind(&word)
    .bind(cut(
        body.context.as_deref().unwrap_or_default(),
        MAX_CONTEXT,
    ))
    .bind(&lang)
    .fetch_one(&state.db)
    .await?;

    Ok((StatusCode::CREATED, Json(row.to_json())))
}

pub async fn delete_word(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    sqlx::query("delete from words where id = $1 and user_id = $2")
        .bind(id)
        .bind(user.id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── статистика ────────────────────────────────────────────────────────────

/// Сколько дней подряд читали, считая от сегодня. Сегодня ещё не читали —
/// стрик держится вчерашним днём: он обрывается пропущенным днём, а не тем,
/// что сейчас утро.
fn streak(days: &[NaiveDate], today: NaiveDate) -> i64 {
    let mut expected = match days.last() {
        Some(&last) if last == today || last == today.pred_opt().unwrap_or(today) => last,
        _ => return 0,
    };
    let mut count = 0;
    for &day in days.iter().rev() {
        if day != expected {
            break;
        }
        count += 1;
        expected = match expected.pred_opt() {
            Some(d) => d,
            None => break,
        };
    }
    count
}

/// Книга считается прочитанной, когда до конца осталось меньше страницы
/// с небольшим запасом: последний экран часто наполовину пустой, и ровно
/// единицы там не бывает.
const FINISHED: f64 = 0.98;

/// Позиция в книге — строка: у закладок из прежней читалки там epubcfi,
/// числом он не притворяется. Всё, что не разбирается, считаем началом.
fn frac(position: &str) -> f64 {
    position.parse::<f64>().unwrap_or(0.0).clamp(0.0, 1.0)
}

pub async fn stats(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
) -> Result<Json<serde_json::Value>> {
    // Год — чтобы стрик было на чём считать, а график рисует хвост сам.
    let rows: Vec<(NaiveDate, i32)> = sqlx::query_as(
        "select day, pages from reading_days
         where user_id = $1 and day > current_date - 365 order by day",
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;

    let (today,): (NaiveDate,) = sqlx::query_as("select current_date")
        .fetch_one(&state.db)
        .await?;

    let positions: Vec<(String,)> =
        sqlx::query_as("select position from reading_progress where user_id = $1")
            .bind(user.id)
            .fetch_all(&state.db)
            .await?;
    let finished = positions.iter().filter(|(p,)| frac(p) >= FINISHED).count();

    let days: Vec<NaiveDate> = rows.iter().map(|(d, _)| *d).collect();
    Ok(Json(serde_json::json!({
        "streak": streak(&days, today),
        "today": today.to_string(),
        "days": rows.iter()
            .map(|(d, p)| serde_json::json!({ "day": d.to_string(), "pages": p }))
            .collect::<Vec<_>>(),
        "reading": positions.len() - finished,
        "finished": finished,
    })))
}

// ── выгрузка ──────────────────────────────────────────────────────────────

/// Всё, что накопил читатель, одним файлом. Смысл ровно в том, что этот
/// файл можно унести: закладки, полка, цитаты и словарь — его, а не наши.
pub async fn export(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
) -> Result<impl IntoResponse> {
    let (username, email): (String, String) =
        sqlx::query_as("select username, email from users where id = $1")
            .bind(user.id)
            .fetch_one(&state.db)
            .await?;

    let shelf: Vec<(String, String, DateTime<Utc>)> = sqlx::query_as(
        "select b.title, b.author, s.added_at from shelf s
         join books b on b.id = s.book_id
         where s.user_id = $1 order by s.added_at desc",
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;

    let progress: Vec<(String, String, String, DateTime<Utc>)> = sqlx::query_as(
        "select b.title, b.author, p.position, p.updated_at from reading_progress p
         join books b on b.id = p.book_id
         where p.user_id = $1 order by p.updated_at desc",
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;

    let quotes: Vec<QuoteRow> = sqlx::query_as(QUOTE_SELECT)
        .bind(user.id)
        .fetch_all(&state.db)
        .await?;
    let words: Vec<WordRow> = sqlx::query_as(WORD_SELECT)
        .bind(user.id)
        .fetch_all(&state.db)
        .await?;
    let days: Vec<(NaiveDate, i32)> =
        sqlx::query_as("select day, pages from reading_days where user_id = $1 order by day")
            .bind(user.id)
            .fetch_all(&state.db)
            .await?;

    let body = serde_json::json!({
        "leser": 1,
        "user": { "username": username, "email": email },
        "shelf": shelf.iter().map(|(title, author, at)| serde_json::json!({
            "title": title, "author": author, "addedAt": at.to_rfc3339(),
        })).collect::<Vec<_>>(),
        "progress": progress.iter().map(|(title, author, pos, at)| serde_json::json!({
            "title": title, "author": author,
            "position": frac(pos), "updatedAt": at.to_rfc3339(),
        })).collect::<Vec<_>>(),
        "quotes": quotes.iter().map(QuoteRow::to_json).collect::<Vec<_>>(),
        "words": words.iter().map(WordRow::to_json).collect::<Vec<_>>(),
        "readingDays": days.iter().map(|(d, p)| serde_json::json!({
            "day": d.to_string(), "pages": p,
        })).collect::<Vec<_>>(),
    });

    Ok((
        [
            (header::CONTENT_TYPE, "application/json; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                r#"attachment; filename="leser.json""#,
            ),
        ],
        // pretty: файл открывают глазами, а не только программой
        serde_json::to_string_pretty(&body).map_err(AppError::internal)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn streak_counts_days_in_a_row_and_stops_at_the_first_gap() {
        let today = day("2026-08-06");

        assert_eq!(streak(&[], today), 0);
        // сегодня читали
        assert_eq!(streak(&[day("2026-08-06")], today), 1);
        // ещё не читали сегодня — вчерашний стрик жив
        assert_eq!(streak(&[day("2026-08-05")], today), 1);
        // позавчера читали, вчера нет — стрик уже оборвался
        assert_eq!(streak(&[day("2026-08-04")], today), 0);

        let week = [
            day("2026-08-01"),
            day("2026-08-02"),
            day("2026-08-03"),
            day("2026-08-06"),
        ];
        assert_eq!(streak(&week, today), 1, "дыра 4–5 августа обрывает счёт");

        let solid = [
            day("2026-08-01"),
            day("2026-08-04"),
            day("2026-08-05"),
            day("2026-08-06"),
        ];
        assert_eq!(streak(&solid, today), 3);
    }

    #[test]
    fn position_of_an_old_bookmark_is_the_start_not_a_crash() {
        assert_eq!(frac("0.5"), 0.5);
        assert_eq!(frac("epubcfi(/6/14[id]!/4/2/2)"), 0.0);
        assert_eq!(frac(""), 0.0);
        // из битой записи не должно вылезти «прочитано 300%»
        assert_eq!(frac("42"), 1.0);
        assert_eq!(frac("-1"), 0.0);
    }

    #[test]
    fn long_selections_are_cut_on_character_boundaries() {
        assert_eq!(cut("  цитата  ", 100), "цитата");
        // обрезка по символам: по байтам это была бы паника посреди буквы
        assert_eq!(cut("цитата", 3), "цит");
        assert_eq!(cut("", 10), "");
    }
}
