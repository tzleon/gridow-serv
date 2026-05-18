use axum::extract::{Path, Query, State};
use axum::Json;

use crate::auth::AuthUser;
use crate::models::error::AppError;
use crate::models::history::*;
use crate::state::AppState;

async fn resolve_item_internal(state: &AppState, public_id: &str) -> Result<(i64, i64), AppError> {
    let row: (i64, i64) = sqlx::query_as("SELECT id, owner_id FROM items WHERE public_id = $1")
        .bind(public_id).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;
    Ok(row)
}

async fn resolve_user_internal(state: &AppState, public_id: &str) -> Result<i64, AppError> {
    let (id,): (i64,) = sqlx::query_as("SELECT id FROM users WHERE public_id = $1")
        .bind(public_id).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;
    Ok(id)
}

pub async fn list_history(
    State(state): State<AppState>, auth: AuthUser, Query(params): Query<HistoryQueryParams>,
) -> Result<Json<Vec<HistoryRecord>>, AppError> {
    let user_internal = resolve_user_internal(&state, &auth.public_id).await?;
    let limit = params.limit.clamp(1, 100);

    let mut builder = sqlx::QueryBuilder::new(
        "SELECT h.*, i.public_id AS item_public_id FROM history h JOIN items i ON h.item_id = i.id WHERE (i.owner_id = ",
    );
    builder.push_bind(user_internal);
    builder.push(" OR i.id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'item' AND user_id = ");
    builder.push_bind(user_internal);
    builder.push("))");

    if let Some(ref ht) = params.r#type { builder.push(" AND h.type = "); builder.push_bind(ht); }
    if let Some(ref after) = params.after { builder.push(" AND h.time >= "); builder.push_bind(after); }
    if let Some(ref before) = params.before { builder.push(" AND h.time < "); builder.push_bind(before); }

    builder.push(" ORDER BY h.time DESC LIMIT "); builder.push_bind(limit);

    let records = builder.build_query_as().fetch_all(&state.db).await.map_err(AppError::Database)?;
    Ok(Json(records))
}

pub async fn get_item_history(
    State(state): State<AppState>, auth: AuthUser, Path(item_public_id): Path<String>,
) -> Result<Json<Vec<HistoryRecord>>, AppError> {
    let user_internal = resolve_user_internal(&state, &auth.public_id).await?;
    let (item_internal, owner_id) = resolve_item_internal(&state, &item_public_id).await?;

    if owner_id != user_internal {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM collaborators WHERE entity_type = 'item' AND entity_id = $1 AND user_id = $2"
        ).bind(item_internal).bind(user_internal).fetch_one(&state.db).await.map_err(AppError::Database)?;
        if count == 0 { return Err(AppError::Forbidden); }
    }

    let records = sqlx::query_as::<_, HistoryRecord>(
        "SELECT h.*, i.public_id AS item_public_id FROM history h JOIN items i ON h.item_id = i.id WHERE h.item_id = $1 ORDER BY h.time DESC",
    ).bind(item_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;

    Ok(Json(records))
}
