use axum::extract::{Path, State};
use axum::Json;

use crate::auth::AuthUser;
use crate::models::error::AppError;
use crate::models::tag::*;
use crate::state::AppState;

async fn resolve_user_internal(state: &AppState, public_id: &str) -> Result<i64, AppError> {
    let (id,): (i64,) = sqlx::query_as("SELECT id FROM users WHERE public_id = $1")
        .bind(public_id).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;
    Ok(id)
}

pub async fn list_tags(
    State(state): State<AppState>, auth: AuthUser,
) -> Result<Json<Vec<Tag>>, AppError> {
    let user_internal = resolve_user_internal(&state, &auth.public_id).await?;
    let tags = sqlx::query_as::<_, Tag>("SELECT * FROM tags WHERE owner_id = $1 AND is_deleted = 0 ORDER BY name")
        .bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;
    Ok(Json(tags))
}

pub async fn create_tag(
    State(state): State<AppState>, auth: AuthUser, Json(req): Json<TagCreateRequest>,
) -> Result<(axum::http::StatusCode, Json<Tag>), AppError> {
    let user_internal = resolve_user_internal(&state, &auth.public_id).await?;
    let (id, public_id) = state.new_id();
    let now = chrono::Utc::now().naive_utc().format("%Y-%m-%d %H:%M:%S").to_string();
    let version = state.next_version().await.map_err(AppError::Database)?;

    let tag = sqlx::query_as::<_, Tag>(
        r#"INSERT INTO tags (id, public_id, name, owner_id, created_at, version, is_deleted)
           VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *"#,
    ).bind(id).bind(&public_id).bind(&req.name).bind(user_internal).bind(&now)
    .bind(version).bind(0i16)
    .fetch_one(&state.db).await.map_err(AppError::Database)?;

    Ok((axum::http::StatusCode::CREATED, Json(tag)))
}

pub async fn update_tag(
    State(state): State<AppState>, auth: AuthUser, Path(tag_public_id): Path<String>,
    Json(req): Json<TagUpdateRequest>,
) -> Result<Json<Tag>, AppError> {
    let user_internal = resolve_user_internal(&state, &auth.public_id).await?;
    let existing = sqlx::query_as::<_, Tag>("SELECT * FROM tags WHERE public_id = $1 AND is_deleted = 0")
        .bind(&tag_public_id).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;

    if existing.owner_id != user_internal { return Err(AppError::Forbidden); }

    let name = req.name.unwrap_or(existing.name);
    let version = state.next_version().await.map_err(AppError::Database)?;

    let tag = sqlx::query_as::<_, Tag>("UPDATE tags SET name = $1, version = $2 WHERE id = $3 RETURNING *")
        .bind(&name).bind(version).bind(existing.id).fetch_one(&state.db).await.map_err(AppError::Database)?;

    Ok(Json(tag))
}

pub async fn delete_tag(
    State(state): State<AppState>, auth: AuthUser, Path(tag_public_id): Path<String>,
) -> Result<axum::http::StatusCode, AppError> {
    let user_internal = resolve_user_internal(&state, &auth.public_id).await?;
    let existing = sqlx::query_as::<_, Tag>("SELECT * FROM tags WHERE public_id = $1 AND is_deleted = 0")
        .bind(&tag_public_id).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;

    if existing.owner_id != user_internal { return Err(AppError::Forbidden); }

    let version = state.next_version().await.map_err(AppError::Database)?;

    sqlx::query("UPDATE tags SET is_deleted = 1, version = $1 WHERE id = $2")
        .bind(version).bind(existing.id).execute(&state.db).await.map_err(AppError::Database)?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}
