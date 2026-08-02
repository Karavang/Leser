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

/// Единственный тип ошибки. Тело ответа — `{"message": ...}`, как ждёт фронт.
#[derive(Debug)]
pub struct AppError(pub StatusCode, pub String);

impl AppError {
    pub fn new(code: StatusCode, msg: impl Into<String>) -> Self {
        Self(code, msg.into())
    }
    pub fn bad(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, msg)
    }
    /// Для чужих ошибок без `From`-импла (в основном generic-и AWS SDK):
    /// `.map_err(AppError::internal)?`
    pub fn internal(e: impl std::fmt::Display) -> Self {
        Self(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    }
}

// Нужно, чтобы ошибку можно было и залогировать через `{e}`, и вернуть из
// main через `?` наравне с любой другой.
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.0, self.1)
    }
}
impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Наружу уходит текст сообщения; внутренние подробности — только в лог.
        if self.0.is_server_error() {
            tracing::error!("{}", self.1);
            return (
                self.0,
                Json(serde_json::json!({ "message": "Internal error" })),
            )
                .into_response();
        }
        (self.0, Json(serde_json::json!({ "message": self.1 }))).into_response()
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

/// Слишком большой файл — это 413, а не 500, поэтому статус берём у самой ошибки.
impl From<axum::extract::multipart::MultipartError> for AppError {
    fn from(e: axum::extract::multipart::MultipartError) -> Self {
        Self(e.status(), e.body_text())
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
        .route("/booksInRead", get(books::books_in_read))
        .route("/searchSources", get(books::search_sources))
        .route("/import", post(books::import))
        .route("/registration", post(auth::registration))
        .route("/login", post(auth::login))
        .route("/logout", get(auth::logout))
        .route("/account", delete(auth::delete_account))
        .route("/health", get(|| async { "ok" }))
        .fallback(|| async { AppError::new(StatusCode::NOT_FOUND, "Not found") })
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "4444".into());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    tracing::info!("listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
