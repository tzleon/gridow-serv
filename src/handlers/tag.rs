use axum::extract::{Path, State};
use axum::Json;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::models::error::AppError;
use crate::models::tag::*;
use crate::state::AppState;

/// 获取当前用户的标签列表
pub async fn list_tags(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<Tag>>, AppError> {
    let tags = sqlx::query_as::<_, Tag>(
        "SELECT * FROM tags WHERE owner_id = $1 ORDER BY name"
    )
        .bind(&auth.user_id)
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Database)?;

    Ok(Json(tags))
}

/// 创建标签
pub async fn create_tag(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<TagCreateRequest>,
) -> Result<(axum::http::StatusCode, Json<Tag>), AppError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    let tag = sqlx::query_as::<_, Tag>(
        r#"INSERT INTO tags (id, name, owner_id, created_at)
           VALUES ($1, $2, $3, $4)
           RETURNING *"#,
    )
        .bind(&id)
        .bind(&req.name)
        .bind(&auth.user_id)
        .bind(&now)
        .fetch_one(&state.db)
        .await
        .map_err(AppError::Database)?;

    Ok((axum::http::StatusCode::CREATED, Json(tag)))
}

/// 更新标签
pub async fn update_tag(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(tag_id): Path<String>,
    Json(req): Json<TagUpdateRequest>,
) -> Result<Json<Tag>, AppError> {
    let existing = sqlx::query_as::<_, Tag>("SELECT * FROM tags WHERE id = $1")
        .bind(&tag_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    if existing.owner_id != auth.user_id {
        return Err(AppError::Forbidden);
    }

    let name = req.name.unwrap_or(existing.name);

    let tag = sqlx::query_as::<_, Tag>(
        "UPDATE tags SET name = $1 WHERE id = $2 RETURNING *"
    )
        .bind(&name)
        .bind(&tag_id)
        .fetch_one(&state.db)
        .await
        .map_err(AppError::Database)?;

    Ok(Json(tag))
}

/// 删除标签
pub async fn delete_tag(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(tag_id): Path<String>,
) -> Result<axum::http::StatusCode, AppError> {
    let existing = sqlx::query_as::<_, Tag>("SELECT * FROM tags WHERE id = $1")
        .bind(&tag_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    if existing.owner_id != auth.user_id {
        return Err(AppError::Forbidden);
    }

    sqlx::query("DELETE FROM tags WHERE id = $1")
        .bind(&tag_id)
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}
