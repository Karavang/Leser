use std::sync::Arc;

use axum::{
    extract::{Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
// chrono приезжает вместе с sqlx (фича "chrono"), отдельной зависимости не нужно
use sqlx::{
    types::chrono::{DateTime, Utc},
    PgPool,
};
use uuid::Uuid;

use crate::{auth::AuthUser, AppError, AppState, Result, MAX_UPLOAD};

#[derive(sqlx::FromRow)]
pub struct Book {
    id: Uuid,
    title: String,
    author: String,
    published: Option<String>,
    lang: Option<String>,
    description: Option<String>,
    series: Option<String>,
    ext: String,
    owner_id: Option<Uuid>,
    /// null у книг, чей владелец удалил аккаунт, — они остаются в библиотеке
    added_by: Option<String>,
    /// заполнен у книг, принесённых из внешнего источника; для своих загрузок null
    source_url: Option<String>,
    created_at: DateTime<Utc>,
    /// книга лежит на полке у того, кто спрашивает («мои книги»)
    on_shelf: bool,
}

/// Форматы, которые вообще принимаются. Тот же список стоит в `accept`
/// у файлового поля, но полагаться на него нельзя: запрос ничего не стоит
/// послать мимо формы.
///
/// `rda` — формат данных R; книги в нём попадаются на GitHub. Читалки под
/// него нет и не будет: он превращается в fb2 прямо на входе (`store_book`).
///
/// Чего здесь нет намеренно: `djvu` — это сканы, а не текст, и без djvulibre
/// из них не достать даже слоя OCR; `kfx` — закрытый формат Amazon, который
/// в живой природе всегда под DRM. Обоим место в `TODO.md`, а не здесь.
pub const BOOK_EXTS: [&str; 9] = [
    "epub", "fb2", "mobi", "azw3", "pdf", "txt", "md", "html", "rda",
];

const BOOK_COLS: &str = "b.id, b.title, b.author, b.published, b.lang, b.description, b.series,
     b.ext, b.owner_id, b.source_url, b.created_at, u.username as added_by,
     (s.user_id is not null) as on_shelf";
/// `$1` — всегда читатель, который спрашивает: от него зависит и полка.
const BOOK_FROM: &str = "from books b
     left join users u on u.id = b.owner_id
     left join shelf s on s.book_id = b.id and s.user_id = $1";

/// Название, автор и серия одной строкой: искать надо по всем трём сразу,
/// а слово из запроса может лежать в любом.
const HAY: &str = "(b.title || ' ' || b.author || ' ' || coalesce(b.series, ''))";

/// Порог похожести триграмм. 0.5 связывает словоформы («завоеват» с
/// «завоевывать» — 0.67) и прощает опечатки («гари потер» — 0.6), но ещё
/// не тащит в выдачу случайные книги.
const FUZZY: f32 = 0.5;

/// Слишком короткое слово похоже на слишком многое, поэтому для него работает
/// только точное вхождение.
const FUZZY_MIN_LEN: i32 = 4;

/// Книга подходит, если совпало **каждое** слово запроса — отсюда «нет слова,
/// которое не совпало». Само слово совпадает либо подстрокой, либо по
/// триграммам: подстрока не связывает словоформы, `ilike` в принципе не умеет
/// «завоевать» ≈ «завоевывать», а русский стеммер тоже не умеет.
///
/// Регистр многоязычен и без нас: свёртку делает коллация базы (`en_US.utf8`),
/// одинаково для `ПРИВЕТ`, `ĞÜNEŞ` и `ΑΘΗΝΑ`.
fn all_words_match(param: &str) -> String {
    format!(
        "not exists (
             select 1 from unnest({param}::text[]) w
             where {HAY} not ilike '%' || w || '%'
               and (length(w) < {FUZZY_MIN_LEN} or word_similarity(w, {HAY}) < {FUZZY})
         )"
    )
}

impl Book {
    /// Форма ответа как у старого Node-бэка: filename собирается из id и ext.
    /// `mine`/`canDelete` считает сервер — фронт не должен знать правила прав.
    ///
    /// `mine` — про полку читателя, `canDelete` — про то, кто книгу принёс.
    /// Это разные вещи: чужую книгу можно отложить себе, но удалить из
    /// библиотеки её нельзя, а свою можно убрать с полки, не удаляя.
    ///
    /// `source`/`addedAt` — про то, откуда книга взялась, а не про произведение;
    /// на карточке им не место, фронт показывает их только в подробностях.
    fn to_json(&self, user: &AuthUser) -> serde_json::Value {
        let owner = self.owner_id == Some(user.id);

        // Источник — одно понятие на два случая: либо читатель, который принёс
        // файл, либо адрес, откуда книгу забрали. Ссылка есть только у второго.
        let source = match (&self.source_url, &self.added_by) {
            (Some(url), _) => url
                .parse::<reqwest::Url>()
                .ok()
                .and_then(|u| u.host_str().map(str::to_string))
                .unwrap_or_else(|| url.clone()),
            (None, Some(who)) => who.clone(),
            // владелец удалил аккаунт, а книга осталась в библиотеке
            (None, None) => "—".into(),
        };

        serde_json::json!({
            "id": self.id,
            "title": self.title,
            "author": self.author,
            "date": self.published,
            "lang": self.lang,
            "desc": self.description,
            "series": self.series,
            "filename": format!("{}.{}", self.id, self.ext),
            "source": source,
            // ссылка только у внешних: у своей загрузки вести некуда
            "sourceUrl": self.source_url,
            // rfc3339, чтобы фронт отформатировал под локаль читателя сам
            "addedAt": self.created_at.to_rfc3339(),
            "mine": self.on_shelf,
            "canDelete": owner || user.admin,
        })
    }
}

/// `filename` во внешнем API — это `{uuid}.{ext}`. Разбираем строго: наружу
/// не собирается никакой путь, в запрос уходит только проверенный uuid.
pub fn parse_filename(filename: &str) -> Result<(Uuid, String)> {
    let (id, ext) = filename
        .rsplit_once('.')
        .ok_or_else(|| AppError::bad("Неверное имя файла", "Invalid filename"))?;
    let id: Uuid = id
        .parse()
        .map_err(|_| AppError::bad("Неверное имя файла", "Invalid filename"))?;
    if ext.is_empty() || !ext.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(AppError::bad("Неверное имя файла", "Invalid filename"));
    }
    Ok((id, ext.to_lowercase()))
}

