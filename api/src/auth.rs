use std::sync::Arc;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{FromRequestParts, State},
    http::{request::Parts, StatusCode},
    Json,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AppError, AppState, Result};

const TOKEN_DAYS: i64 = 30;

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub exp: i64,
}

// ponytail: the role is read from the db on every request (one primary-key lookup), so a demotion
// or a deleted account takes effect at once; the token itself still cannot be revoked before it expires.
pub struct AuthUser {
    pub id: Uuid,
    pub admin: bool,
    pub full: bool,
}

impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self> {
        let unauthorized =
            || AppError::new(StatusCode::UNAUTHORIZED, "Нужен вход", "Not authorized");
        let token = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or_else(unauthorized)?;

        let claims = decode::<Claims>(
            token.trim(),
            &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| unauthorized())?
        .claims;

        let (role,): (String,) = sqlx::query_as("select role from users where id = $1")
            .bind(claims.sub)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(unauthorized)?;
        Ok(AuthUser {
            id: claims.sub,
            admin: role == ADMIN,
            full: role != USER,
        })
    }
}

pub const ADMIN: &str = "admin";
pub const FULL: &str = "full";
pub const USER: &str = "user";

fn issue_token(secret: &str, id: Uuid) -> Result<String> {
    let exp = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64)
        + TOKEN_DAYS * 86_400;
    Ok(encode(
        &Header::default(),
        &Claims { sub: id, exp },
        &EncodingKey::from_secret(secret.as_bytes()),
    )?)
}

// ponytail: the token rides in the image URL because <img> carries no headers.
// It is scoped to one book and dies in a day; a signed cookie is the upgrade
// if URLs start leaking into logs that matter.
pub fn book_token(secret: &str, book: Uuid) -> Result<String> {
    let exp = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64)
        + 86_400;
    Ok(encode(
        &Header::default(),
        &Claims { sub: book, exp },
        &EncodingKey::from_secret(secret.as_bytes()),
    )?)
}

pub fn book_of_token(secret: &str, token: &str) -> Option<Uuid> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .ok()
    .map(|t| t.claims.sub)
}

fn valid_email(e: &str) -> bool {
    let mut parts = e.split('@');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(local), Some(domain), None) => {
            !local.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && !e.chars().any(char::is_whitespace)
        }
        _ => false,
    }
}

