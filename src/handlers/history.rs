use axum::extract::{Path, Query, State};
use axum::Json;

use crate::auth::AuthUser;
use crate::models::error::AppError;
use crate::models::history::*;
use crate::state::AppState;

pub async fn list_history(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<HistoryQueryParams>,
) -> Result<Json<Vec<HistoryRecord>>, AppError> {
    let limit = params.limit.clamp(1, 100);

    let records = if let Some(ref history_type) = params.r#type {
        if let Some(ref before) = params.before {
            sqlx::query_as::<_, HistoryRecord>(
                r#"SELECT h.* FROM history h
                   JOIN items i ON h.item_id = i.id
                   WHERE h.type = $1 AND h.time < $2
                   AND (i.owner_id = $3 OR i.id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'item' AND user_id = $3))
                   ORDER BY h.time DESC LIMIT $4"#,
            )
            .bind(history_type)
            .bind(before)
            .bind(&auth.user_id)
            .bind(limit)
            .fetch_all(&state.db)
            .await
            .map_err(AppError::Database)?
        } else {
            sqlx::query_as::<_, HistoryRecord>(
                r#"SELECT h.* FROM history h
                   JOIN items i ON h.item_id = i.id
                   WHERE h.type = $1
                   AND (i.owner_id = $2 OR i.id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'item' AND user_id = $2))
                   ORDER BY h.time DESC LIMIT $3"#,
            )
            .bind(history_type)
            .bind(&auth.user_id)
            .bind(limit)
            .fetch_all(&state.db)
            .await
            .map_err(AppError::Database)?
        }
    } else if let Some(ref before) = params.before {
        sqlx::query_as::<_, HistoryRecord>(
            r#"SELECT h.* FROM history h
               JOIN items i ON h.item_id = i.id
               WHERE h.time < $1
               AND (i.owner_id = $2 OR i.id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'item' AND user_id = $2))
               ORDER BY h.time DESC LIMIT $3"#,
        )
        .bind(before)
        .bind(&auth.user_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Database)?
    } else {
        sqlx::query_as::<_, HistoryRecord>(
            r#"SELECT h.* FROM history h
               JOIN items i ON h.item_id = i.id
               WHERE (i.owner_id = $1 OR i.id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'item' AND user_id = $1))
               ORDER BY h.time DESC LIMIT $2"#,
        )
        .bind(&auth.user_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Database)?
    };

    Ok(Json(records))
}

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
