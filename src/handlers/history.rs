//! 操作历史处理器
//!
//! 提供操作历史的查询功能。
//! 历史记录由物品操作（入库/出库/转移）自动生成，不支持手动创建。
//!
//! # 权限模型
//! * 仅返回当前用户拥有或协管的物品的历史记录
//! * 通过 JOIN items 表过滤出授权物品

use axum::extract::{Path, Query, State};
use axum::Json;

use crate::auth::AuthUser;
use crate::models::error::AppError;
use crate::models::history::*;
use crate::state::AppState;

/// 操作历史列表查询
///
/// 支持按操作类型（`type`）筛选、时间段（`after` + `before`）筛选、游标分页（`before` + `limit`）。
/// 通过 JOIN items 过滤：仅返回当前用户拥有或协管的物品的历史。
pub async fn list_history(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<HistoryQueryParams>,
) -> Result<Json<Vec<HistoryRecord>>, AppError> {
    let limit = params.limit.clamp(1, 100);

    let mut builder = sqlx::QueryBuilder::new(
        "SELECT h.* FROM history h \
         JOIN items i ON h.item_id = i.id \
         WHERE (i.owner_id = ",
    );
    builder.push_bind(&auth.user_id);
    builder.push(" OR i.id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'item' AND user_id = ");
    builder.push_bind(&auth.user_id);
    builder.push("))");

    if let Some(ref history_type) = params.r#type {
        builder.push(" AND h.type = ");
        builder.push_bind(history_type);
    }

    if let Some(ref after) = params.after {
        builder.push(" AND h.time >= ");
        builder.push_bind(after);
    }

    if let Some(ref before) = params.before {
        builder.push(" AND h.time < ");
        builder.push_bind(before);
    }

    builder.push(" ORDER BY h.time DESC LIMIT ");
    builder.push_bind(limit);

    let records = builder
        .build_query_as()
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Database)?;

    Ok(Json(records))
}

/// 获取指定物品的操作历史
///
/// 需要用户是该物品的 owner 或协管。
pub async fn get_item_history(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(item_id): Path<String>,
) -> Result<Json<Vec<HistoryRecord>>, AppError> {
    let item: crate::models::item::Item = sqlx::query_as("SELECT * FROM items WHERE id = $1")
        .bind(&item_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    // 校验权限：owner 或协管
    if item.owner_id != auth.user_id {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM collaborators WHERE entity_type = 'item' AND entity_id = $1 AND user_id = $2"
        )
        .bind(&item_id)
        .bind(&auth.user_id)
        .fetch_one(&state.db)
        .await
        .map_err(AppError::Database)?;

        if count == 0 {
            return Err(AppError::Forbidden);
        }
    }

    let records = sqlx::query_as::<_, HistoryRecord>(
        "SELECT * FROM history WHERE item_id = $1 ORDER BY time DESC",
    )
    .bind(&item_id)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Database)?;

    Ok(Json(records))
}