/// Тип для выдачи самого файла книги.
///
/// `html` здесь намеренно не `text/html`: файл пришёл от читателя и лежит
/// на нашем же origin — браузер выполнил бы его скрипты вместе с сессией
/// того, кто его открыл. Скачивание книги — это скачивание, а не просмотр.
fn content_type(ext: &str) -> &'static str {
    match ext {
        "epub" => "application/epub+zip",
        "fb2" => "application/x-fictionbook+xml",
        "pdf" => "application/pdf",
        "mobi" => "application/x-mobipocket-ebook",
        "azw3" => "application/vnd.amazon.ebook",
        // без charset: в файле может лежать и windows-1251
        "txt" => "text/plain",
        "md" => "text/markdown",
        _ => "application/octet-stream",
    }
}

pub async fn pages_of(db: &PgPool, user_id: Uuid) -> Result<Vec<serde_json::Value>> {
    let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
        "select p.book_id, b.ext, p.position
         from reading_progress p join books b on b.id = p.book_id
         where p.user_id = $1",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, ext, page)| serde_json::json!({ "filename": format!("{id}.{ext}"), "page": page }))
        .collect())
}

#[derive(Deserialize)]
pub struct SearchQuery {
    q: Option<String>,
}

pub async fn get_all(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Query(search): Query<SearchQuery>,
) -> Result<Json<Vec<serde_json::Value>>> {
    let q = crate::search::Query::parse(&search.q.unwrap_or_default());

    // Прочтений может быть несколько: как набрано, транслитерация, обе
    // раскладки. Подходит книга, совпавшая с любым из них — отсюда OR.
    // Плейсхолдеры генерим мы, значения по-прежнему уходят через bind.
    let filter = if q.is_empty() {
        "true".to_string()
    } else {
        q.variants
            .iter()
            .enumerate()
            .map(|(i, _)| all_words_match(&format!("${}", i + 2)))
            .collect::<Vec<_>>()
            .join(" or ")
    };

    // ponytail: пагинации нет и раздел "мои книги" фронт фильтрует по флагу mine
    // на уже полученном списке — на текущей библиотеке это один скан.
    // Появится тысяча книг — limit/offset, ?owner=me и trigram-индекс из 0002.
    // sql отдельной переменной: запрос одалживает строку, а временная из
    // format! умерла бы концом этого же выражения.
    let sql = format!("select {BOOK_COLS} {BOOK_FROM} where ({filter}) order by b.created_at desc");
    let mut query = sqlx::query_as::<_, Book>(&sql).bind(user.id);
    for words in &q.variants {
        query = query.bind(words);
    }

    let books = query.fetch_all(&state.db).await?;
    Ok(Json(books.iter().map(|b| b.to_json(&user)).collect()))
}

