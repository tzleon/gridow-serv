use axum::extract::{Path, Query, State};
use axum::Json;

use crate::models::error::AppError;
use crate::models::history::*;
use crate::state::AppState;

pub async fn list_history(
    State(state): State<AppState>,
    Query(params): Query<HistoryQueryParams>,
) -> Result<Json<Vec<HistoryRecord>>, AppError> {
    let limit = params.limit.clamp(1, 100);

    let records = if let Some(ref history_type) = params.r#type {
        if let Some(ref before) = params.before {
            sqlx::query_as::<_, HistoryRecord>(
                "SELECT * FROM history WHERE type = $1 AND time < $2 ORDER BY time DESC LIMIT $3",
            )
            .bind(history_type)
            .bind(before)
            .bind(limit)
            .fetch_all(&state.db)
            .await
            .map_err(AppError::Database)?
        } else {
            sqlx::query_as::<_, HistoryRecord>(
                "SELECT * FROM history WHERE type = $1 ORDER BY time DESC LIMIT $2",
            )
            .bind(history_type)
            .bind(limit)
            .fetch_all(&state.db)
            .await
            .map_err(AppError::Database)?
        }
    } else if let Some(ref before) = params.before {
        sqlx::query_as::<_, HistoryRecord>(
            "SELECT * FROM history WHERE time < $1 ORDER BY time DESC LIMIT $2",
        )
        .bind(before)
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Database)?
    } else {
        sqlx::query_as::<_, HistoryRecord>(
            "SELECT * FROM history ORDER BY time DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Database)?
    };

    Ok(Json(records))
}

pub async fn get_item_history(
    State(state): State<AppState>,
    Path(item_id): Path<String>,
) -> Result<Json<Vec<HistoryRecord>>, AppError> {
    let records = sqlx::query_as::<_, HistoryRecord>(
        "SELECT * FROM history WHERE item_id = $1 ORDER BY time DESC",
    )
    .bind(&item_id)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Database)?;

    Ok(Json(records))
}