#[derive(Deserialize)]
pub struct RegisterBody {
    username: String,
    email: String,
    password: String,
    invite: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginBody {
    email: String,
    password: String,
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    username: String,
    email: String,
    password_hash: String,
    role: String,
}

/// Тот же JSON, что отдавал Node, — фронт не трогаем.
/// `pages` фронт читает в EpubReader, поэтому отдаём всегда.
async fn session_response(
    state: &AppState,
    user: UserRow,
    code: StatusCode,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let token = issue_token(&state.jwt_secret, user.id)?;
    let pages = crate::books::pages_of(&state.db, user.id).await?;
    Ok((
        code,
        Json(serde_json::json!({
            "status": "success",
            "code": code.as_u16(),
            "data": { "user": {
                "id": user.id,
                "username": user.username,
                "email": user.email,
                "isAdmin": user.role == ADMIN,
                "role": user.role,
                "token": token,
                "pages": pages,
            }},
        })),
    ))
}

pub async fn registration(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RegisterBody>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let email = body.email.trim().to_lowercase();
    if !valid_email(&email) {
        return Err(AppError::new(
            StatusCode::CONFLICT,
            "Это не почта",
            "It isn't email",
        ));
    }
    if body.password.len() < 8 {
        return Err(AppError::bad(
            "Пароль должен быть не короче 8 символов",
            "Password must be at least 8 characters",
        ));
    }
    if body.username.trim().is_empty() {
        return Err(AppError::bad("Нужно имя", "Username is required"));
    }

    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(body.password.as_bytes(), &salt)?
        .to_string();

    let mut tx = state.db.begin().await?;
    let role = match body
        .invite
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        None => USER,
        Some(token) => {
            sqlx::query("update invites set used_at = now() where token = $1 and used_at is null")
                .bind(token)
                .execute(&mut *tx)
                .await?
                .rows_affected()
                .eq(&1)
                .then_some(FULL)
                .ok_or_else(|| {
                    AppError::bad(
                        "Приглашение уже использовано или не существует",
                        "The invite is used up or does not exist",
                    )
                })?
        }
    };
    let user = sqlx::query_as::<_, UserRow>(
        "insert into users (username, email, password_hash, role) values ($1, $2, $3, $4)
         returning id, username, email, password_hash, role",
    )
    .bind(body.username.trim())
    .bind(&email)
    .bind(&hash)
    .bind(role)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| match e {
        // уникальный индекс на email — гонку двух регистраций ловит БД, не мы
        sqlx::Error::Database(d) if d.is_unique_violation() => AppError::new(
            StatusCode::CONFLICT,
            "Эта почта уже занята",
            "Email is already in use",
        ),
        e => e.into(),
    })?;
    if let Some(token) = &body.invite {
        sqlx::query("update invites set used_by = $2 where token = $1")
            .bind(token.trim())
            .bind(user.id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    session_response(&state, user, StatusCode::CREATED).await
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginBody>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let invalid = || {
        AppError::new(
            StatusCode::UNAUTHORIZED,
            "Неверная почта или пароль",
            "Invalid credentials",
        )
    };
    if body.email.is_empty() || body.password.is_empty() {
        return Err(AppError::bad(
            "Нужны и почта, и пароль",
            "Email and password are required",
        ));
    }

    let user = sqlx::query_as::<_, UserRow>(
        "select id, username, email, password_hash, role from users where email = $1",
    )
    .bind(body.email.trim().to_lowercase())
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(invalid)?;

    let parsed = PasswordHash::new(&user.password_hash)?;
    Argon2::default()
        .verify_password(body.password.as_bytes(), &parsed)
        .map_err(|_| invalid())?;

    session_response(&state, user, StatusCode::OK).await
}

/// Токен stateless — сервер его не хранит и отзывать нечего.
/// Эндпоинт оставлен, чтобы фронт не менять; выход = удалить токен на клиенте.
pub async fn logout() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "success", "code": 200, "message": "Logged out" }))
}

/// Удаление аккаунта. Кого удалять — берётся из токена, тело не читается:
/// подделать чужой id нельзя, подпись уже проверена в `AuthUser`.
/// Закладки уходят каскадом (`reading_progress.user_id`), книги остаются
/// в библиотеке с `owner_id = null` — чужие читатели их не теряют.
// ponytail: токен живёт до истечения, но ссылается на несуществующего юзера;
// все его запросы упрутся в foreign key. Нужен внятный 401 сразу — сверять
// users.token_version, тот же механизм, что и для "выйти на всех устройствах".
pub async fn delete_account(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
) -> Result<StatusCode> {
    let deleted = sqlx::query("delete from users where id = $1")
        .bind(user.id)
        .execute(&state.db)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::new(
            StatusCode::NOT_FOUND,
            "Не найдено",
            "Not found",
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails() {
        for ok in ["a@b.co", "user.name+tag@sub.example.com"] {
            assert!(valid_email(ok), "{ok}");
        }
        for bad in ["", "a@b", "@b.co", "a@.co", "a@b.", "a b@c.co", "a@b@c.co"] {
            assert!(!valid_email(bad), "{bad}");
        }
    }

    #[test]
    fn token_roundtrip() {
        let id = Uuid::new_v4();
        let t = issue_token("s3cret", id).unwrap();
        let c = decode::<Claims>(
            &t,
            &DecodingKey::from_secret(b"s3cret"),
            &Validation::default(),
        )
        .unwrap()
        .claims;
        assert_eq!(c.sub, id);
        // чужим ключом не разбирается
        assert!(decode::<Claims>(
            &t,
            &DecodingKey::from_secret(b"other"),
            &Validation::default()
        )
        .is_err());
    }
}