pub async fn books_in_read(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
) -> Result<Json<Vec<serde_json::Value>>> {
    let books: Vec<Book> = sqlx::query_as(&format!(
        "select {BOOK_COLS} {BOOK_FROM}
         join reading_progress p on p.book_id = b.id
         where p.user_id = $1 order by p.updated_at desc"
    ))
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(books.iter().map(|b| b.to_json(&user)).collect()))
}

/// Отдаёт сам файл книги: читалка получает байты одним запросом.
pub async fn download_one(
    State(state): State<Arc<AppState>>,
    _user: AuthUser,
    Path(filename): Path<String>,
) -> Result<impl IntoResponse> {
    let (id, ext) = parse_filename(&filename)?;

    let row: Option<(Vec<u8>,)> = sqlx::query_as(
        "select f.data from book_files f join books b on b.id = f.book_id
         where f.book_id = $1 and b.ext = $2",
    )
    .bind(id)
    .bind(&ext)
    .fetch_optional(&state.db)
    .await?;

    let (data,) = row
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, "Файл не найден", "File not found"))?;

    Ok((
        [
            (header::CONTENT_TYPE, content_type(&ext)),
            // Файл неизменяем: на каждую загрузку новый id. Значит можно кэшировать
            // навсегда, и повторное открытие книги вообще не дойдёт до сервера.
            (
                header::CACHE_CONTROL,
                "private, max-age=31536000, immutable",
            ),
            // Тип мы указали сами и угадывать его по содержимому не надо:
            // иначе браузер найдёт в чужом файле html и покажет его как html.
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        data,
    ))
}

/// Разбирает книгу в то, что читает читалка, и сразу сериализует: в БД и в
/// ответ уходит одна и та же строка, лишнего круга через serde нет.
///
/// Разбор синхронный и на большой книге не мгновенный — как и в parse.rs,
/// он уходит в blocking-пул, чтобы не держать поток рантайма.
async fn reader_doc(
    ext: &str,
    bytes: axum::body::Bytes,
    title: &str,
    author: &str,
    lang: &Option<String>,
) -> Result<String> {
    let ext = ext.to_string();
    let head = serde_json::json!({
        "title": title,
        "author": author,
        // язык нужен читалке для переносов: без него hyphens: auto
        // не знает словаря и просто ничего не делает
        "lang": lang,
    });
    tokio::task::spawn_blocking(move || {
        let chapters = crate::reader::chapters(&ext, &bytes)?;
        let mut doc = head;
        // В txt, pdf и html языку взяться неоткуда — а переносам он нужен.
        if doc["lang"].is_null() {
            doc["lang"] = crate::reader::guess_lang(&chapters).into();
        }
        doc["chapters"] = serde_json::to_value(chapters).map_err(AppError::internal)?;
        serde_json::to_string(&doc).map_err(AppError::internal)
    })
    .await
    .map_err(AppError::internal)?
}

