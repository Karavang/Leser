use std::{sync::Arc, time::Duration};

use axum::{
    extract::DefaultBodyLimit,
    http::{HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use sqlx::postgres::PgPoolOptions;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

mod auth;
mod books;
mod me;
mod mobi;
mod parse;
mod rda;
mod reader;
mod search;
mod sources;

/// Потолок на размер книги. epub/fb2 в этом умещаются с большим запасом.
const MAX_UPLOAD: usize = 64 * 1024 * 1024;

pub struct AppState {
    pub db: sqlx::PgPool,
    pub jwt_secret: String,
    /// Один на всё приложение: reqwest держит в нём пул соединений,
    /// новый клиент на запрос сводил бы пул на нет.
    pub http: reqwest::Client,
    /// Без него GitHub отвечает 401 на поиск по коду, и источник просто
    /// выключается — остальные продолжают работать.
    pub github_token: Option<String>,
}

/// Язык ответа. Правило то же, что во фронте: русский — только если читатель
/// попросил русский, всё остальное читается по-английски.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Ru,
    En,
}

impl Lang {
    /// `X-Lang` важнее `Accept-Language`: язык интерфейса читатель выбирает
    /// сам, и с языком браузера он совпадать не обязан. Из `Accept-Language`
    /// берётся первый вариант — разбирать q-веса ради двух языков незачем.
    fn of(h: &axum::http::HeaderMap) -> Self {
        let value = |name| h.get(name).and_then(|v: &HeaderValue| v.to_str().ok());
        let asked = value("x-lang").or_else(|| value("accept-language"));
        match asked.and_then(|v| v.split(',').next()) {
            Some(v) if v.trim().to_ascii_lowercase().starts_with("ru") => Lang::Ru,
            _ => Lang::En,
        }
    }
}

tokio::task_local! {
    // ponytail: язык запроса лежит в task-local, а не в аргументах. Иначе его
    // пришлось бы протаскивать через полсотни мест, включая разбор epub и mobi,
    // который про http не знает ничего и знать не должен.
    static LANG: Lang;
}

/// Язык текущего запроса. Вне запроса (тесты, старт) — английский.
pub fn lang() -> Lang {
    LANG.try_with(|l| *l).unwrap_or(Lang::En)
}

async fn with_lang(req: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let lang = Lang::of(req.headers());
    LANG.scope(lang, next.run(req)).await
}

/// Единственный тип ошибки. Тело ответа — `{"message": ...}`, как ждёт фронт;
/// текст хранится на обоих языках, нужный выбирается при отправке.
#[derive(Debug)]
pub struct AppError {
    pub code: StatusCode,
    pub ru: String,
    pub en: String,
}

impl AppError {
    pub fn new(code: StatusCode, ru: impl Into<String>, en: impl Into<String>) -> Self {
        Self {
            code,
            ru: ru.into(),
            en: en.into(),
        }
    }
    pub fn bad(ru: impl Into<String>, en: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, ru, en)
    }
    /// Для чужих ошибок без `From`-импла: `.map_err(AppError::internal)?`.
    /// Наружу такой текст не уходит вовсе — только в лог, поэтому он один.
    pub fn internal(e: impl std::fmt::Display) -> Self {
        let text = e.to_string();
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, text.clone(), text)
    }

    pub fn msg(&self) -> &str {
        match lang() {
            Lang::Ru => &self.ru,
            Lang::En => &self.en,
        }
    }
}

// Нужно, чтобы ошибку можно было и залогировать через `{e}`, и вернуть из
// main через `?` наравне с любой другой. В логе всегда английский: язык
// запроса к записи в журнале отношения не имеет.
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.en)
    }
}
impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Наружу уходит текст сообщения; внутренние подробности — только в лог.
        if self.code.is_server_error() {
            tracing::error!("{}", self.en);
            let message = match lang() {
                Lang::Ru => "Внутренняя ошибка",
                Lang::En => "Internal error",
            };
            return (self.code, Json(serde_json::json!({ "message": message }))).into_response();
        }
        let message = self.msg();
        (self.code, Json(serde_json::json!({ "message": message }))).into_response()
    }
}

// ponytail: блэнкет-импл From<E: Display> конфликтует с core, поэтому руками.
macro_rules! internal_from {
    ($($t:ty),* $(,)?) => { $(
        impl From<$t> for AppError {
            fn from(e: $t) -> Self { Self::internal(e) }
        }
    )* };
}
internal_from!(
    sqlx::Error,
    std::io::Error,
    uuid::Error,
    quick_xml::Error,
    jsonwebtoken::errors::Error,
    argon2::password_hash::Error,
);

