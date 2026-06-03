use axum::extract::{Path, Query, State};
use axum::Json;

use crate::auth::AuthUser;
use crate::models::error::AppError;
use crate::models::history::*;
use crate::state::AppState;

pub async fn list_history(
    State(state): State<AppState>, auth: AuthUser, Query(params): Query<HistoryQueryParams>,
) -> Result<Json<Vec<HistoryRecord>>, AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
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
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let item_internal = state.resolve_item_id(&item_public_id).await?;

    let (owner_id,): (i64,) = sqlx::query_as("SELECT owner_id FROM items WHERE id = $1")
        .bind(item_internal).fetch_one(&state.db).await.map_err(AppError::Database)?;

    state.check_access(user_internal, "item", item_internal, owner_id).await?;

    let records = sqlx::query_as::<_, HistoryRecord>(
        "SELECT h.*, i.public_id AS item_public_id FROM history h JOIN items i ON h.item_id = i.id WHERE h.item_id = $1 ORDER BY h.time DESC",
    ).bind(item_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;

    Ok(Json(records))
}
