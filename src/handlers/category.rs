use axum::extract::{Path, State};
use axum::Json;

use crate::auth::AuthUser;
use crate::models::category::*;
use crate::models::error::AppError;
use crate::state::AppState;

async fn resolve_user_internal(state: &AppState, public_id: &str) -> Result<i64, AppError> {
    let (id,): (i64,) = sqlx::query_as("SELECT id FROM users WHERE public_id = $1")
        .bind(public_id).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;
    Ok(id)
}

pub async fn list_categories(
    State(state): State<AppState>, auth: AuthUser,
) -> Result<Json<Vec<Category>>, AppError> {
    let user_internal = resolve_user_internal(&state, &auth.public_id).await?;

    let categories = sqlx::query_as::<_, Category>(
        r#"SELECT c.*, COALESCE(COUNT(i.id), 0) AS item_count, MAX(i.updated_at) AS last_used_at
           FROM categories c LEFT JOIN items i ON i.category = c.name AND i.owner_id = $1
           WHERE c.owner_id = $1 AND c.is_deleted = 0 GROUP BY c.id
           ORDER BY CASE WHEN c.created_at::timestamp >= (NOW() - INTERVAL '12 hours') THEN 0 ELSE 1 END,
                    COUNT(i.id) DESC, MAX(i.updated_at) DESC NULLS LAST, c.created_at DESC"#,
    ).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;

    Ok(Json(categories))
}

pub async fn create_category(
    State(state): State<AppState>, auth: AuthUser, Json(req): Json<CategoryCreateRequest>,
) -> Result<(axum::http::StatusCode, Json<Category>), AppError> {
    let user_internal = resolve_user_internal(&state, &auth.public_id).await?;
    let (id, public_id) = state.new_id();
    let now = chrono::Utc::now().naive_utc().format("%Y-%m-%d %H:%M:%S").to_string();
    let version = state.next_version().await.map_err(AppError::Database)?;

    let next_order: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM categories WHERE owner_id = $1"
    ).bind(user_internal).fetch_one(&state.db).await.map_err(AppError::Database)?;

    let category = sqlx::query_as::<_, Category>(
        r#"INSERT INTO categories (id, public_id, name, icon, sort_order, owner_id, created_at, version, is_deleted)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING *"#,
    ).bind(id).bind(&public_id).bind(&req.name).bind(&req.icon).bind(next_order).bind(user_internal).bind(&now)
    .bind(version).bind(0i16)
    .fetch_one(&state.db).await.map_err(AppError::Database)?;

    Ok((axum::http::StatusCode::CREATED, Json(category)))
}

pub async fn update_category(
    State(state): State<AppState>, auth: AuthUser, Path(cat_public_id): Path<String>,
    Json(req): Json<CategoryUpdateRequest>,
) -> Result<Json<Category>, AppError> {
    let user_internal = resolve_user_internal(&state, &auth.public_id).await?;
    let existing = sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE public_id = $1 AND is_deleted = 0")
        .bind(&cat_public_id).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;

    if existing.owner_id != user_internal { return Err(AppError::Forbidden); }

    let name = req.name.unwrap_or(existing.name);
    let icon = req.icon.unwrap_or(existing.icon);
    let version = state.next_version().await.map_err(AppError::Database)?;

    let category = sqlx::query_as::<_, Category>(
        "UPDATE categories SET name = $1, icon = $2, version = $3 WHERE id = $4 RETURNING *"
    ).bind(&name).bind(&icon).bind(version).bind(existing.id).fetch_one(&state.db).await.map_err(AppError::Database)?;

    Ok(Json(category))
}

pub async fn delete_category(
    State(state): State<AppState>, auth: AuthUser, Path(cat_public_id): Path<String>,
) -> Result<axum::http::StatusCode, AppError> {
    let user_internal = resolve_user_internal(&state, &auth.public_id).await?;
    let existing = sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE public_id = $1 AND is_deleted = 0")
        .bind(&cat_public_id).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;

    if existing.owner_id != user_internal { return Err(AppError::Forbidden); }

    let version = state.next_version().await.map_err(AppError::Database)?;

    sqlx::query("UPDATE categories SET is_deleted = 1, version = $1 WHERE id = $2")
        .bind(version).bind(existing.id).execute(&state.db).await.map_err(AppError::Database)?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}
