use axum::extract::{Path, State};
use axum::Json;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::models::category::*;
use crate::models::error::AppError;
use crate::state::AppState;

/// 获取当前用户的分类列表
///
/// 若用户尚无分类，自动创建默认分类集合并返回。
pub async fn list_categories(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<Category>>, AppError> {
    let categories = sqlx::query_as::<_, Category>(
        "SELECT * FROM categories WHERE owner_id = $1 ORDER BY sort_order, name"
    )
        .bind(&auth.user_id)
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Database)?;

    if categories.is_empty() {
        let defaults = vec![
            ("日用品", "🧴"),
            ("食品", "🍎"),
            ("工具", "🔧"),
            ("药品", "💊"),
            ("服装", "👕"),
            ("电子", "🔌"),
        ];
        let now = chrono::Utc::now()
            .naive_utc()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        let mut created = Vec::new();
        for (i, (name, icon)) in defaults.iter().enumerate() {
            let id = uuid::Uuid::new_v4().to_string();
            let cat = sqlx::query_as::<_, Category>(
                r#"INSERT INTO categories (id, name, icon, sort_order, owner_id, created_at)
                   VALUES ($1, $2, $3, $4, $5, $6)
                   RETURNING *"#,
            )
                .bind(&id)
                .bind(name)
                .bind(icon)
                .bind(i as i32)
                .bind(&auth.user_id)
                .bind(&now)
                .fetch_one(&state.db)
                .await
                .map_err(AppError::Database)?;
            created.push(cat);
        }

        return Ok(Json(created));
    }

    Ok(Json(categories))
}

/// 创建分类
pub async fn create_category(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CategoryCreateRequest>,
) -> Result<(axum::http::StatusCode, Json<Category>), AppError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    let next_order: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM categories WHERE owner_id = $1"
    )
        .bind(&auth.user_id)
        .fetch_one(&state.db)
        .await
        .map_err(AppError::Database)?;

    let category = sqlx::query_as::<_, Category>(
        r#"INSERT INTO categories (id, name, icon, sort_order, owner_id, created_at)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING *"#,
    )
        .bind(&id)
        .bind(&req.name)
        .bind(&req.icon)
        .bind(next_order)
        .bind(&auth.user_id)
        .bind(&now)
        .fetch_one(&state.db)
        .await
        .map_err(AppError::Database)?;

    Ok((axum::http::StatusCode::CREATED, Json(category)))
}

/// 更新分类
pub async fn update_category(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(category_id): Path<String>,
    Json(req): Json<CategoryUpdateRequest>,
) -> Result<Json<Category>, AppError> {
    let existing = sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE id = $1")
        .bind(&category_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    if existing.owner_id != auth.user_id {
        return Err(AppError::Forbidden);
    }

    let name = req.name.unwrap_or(existing.name);
    let icon = req.icon.unwrap_or(existing.icon);

    let category = sqlx::query_as::<_, Category>(
        "UPDATE categories SET name = $1, icon = $2 WHERE id = $3 RETURNING *"
    )
        .bind(&name)
        .bind(&icon)
        .bind(&category_id)
        .fetch_one(&state.db)
        .await
        .map_err(AppError::Database)?;

    Ok(Json(category))
}

/// 删除分类
pub async fn delete_category(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(category_id): Path<String>,
) -> Result<axum::http::StatusCode, AppError> {
    let existing = sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE id = $1")
        .bind(&category_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    if existing.owner_id != auth.user_id {
        return Err(AppError::Forbidden);
    }

    sqlx::query("DELETE FROM categories WHERE id = $1")
        .bind(&category_id)
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}