/// Слишком большой файл — это 413, а не 500, поэтому статус берём у самой
/// ошибки. Текст у неё свой, английский, — на два языка его не разложить.
impl From<axum::extract::multipart::MultipartError> for AppError {
    fn from(e: axum::extract::multipart::MultipartError) -> Self {
        Self::new(e.status(), e.body_text(), e.body_text())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("missing env var {key}"))
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "leser_api=info,tower_http=info".into()),
        )
        .init();

    let db = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&env("DATABASE_URL"))
        .await?;
    sqlx::migrate!().run(&db).await?;

    let state = Arc::new(AppState {
        jwt_secret: env("JWT_SECRET"),
        http: sources::client()?,
        github_token: std::env::var("GITHUB_TOKEN").ok().filter(|t| !t.is_empty()),
        db,
    });
    if state.github_token.is_none() {
        tracing::info!("GITHUB_TOKEN не задан — поиск по GitHub выключен");
    }

    let cors = CorsLayer::new()
        .allow_origin(
            env("CORS_ORIGINS")
                .split(',')
                .map(|o| o.trim().parse::<HeaderValue>().expect("bad CORS origin"))
                .collect::<Vec<_>>(),
        )
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderName::from_static("x-lang"),
        ]);

    let app = Router::new()
        .route("/getAll", get(books::get_all))
        .route("/downloadOne/{filename}", get(books::download_one))
        .route("/read/{filename}", get(books::read_one))
        .route("/deleteOne/{filename}", delete(books::delete_one))
        // Большой лимит только на загрузку книги: JSON-эндпоинты остаются
        // с дефолтными 2 МБ, чтобы их нельзя было завалить телом на 64 МБ.
        .route(
            "/putNewOne",
            post(books::put_new_one).layer(DefaultBodyLimit::max(MAX_UPLOAD)),
        )
        .route("/pageWasFlipped", post(books::page_was_flipped))
        .route(
            "/myBooks/{filename}",
            post(books::add_to_my).delete(books::remove_from_my),
        )
        .route("/progress/{filename}", get(books::progress_of))
        .route("/booksInRead", get(books::books_in_read))
        // Личный кабинет: выписки, словарь, статистика и выгрузка всего своего.
        .route("/quotes", get(me::quotes).post(me::add_quote))
        .route("/quotes/{id}", delete(me::delete_quote))
        .route("/words", get(me::words).post(me::add_word))
        .route("/words/{id}", delete(me::delete_word))
        .route("/stats", get(me::stats))
        .route("/export", get(me::export))
        .route("/searchSources", get(books::search_sources))
        .route("/import", post(books::import))
        .route("/registration", post(auth::registration))
        .route("/login", post(auth::login))
        .route("/logout", get(auth::logout))
        .route("/account", delete(auth::delete_account))
        .route("/health", get(|| async { "ok" }))
        .fallback(|| async {
            AppError::new(StatusCode::NOT_FOUND, "Не найдено", "Not found")
        })
        .layer(cors)
        // Ниже cors и логов: язык нужен только тому, что отвечает телом.
        .layer(axum::middleware::from_fn(with_lang))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "4444".into());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    tracing::info!("listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// Ctrl-C в терминале и SIGTERM от `docker stop`. Второй обязателен: в
/// контейнере процесс — PID 1, а ему ядро не применяет действие по умолчанию,
/// то есть без обработчика SIGTERM просто игнорируется, docker ждёт таймаут
/// и убивает SIGKILL'ом — вместе с загрузкой, которая шла в этот момент.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(
                axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                HeaderValue::from_str(v).unwrap(),
            );
        }
        h
    }

    #[test]
    fn language_comes_from_the_reader_first_and_the_browser_second() {
        assert_eq!(Lang::of(&headers(&[])), Lang::En);
        assert_eq!(
            Lang::of(&headers(&[("accept-language", "ru-RU,ru")])),
            Lang::Ru
        );
        assert_eq!(
            Lang::of(&headers(&[("accept-language", "de-DE")])),
            Lang::En
        );
        // выбор в настройках перевешивает язык браузера — в обе стороны
        assert_eq!(
            Lang::of(&headers(&[("x-lang", "en"), ("accept-language", "ru-RU")])),
            Lang::En
        );
        assert_eq!(
            Lang::of(&headers(&[("x-lang", "ru"), ("accept-language", "en-US")])),
            Lang::Ru
        );
    }

    /// Ошибка несёт оба текста, а выбирает между ними отправка ответа.
    #[tokio::test]
    async fn the_message_follows_the_language_of_the_request() {
        let err = || AppError::bad("Файл не приложен", "No file uploaded");
        assert_eq!(err().msg(), "No file uploaded", "вне запроса — английский");
        assert_eq!(
            LANG.scope(Lang::Ru, async { err().msg().to_string() })
                .await,
            "Файл не приложен"
        );
        assert_eq!(
            LANG.scope(Lang::En, async { err().msg().to_string() })
                .await,
            "No file uploaded"
        );
    }
}
