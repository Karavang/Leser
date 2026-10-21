use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use sqlx::types::chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    auth::{AuthUser, ADMIN, FULL, USER},
    books::parse_filename,
    AppError, AppState, Result,
};

fn admin_only(user: &AuthUser) -> Result<()> {
    if user.admin {
        Ok(())
    } else {
        Err(AppError::new(
            StatusCode::FORBIDDEN,
            "Только для админа",
            "Admins only",
        ))
    }
}

fn not_found() -> AppError {
    AppError::new(StatusCode::NOT_FOUND, "Не найдено", "Not found")
}

pub async fn users(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
) -> Result<Json<Vec<serde_json::Value>>> {
    admin_only(&user)?;
    let rows: Vec<(Uuid, String, String, String, DateTime<Utc>)> = sqlx::query_as(
        "select id, username, email, role, created_at from users order by created_at",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, username, email, role, created)| {
                serde_json::json!({
                    "id": id,
                    "username": username,
                    "email": email,
                    "role": role,
                    "createdAt": created.to_rfc3339(),
                    "me": id == user.id,
                })
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct RoleBody {
    role: String,
}

pub async fn set_role(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<RoleBody>,
) -> Result<StatusCode> {
    admin_only(&user)?;
    if ![ADMIN, FULL, USER].contains(&body.role.as_str()) {
        return Err(AppError::bad("Нет такой роли", "No such role"));
    }
    if id == user.id {
        return Err(AppError::bad(
            "Свою роль менять нельзя",
            "You cannot change your own role",
        ));
    }
    let changed = sqlx::query("update users set role = $2 where id = $1")
        .bind(id)
        .bind(&body.role)
        .execute(&state.db)
        .await?;
    if changed.rows_affected() == 0 {
        return Err(not_found());
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn invites(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
) -> Result<Json<Vec<serde_json::Value>>> {
    admin_only(&user)?;
    type Row = (String, DateTime<Utc>, Option<DateTime<Utc>>, Option<String>);
    let rows: Vec<Row> = sqlx::query_as(
        "select i.token, i.created_at, i.used_at, u.username
             from invites i left join users u on u.id = i.used_by
             order by i.created_at desc",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(token, created, used, by)| {
                serde_json::json!({
                    "token": token,
                    "createdAt": created.to_rfc3339(),
                    "usedAt": used.map(|t| t.to_rfc3339()),
                    "usedBy": by,
                })
            })
            .collect(),
    ))
}

// ponytail: invites never expire; the admin deletes the ones that leaked. Add an expires_at column if that gets tedious.
pub async fn new_invite(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    admin_only(&user)?;
    let token = Uuid::new_v4().simple().to_string();
    sqlx::query("insert into invites (token, created_by) values ($1, $2)")
        .bind(&token)
        .bind(user.id)
        .execute(&state.db)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "token": token })),
    ))
}

pub async fn delete_invite(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(token): Path<String>,
) -> Result<StatusCode> {
    admin_only(&user)?;
    sqlx::query("delete from invites where token = $1")
        .bind(token)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct PublicBody {
    public: bool,
}

pub async fn set_public(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Path(filename): Path<String>,
    Json(body): Json<PublicBody>,
) -> Result<StatusCode> {
    admin_only(&user)?;
    let (id, _) = parse_filename(&filename)?;
    let changed = sqlx::query("update books set public = $2 where id = $1")
        .bind(id)
        .bind(body.public)
        .execute(&state.db)
        .await?;
    if changed.rows_affected() == 0 {
        return Err(not_found());
    }
    Ok(StatusCode::NO_CONTENT)
}
