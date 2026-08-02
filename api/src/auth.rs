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
    pub admin: bool,
    pub exp: i64,
}

/// Извлекается из JWT без похода в БД — на каждый запрос экономится один запрос.
// ponytail: токен нельзя отозвать до истечения 30 дней. Нужен "выйти на всех
// устройствах" — добавить users.token_version и сверять с полем в claims.
pub struct AuthUser {
    pub id: Uuid,
    pub admin: bool,
}

impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self> {
        let unauthorized = || AppError::new(StatusCode::UNAUTHORIZED, "Not authorized");
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

        Ok(AuthUser {
            id: claims.sub,
            admin: claims.admin,
        })
    }
}

fn issue_token(secret: &str, id: Uuid, admin: bool) -> Result<String> {
    let exp = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64)
        + TOKEN_DAYS * 86_400;
    Ok(encode(
        &Header::default(),
        &Claims {
            sub: id,
            admin,
            exp,
        },
        &EncodingKey::from_secret(secret.as_bytes()),
    )?)
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
    is_admin: bool,
}

/// Тот же JSON, что отдавал Node, — фронт не трогаем.
/// `pages` фронт читает в EpubReader, поэтому отдаём всегда.
async fn session_response(
    state: &AppState,
    user: UserRow,
    code: StatusCode,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let token = issue_token(&state.jwt_secret, user.id, user.is_admin)?;
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
                "isAdmin": user.is_admin,
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
        return Err(AppError::new(StatusCode::CONFLICT, "It isn't email"));
    }
    if body.password.len() < 8 {
        return Err(AppError::bad("Password must be at least 8 characters"));
    }
    if body.username.trim().is_empty() {
        return Err(AppError::bad("Username is required"));
    }

    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(body.password.as_bytes(), &salt)?
        .to_string();

    let user = sqlx::query_as::<_, UserRow>(
        "insert into users (username, email, password_hash) values ($1, $2, $3)
         returning id, username, email, password_hash, is_admin",
    )
    .bind(body.username.trim())
    .bind(&email)
    .bind(&hash)
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        // уникальный индекс на email — гонку двух регистраций ловит БД, не мы
        sqlx::Error::Database(d) if d.is_unique_violation() => {
            AppError::new(StatusCode::CONFLICT, "Email is already in use")
        }
        e => e.into(),
    })?;

    session_response(&state, user, StatusCode::CREATED).await
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginBody>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let invalid = || AppError::new(StatusCode::UNAUTHORIZED, "Invalid credentials");
    if body.email.is_empty() || body.password.is_empty() {
        return Err(AppError::bad("Email and password are required"));
    }

    let user = sqlx::query_as::<_, UserRow>(
        "select id, username, email, password_hash, is_admin from users where email = $1",
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
        return Err(AppError::new(StatusCode::NOT_FOUND, "Not found"));
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
        let t = issue_token("s3cret", id, true).unwrap();
        let c = decode::<Claims>(
            &t,
            &DecodingKey::from_secret(b"s3cret"),
            &Validation::default(),
        )
        .unwrap()
        .claims;
        assert_eq!(c.sub, id);
        assert!(c.admin);
        // чужим ключом не разбирается
        assert!(decode::<Claims>(
            &t,
            &DecodingKey::from_secret(b"other"),
            &Validation::default()
        )
        .is_err());
    }
}