/// Книга для читалки: главы с разметкой, одним ответом на всю книгу.
///
/// Формат разбирает сервер, поэтому читалка на фронте одна на все форматы
/// и ничего не знает ни про zip внутри epub, ни про xml внутри fb2. Разбор
/// уже лежит готовым — сюда он попал при загрузке книги.
pub async fn read_one(
    State(state): State<Arc<AppState>>,
    _user: AuthUser,
    Path(filename): Path<String>,
) -> Result<impl IntoResponse> {
    let (id, ext) = parse_filename(&filename)?;

    let doc: Option<(String,)> = sqlx::query_as(
        "select d.doc from book_docs d join books b on b.id = d.book_id
         where d.book_id = $1 and b.ext = $2",
    )
    .bind(id)
    .bind(&ext)
    .fetch_optional(&state.db)
    .await?;

    let doc = match doc {
        Some((doc,)) => doc,
        // Книга легла в библиотеку раньше, чем появился разбор при загрузке,
        // или book_docs почистили ради пересборки. Собираем и запоминаем —
        // второй раз этого уже не потребуется.
        None => rebuild_doc(&state, id, &ext).await?,
    };

    Ok((
        [
            (header::CONTENT_TYPE, "application/json"),
            // Файл книги неизменяем (на каждую загрузку новый id), значит и разбор
            // тоже: второе открытие книги до сервера не доходит вовсе.
            (
                header::CACHE_CONTROL,
                "private, max-age=31536000, immutable",
            ),
        ],
        doc,
    ))
}

async fn rebuild_doc(state: &AppState, id: Uuid, ext: &str) -> Result<String> {
    let row: Option<(Vec<u8>, String, String, Option<String>)> = sqlx::query_as(
        "select f.data, b.title, b.author, b.lang
         from book_files f join books b on b.id = f.book_id
         where f.book_id = $1 and b.ext = $2",
    )
    .bind(id)
    .bind(ext)
    .fetch_optional(&state.db)
    .await?;

    let (data, title, author, lang) = row
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, "Файл не найден", "File not found"))?;

    let doc = reader_doc(ext, data.into(), &title, &author, &lang).await?;
    sqlx::query(
        "insert into book_docs (book_id, doc) values ($1, $2)
         on conflict (book_id) do update set doc = excluded.doc",
    )
    .bind(id)
    .bind(&doc)
    .execute(&state.db)
    .await?;
    Ok(doc)
}

