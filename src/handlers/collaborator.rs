//! 协管管理处理器
//!
//! 提供物品和空间的协管（Collaborator）管理功能：
//! * 添加协管 — 仅 owner 可操作
//! * 移除协管 — 仅 owner 可操作
//! * 协管列表 — 所有人可查看（JOIN users 表获取协管详情）
//!
//! # 设计说明
//! 内部使用三个通用函数（`add_collaborator_inner` 等）避免代码重复，
//! 对外暴露 item 和 space 两套接口以保持 RESTful 路径结构。

use axum::extract::{Path, State};
use axum::Json;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::models::collaborator::*;
use crate::models::error::AppError;
use crate::state::AppState;

/// 添加协管的内部实现
///
/// 验证请求者是 owner 后，向 `collaborators` 表写入记录。
/// 使用 `ON CONFLICT ... DO UPDATE` 支持幂等操作。
async fn add_collaborator_inner(
    state: &AppState,
    auth: &AuthUser,
    entity_type: &str,
    entity_id: &str,
    target_user_id: &str,
) -> Result<Collaborator, AppError> {
    // 验证请求者是 owner
    let owner_id: String = match entity_type {
        "item" => {
            let row: (String,) = sqlx::query_as("SELECT owner_id FROM items WHERE id = $1")
                .bind(entity_id)
                .fetch_optional(&state.db)
                .await
                .map_err(AppError::Database)?
                .ok_or(AppError::NotFound)?;
            row.0
        }
        "space" => {
            let row: (String,) = sqlx::query_as("SELECT owner_id FROM spaces WHERE id = $1")
                .bind(entity_id)
                .fetch_optional(&state.db)
                .await
                .map_err(AppError::Database)?
                .ok_or(AppError::NotFound)?;
            row.0
        }
        _ => unreachable!(),
    };

    if owner_id != auth.user_id {
        return Err(AppError::Forbidden);
    }

    let id = Uuid::new_v4().to_string();
    let now = crate::models::item::now_string();

    // ON CONFLICT 实现幂等：重复添加同一用户不会报错
    let collaborator = sqlx::query_as::<_, Collaborator>(
        r#"INSERT INTO collaborators (id, entity_type, entity_id, user_id, created_at)
           VALUES ($1, $2, $3, $4, $5)
           ON CONFLICT (entity_type, entity_id, user_id) DO UPDATE SET user_id = $4
           RETURNING *"#,
    )
    .bind(&id)
    .bind(entity_type)
    .bind(entity_id)
    .bind(target_user_id)
    .bind(&now)
    .fetch_one(&state.db)
    .await
    .map_err(AppError::Database)?;

    Ok(collaborator)
}

/// 移除协管的内部实现
async fn remove_collaborator_inner(
    state: &AppState,
    auth: &AuthUser,
    entity_type: &str,
    entity_id: &str,
    target_user_id: &str,
) -> Result<(), AppError> {
    // 验证请求者是 owner
    let owner_id: String = match entity_type {
        "item" => {
            let row: (String,) = sqlx::query_as("SELECT owner_id FROM items WHERE id = $1")
                .bind(entity_id)
                .fetch_optional(&state.db)
                .await
                .map_err(AppError::Database)?
                .ok_or(AppError::NotFound)?;
            row.0
        }
        "space" => {
            let row: (String,) = sqlx::query_as("SELECT owner_id FROM spaces WHERE id = $1")
                .bind(entity_id)
                .fetch_optional(&state.db)
                .await
                .map_err(AppError::Database)?
                .ok_or(AppError::NotFound)?;
            row.0
        }
        _ => unreachable!(),
    };

    if owner_id != auth.user_id {
        return Err(AppError::Forbidden);
    }

    sqlx::query("DELETE FROM collaborators WHERE entity_type = $1 AND entity_id = $2 AND user_id = $3")
        .bind(entity_type)
        .bind(entity_id)
        .bind(target_user_id)
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?;

    Ok(())
}

/// 获取协管列表的内部实现
///
/// JOIN users 表获取协管用户的详细信息（用户名、邮箱、头像）。
async fn list_collaborators_inner(
    state: &AppState,
    entity_type: &str,
    entity_id: &str,
) -> Result<Vec<CollaboratorInfo>, AppError> {
    let collaborators = sqlx::query_as::<_, CollaboratorInfo>(
        r#"SELECT c.id, c.user_id, u.username, u.email, u.avatar
           FROM collaborators c
           JOIN users u ON c.user_id = u.id
           WHERE c.entity_type = $1 AND c.entity_id = $2
           ORDER BY c.created_at"#,
    )
    .bind(entity_type)
    .bind(entity_id)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Database)?;

    Ok(collaborators)
}

// ── 物品协管 API ────────────────────────────────────────────

/// 添加物品协管
pub async fn add_item_collaborator(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(item_id): Path<String>,
    Json(req): Json<AddCollaboratorRequest>,
) -> Result<Json<Collaborator>, AppError> {
    let c = add_collaborator_inner(&state, &auth, "item", &item_id, &req.user_id).await?;
    Ok(Json(c))
}

/// 移除物品协管
pub async fn remove_item_collaborator(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((item_id, user_id)): Path<(String, String)>,
) -> Result<axum::http::StatusCode, AppError> {
    remove_collaborator_inner(&state, &auth, "item", &item_id, &user_id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// 获取物品协管列表
pub async fn list_item_collaborators(
    State(state): State<AppState>,
    Path(item_id): Path<String>,
) -> Result<Json<CollaboratorListResponse>, AppError> {
    let collaborators = list_collaborators_inner(&state, "item", &item_id).await?;
    Ok(Json(CollaboratorListResponse { collaborators }))
}

// ── 空间协管 API ────────────────────────────────────────────

/// 添加空间协管
pub async fn add_space_collaborator(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(space_id): Path<String>,
    Json(req): Json<AddCollaboratorRequest>,
) -> Result<Json<Collaborator>, AppError> {
    let c = add_collaborator_inner(&state, &auth, "space", &space_id, &req.user_id).await?;
    Ok(Json(c))
}

/// 移除空间协管
pub async fn remove_space_collaborator(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((space_id, user_id)): Path<(String, String)>,
) -> Result<axum::http::StatusCode, AppError> {
    remove_collaborator_inner(&state, &auth, "space", &space_id, &user_id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// 获取空间协管列表
pub async fn list_space_collaborators(
    State(state): State<AppState>,
    Path(space_id): Path<String>,
) -> Result<Json<CollaboratorListResponse>, AppError> {
    let collaborators = list_collaborators_inner(&state, "space", &space_id).await?;
    Ok(Json(CollaboratorListResponse { collaborators }))
}