/// Название книги из имени файла — для форматов, где названию внутри просто
/// негде лежать: `.rda`, `.txt`, чаще всего `.pdf`. `philosophers_stone.rda`
/// превращается в «Philosophers Stone».
fn title_from_filename(name: &str) -> String {
    let stem = name
        .rsplit('/')
        .next()
        .unwrap_or(name)
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(name);

    stem.split(['_', '-', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Расширение из имени файла, если оно вообще из тех, что мы читаем.
/// Разные имена одного формата сводятся к одному здесь — дальше по коду
/// про `.htm` и `.markdown` знать уже никому не нужно.
fn book_ext(name: &str) -> Result<String> {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .unwrap_or_default();
    let ext = match ext.as_str() {
        "htm" | "xhtml" => "html".to_string(),
        "markdown" | "mdown" => "md".to_string(),
        "azw" => "mobi".to_string(),
        _ => ext,
    };
    if !BOOK_EXTS.contains(&ext.as_str()) {
        return Err(AppError::bad(
            format!("Библиотека открывает только {}", BOOK_EXTS.join(", ")),
            format!("Only {} are supported", BOOK_EXTS.join(", ")),
        ));
    }
    Ok(ext)
}

/// Единственный путь книги в библиотеку — им идут и загрузка файла, и импорт
/// по ссылке. Значит, разбор, чистка метаданных и транзакция у них общие,
/// и внешняя книга не может попасть в базу по более слабым правилам.
///
/// Расширение — заявление клиента, а не факт: настоящая проверка в том, что
/// файл разбирается. Внутри не epub-архив или не fb2-разметка — 400.
async fn store_book(
    state: &AppState,
    owner: Uuid,
    ext: &str,
    name: &str,
    bytes: axum::body::Bytes,
    source_url: Option<&str>,
) -> Result<String> {
    // .rda хранить как есть незачем: читалки под него нет, а текст внутри
    // обычный. Превращаем в fb2 здесь, и дальше он ничем не отличается
    // от книги, загруженной файлом.
    let (ext, bytes) = if ext == "rda" {
        let chapters = crate::rda::chapters(&bytes)?;
        let title = title_from_filename(name);
        let fb2 = crate::rda::to_fb2(&title, &chapters);
        ("fb2", axum::body::Bytes::from(fb2))
    } else {
        (ext, bytes)
    };

    let mut meta = crate::parse::metadata(ext, bytes.clone()).await?;
    // В txt и pdf названию внутри лежать негде, в html его тоже часто нет.
    // Имя файла — единственное, что о книге вообще известно.
    if meta.title == crate::parse::UNTITLED {
        meta.title = title_from_filename(name);
    }

    // Книга разбирается для читалки здесь же, один раз на всю жизнь книги.
    // Заодно это и проверка: то, из чего не достать ни строчки текста, читать
    // всё равно нечем, и лучше сказать об этом при загрузке, чем оставить
    // в библиотеке книгу, которая не открывается.
    let doc = reader_doc(ext, bytes.clone(), &meta.title, &meta.author, &meta.lang).await?;

    // Метаданные, файл и разбор — одной транзакцией. Строки без файла (сломала бы
    // читалку) и файла без строки (мусор) не бывает по построению — с S3 это
    // не гарантировалось.
    let mut tx = state.db.begin().await?;
    let (id,): (Uuid,) = sqlx::query_as(
        "insert into books
             (title, author, published, lang, description, series, ext, owner_id, source_url)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9) returning id",
    )
    .bind(&meta.title)
    .bind(&meta.author)
    .bind(&meta.published)
    .bind(&meta.lang)
    .bind(&meta.description)
    .bind(&meta.series)
    .bind(ext)
    .bind(owner)
    .bind(source_url)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("insert into book_files (book_id, data) values ($1, $2)")
        .bind(id)
        .bind(bytes.as_ref())
        .execute(&mut *tx)
        .await?;

    sqlx::query("insert into book_docs (book_id, doc) values ($1, $2)")
        .bind(id)
        .bind(&doc)
        .execute(&mut *tx)
        .await?;

    // Кто книгу принёс, у того она сразу и в «моих»: отдельно откладывать
    // себе только что загруженную книгу — лишний шаг.
    sqlx::query("insert into shelf (user_id, book_id) values ($1, $2)")
        .bind(owner)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(format!("{id}.{ext}"))
}

pub async fn put_new_one(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>> {
    let mut file = None;
    while let Some(field) = multipart.next_field().await? {
        if field.name() == Some("file") {
            let name = field.file_name().unwrap_or_default().to_string();
            file = Some((name, field.bytes().await?));
            break;
        }
    }
    let (name, bytes) =
        file.ok_or_else(|| AppError::bad("Файл не приложен", "No file uploaded"))?;
    let ext = book_ext(&name)?;

    let location = store_book(&state, user.id, &ext, &name, bytes, None).await?;
    Ok(Json(serde_json::json!({
        "message": "File uploaded successfully",
        "location": location,
    })))
}

/// Поиск за пределами своей библиотеки. Отдельным запросом, а не внутри
/// `/getAll`: своя библиотека приоритетнее и не должна ждать чужой сети.
pub async fn search_sources(
    State(state): State<Arc<AppState>>,
    _user: AuthUser,
    Query(search): Query<SearchQuery>,
) -> Result<Json<Vec<crate::sources::Found>>> {
    let q = search.q.unwrap_or_default();
    let q = q.trim();
    if q.is_empty() {
        return Ok(Json(Vec::new()));
    }
    Ok(Json(
        crate::sources::search(&state.http, q, state.github_token.as_deref()).await,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportBody {
    url: String,
    /// Страница, откуда книга, — она и попадёт в графу «Источник».
    /// Не указана — источником считается сам файл.
    page_url: Option<String>,
}

/// Забирает книгу из внешнего источника в библиотеку.
///
/// Адрес присылает клиент, поэтому здесь же и защита от SSRF: ходить можно
/// только по https и только на известные хосты, и это проверяется на каждом
/// переходе по редиректу (политика в `sources::client`). Размер ограничен
/// тем же потолком, что и загрузка файла.
pub async fn import(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(body): Json<ImportBody>,
) -> Result<Json<serde_json::Value>> {
    let url: reqwest::Url = body
        .url
        .parse()
        .map_err(|_| AppError::bad("Некорректная ссылка", "Malformed link"))?;
    if !crate::sources::host_allowed(&url) {
        return Err(AppError::bad(
            "Источник не в списке разрешённых",
            "This source is not on the allowed list",
        ));
    }

    // Расширение берём из адреса; окончательно решает всё равно разбор файла.
    // Адрес вида /ebooks/2554.epub3.images расширения не содержит — там epub.
    let ext = book_ext(url.path()).unwrap_or_else(|_| "epub".into());

    let response = state
        .http
        .get(url.clone())
        .send()
        .await
        .map_err(|e| {
            AppError::bad(
                format!("Источник не ответил: {e}"),
                format!("The source did not respond: {e}"),
            )
        })?
        .error_for_status()
        .map_err(|e| {
            AppError::bad(
                format!("Источник ответил ошибкой: {e}"),
                format!("The source answered with an error: {e}"),
            )
        })?;

    // Заявленной длине не верим — проверяем и её, и то, что пришло на самом деле.
    if response
        .content_length()
        .is_some_and(|n| n > MAX_UPLOAD as u64)
    {
        return Err(AppError::bad(
            "Книга больше 64 МБ",
            "The book is over 64 MB",
        ));
    }
    let bytes = response.bytes().await.map_err(|e| {
        AppError::bad(
            format!("Не удалось скачать: {e}"),
            format!("Download failed: {e}"),
        )
    })?;
    if bytes.len() > MAX_UPLOAD {
        return Err(AppError::bad(
            "Книга больше 64 МБ",
            "The book is over 64 MB",
        ));
    }

    let source = body.page_url.as_deref().unwrap_or(&body.url);
    let location = store_book(&state, user.id, &ext, url.path(), bytes, Some(source)).await?;

    Ok(Json(serde_json::json!({
        "message": "Книга добавлена в библиотеку",
        "location": location,
    })))
}

pub async fn delete_one(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(filename): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let (id, _) = parse_filename(&filename)?;

    // Права проверяет сам запрос: свою книгу или любую, если админ.
    // Файл уходит каскадом из book_files, отдельного удаления не нужно.
    let deleted = sqlx::query("delete from books where id = $1 and (owner_id = $2 or $3)")
        .bind(id)
        .bind(user.id)
        .bind(user.admin)
        .execute(&state.db)
        .await?;

    if deleted.rows_affected() == 0 {
        let exists: Option<(Uuid,)> = sqlx::query_as("select id from books where id = $1")
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
        return Err(match exists {
            Some(_) => AppError::new(StatusCode::FORBIDDEN, "Это не ваша книга", "Not your book"),
            None => AppError::new(StatusCode::NOT_FOUND, "Не найдено", "Not found"),
        });
    }

    Ok(Json(
        serde_json::json!({ "message": format!("{filename} deleted") }),
    ))
}

/// Отложить книгу себе. Полка не про права: отложить можно любую книгу
/// из библиотеки, и на саму книгу это никак не влияет.
pub async fn add_to_my(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(filename): Path<String>,
) -> Result<StatusCode> {
    let (book_id, _) = parse_filename(&filename)?;

    // Повторное добавление — не ошибка: кнопку могли нажать дважды или
    // из двух окон сразу.
    sqlx::query(
        "insert into shelf (user_id, book_id) values ($1, $2)
         on conflict (user_id, book_id) do nothing",
    )
    .bind(user.id)
    .bind(book_id)
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(d) if d.is_foreign_key_violation() => {
            AppError::new(StatusCode::NOT_FOUND, "Не найдено", "Not found")
        }
        e => e.into(),
    })?;

    Ok(StatusCode::NO_CONTENT)
}

/// Убрать книгу со своей полки. Из библиотеки она никуда не девается —
/// это разные действия, и удаление живёт в `delete_one`.
pub async fn remove_from_my(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(filename): Path<String>,
) -> Result<StatusCode> {
    let (book_id, _) = parse_filename(&filename)?;

    sqlx::query("delete from shelf where user_id = $1 and book_id = $2")
        .bind(user.id)
        .bind(book_id)
        .execute(&state.db)
        .await?;

    // Книги на полке и так нет — значит, всё как просили.
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct FlipBody {
    filename: String,
    page: String,
}

/// Один upsert вместо чтения и перезаписи всего массива pages, как было в Mongo.
pub async fn page_was_flipped(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(body): Json<FlipBody>,
) -> Result<StatusCode> {
    let (book_id, _) = parse_filename(&body.filename)?;

    sqlx::query(
        "insert into reading_progress (user_id, book_id, position) values ($1, $2, $3)
         on conflict (user_id, book_id)
         do update set position = excluded.position, updated_at = now()",
    )
    .bind(user.id)
    .bind(book_id)
    .bind(&body.page)
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(d) if d.is_foreign_key_violation() => {
            AppError::new(StatusCode::NOT_FOUND, "Не найдено", "Not found")
        }
        e => e.into(),
    })?;

    // История для статистики. Отдельным запросом и без транзакции: закладка
    // важна, счётчик страниц — нет, и терять первое из-за второго незачем.
    // Запрос в читалке дебаунсится, так что это примерно перевёрнутые страницы.
    sqlx::query(
        "insert into reading_days (user_id, day, pages) values ($1, current_date, 1)
         on conflict (user_id, day) do update set pages = reading_days.pages + 1",
    )
    .bind(user.id)
    .execute(&state.db)
    .await?;

    Ok(StatusCode::OK)
}

/// Место в книге по мнению сервера. Раньше позицию знал только вход
/// (`user.pages` в localStorage), поэтому устройство с уже открытой сессией
/// показывало вчерашнюю страницу. Читалка спрашивает при открытии книги.
pub async fn progress_of(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(filename): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let (book_id, _) = parse_filename(&filename)?;

    let row: Option<(String,)> = sqlx::query_as(
        "select position from reading_progress where user_id = $1 and book_id = $2",
    )
    .bind(user.id)
    .bind(book_id)
    .fetch_optional(&state.db)
    .await?;

    // Книгу ещё не открывали — это не ошибка, а начало книги.
    Ok(Json(
        serde_json::json!({ "page": row.map(|(position,)| position) }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// «Моя книга» и «моя, чтобы удалить» — разные вещи, и путать их нельзя:
    /// на полке может лежать чужая книга, а своя — не лежать.
    #[test]
    fn shelf_and_ownership_are_separate() {
        let me = Uuid::new_v4();
        let book = |owner: Option<Uuid>, on_shelf: bool| Book {
            id: Uuid::new_v4(),
            title: "t".into(),
            author: "a".into(),
            published: None,
            lang: None,
            description: None,
            series: None,
            ext: "epub".into(),
            owner_id: owner,
            added_by: owner.map(|_| "vasya".into()),
            created_at: Utc::now(),
            source_url: None,
            on_shelf,
        };
        let user = |admin| AuthUser { id: me, admin };

        let mine = book(Some(me), true).to_json(&user(false));
        assert_eq!(mine["mine"], true);
        assert_eq!(mine["canDelete"], true);

        // свою книгу убрали с полки — удалять её всё ещё можно
        let put_away = book(Some(me), false).to_json(&user(false));
        assert_eq!(put_away["mine"], false);
        assert_eq!(put_away["canDelete"], true);

        // чужая книга на своей полке: читать — да, удалять — нет
        let borrowed = book(Some(Uuid::new_v4()), true);
        assert_eq!(borrowed.to_json(&user(false))["mine"], true);
        assert_eq!(borrowed.to_json(&user(false))["canDelete"], false);
        assert_eq!(borrowed.to_json(&user(true))["canDelete"], true);

        // владелец удалил аккаунт: книга ничья, но админ её всё ещё сносит
        let orphan = book(None, false);
        assert_eq!(orphan.to_json(&user(false))["mine"], false);
        assert_eq!(orphan.to_json(&user(false))["canDelete"], false);
        assert_eq!(orphan.to_json(&user(true))["canDelete"], true);
        assert!(orphan.to_json(&user(false))["addedBy"].is_null());
    }

    #[test]
    fn filenames() {
        let id = Uuid::new_v4();
        assert_eq!(
            parse_filename(&format!("{id}.EPUB")).unwrap(),
            (id, "epub".into())
        );
        for bad in [
            "",
            "noext",
            "../../etc/passwd",
            "../../etc/passwd.epub",
            "not-a-uuid.epub",
            &format!("{id}.ep/ub"),
            &format!("{id}."),
        ] {
            assert!(parse_filename(bad).is_err(), "{bad}");
        }
    }
}
