use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::auth::AuthUser;
use crate::models::error::AppError;
use crate::models::history::HistoryRecord;
use crate::models::item::Item;
use crate::models::space::Space;
use crate::models::sync::*;
use crate::state::AppState;

async fn resolve_user_internal(state: &AppState, public_id: &str) -> Result<i64, AppError> {
    let (id,): (i64,) = sqlx::query_as("SELECT id FROM users WHERE public_id = $1")
        .bind(public_id).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;
    Ok(id)
}

pub async fn sync_pull(
    State(state): State<AppState>, auth: AuthUser, Query(params): Query<SyncPullParams>,
) -> Result<Json<SyncPullResponse>, AppError> {
    let user_internal = resolve_user_internal(&state, &auth.public_id).await?;
    let last_sync_time = params.last_sync_time.unwrap_or_default();

    let created_items: Vec<Item> = sqlx::query_as(
        "SELECT * FROM items WHERE created_at > $1 AND (owner_id = $2 OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'item' AND user_id = $2))"
    ).bind(&last_sync_time).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;

    let updated_items: Vec<Item> = sqlx::query_as(
        "SELECT * FROM items WHERE updated_at > $1 AND created_at <= $2 AND (owner_id = $3 OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'item' AND user_id = $3))"
    ).bind(&last_sync_time).bind(&last_sync_time).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;

    let created_spaces: Vec<Space> = sqlx::query_as(
        "SELECT * FROM spaces WHERE created_at > $1 AND (owner_id = $2 OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'space' AND user_id = $2))"
    ).bind(&last_sync_time).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;

    let updated_spaces: Vec<Space> = sqlx::query_as(
        "SELECT * FROM spaces WHERE updated_at > $1 AND created_at <= $2 AND (owner_id = $3 OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'space' AND user_id = $3))"
    ).bind(&last_sync_time).bind(&last_sync_time).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;

    let created_history: Vec<HistoryRecord> = sqlx::query_as("SELECT h.*, i.public_id AS item_public_id FROM history h JOIN items i ON h.item_id = i.id WHERE h.time > $1")
        .bind(&last_sync_time).fetch_all(&state.db).await.map_err(AppError::Database)?;

    let server_time = chrono::Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    Ok(Json(SyncPullResponse {
        items: SyncEntityChange { created: created_items, updated: updated_items, deleted: vec![] },
        spaces: SyncEntityChange { created: created_spaces, updated: updated_spaces, deleted: vec![] },
        history: SyncHistoryChange { created: created_history, deleted: vec![] },
        server_time, has_more: false,
    }))
}

pub async fn sync_push(
    State(state): State<AppState>, Json(req): Json<SyncPushRequest>,
) -> Result<Json<SyncPushResponse>, AppError> {
    let mut conflicts = Vec::new();

    if let Some(items) = req.items {
        for item in items.created {
            let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM items WHERE id = $1")
                .bind(item.id).fetch_one(&state.db).await.map_err(AppError::Database)?;
            if count > 0 {
                conflicts.push(SyncConflict { entity: "item".to_string(), id: item.public_id.clone(), reason: "version_mismatch".to_string() });
            } else {
                sqlx::query(
                    r#"INSERT INTO items (id, public_id, name, icon, qty, location, location_id, category, tags, barcode, photos, photo_uri, buy_date, expiry, remark, track_low_stock, owner_id, created_at, updated_at)
                       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)"#,
                )
                .bind(item.id).bind(&item.public_id).bind(&item.name).bind(&item.icon).bind(item.qty)
                .bind(&item.location).bind(item.location_id).bind(&item.category).bind(&item.tags)
                .bind(&item.barcode).bind(&item.photos).bind(&item.photo_uri).bind(&item.buy_date)
                .bind(&item.expiry).bind(&item.remark).bind(item.track_low_stock)
                .bind(item.owner_id).bind(&item.created_at).bind(&item.updated_at)
                .execute(&state.db).await.map_err(AppError::Database)?;
            }
        }

        for item in items.updated {
            sqlx::query(
                r#"UPDATE items SET name=$1, icon=$2, qty=$3, location=$4, location_id=$5, category=$6, tags=$7, barcode=$8, photos=$9, photo_uri=$10, buy_date=$11, expiry=$12, remark=$13, track_low_stock=$14, updated_at=$15 WHERE id=$16"#,
            )
            .bind(&item.name).bind(&item.icon).bind(item.qty).bind(&item.location).bind(item.location_id)
            .bind(&item.category).bind(&item.tags).bind(&item.barcode).bind(&item.photos)
            .bind(&item.photo_uri).bind(&item.buy_date).bind(&item.expiry).bind(&item.remark)
            .bind(item.track_low_stock).bind(&item.updated_at).bind(item.id)
            .execute(&state.db).await.map_err(AppError::Database)?;
        }

        for pid in items.deleted {
            let internal: Option<(i64,)> = sqlx::query_as("SELECT id FROM items WHERE public_id = $1")
                .bind(&pid).fetch_optional(&state.db).await.map_err(AppError::Database)?;
            if let Some((id,)) = internal {
                sqlx::query("DELETE FROM items WHERE id = $1").bind(id).execute(&state.db).await.map_err(AppError::Database)?;
            }
        }
    }

    if let Some(spaces) = req.spaces {
        for space in spaces.created {
            let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM spaces WHERE id = $1")
                .bind(space.id).fetch_one(&state.db).await.map_err(AppError::Database)?;
            if count > 0 {
                conflicts.push(SyncConflict { entity: "space".to_string(), id: space.public_id.clone(), reason: "version_mismatch".to_string() });
            } else {
                sqlx::query(
                    r#"INSERT INTO spaces (id, public_id, name, icon, count, parent_id, depth, sort_order, photo_uri, owner_id, created_at, updated_at)
                       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"#,
                )
                .bind(space.id).bind(&space.public_id).bind(&space.name).bind(&space.icon).bind(space.count)
                .bind(space.parent_id).bind(space.depth).bind(space.sort_order).bind(&space.photo_uri)
                .bind(space.owner_id).bind(&space.created_at).bind(&space.updated_at)
                .execute(&state.db).await.map_err(AppError::Database)?;
            }
        }

        for space in spaces.updated {
            sqlx::query(
                "UPDATE spaces SET name=$1, icon=$2, count=$3, parent_id=$4, depth=$5, sort_order=$6, photo_uri=$7, updated_at=$8 WHERE id=$9",
            )
            .bind(&space.name).bind(&space.icon).bind(space.count).bind(space.parent_id)
            .bind(space.depth).bind(space.sort_order).bind(&space.photo_uri).bind(&space.updated_at).bind(space.id)
            .execute(&state.db).await.map_err(AppError::Database)?;
        }

        for pid in spaces.deleted {
            let internal: Option<(i64,)> = sqlx::query_as("SELECT id FROM spaces WHERE public_id = $1")
                .bind(&pid).fetch_optional(&state.db).await.map_err(AppError::Database)?;
            if let Some((id,)) = internal {
                sqlx::query("DELETE FROM spaces WHERE id = $1").bind(id).execute(&state.db).await.map_err(AppError::Database)?;
            }
        }
    }

    if let Some(history) = req.history {
        for record in history.created {
            sqlx::query(
                r#"INSERT INTO history (id, public_id, type, item_id, item_name, qty, from_location, to_location, reason, remark, time)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#,
            )
            .bind(record.id).bind(&record.public_id).bind(&record.r#type).bind(record.item_id)
            .bind(&record.item_name).bind(record.qty).bind(&record.from_location).bind(&record.to_location)
            .bind(&record.reason).bind(&record.remark).bind(&record.time)
            .execute(&state.db).await.map_err(AppError::Database)?;
        }

        for pid in history.deleted {
            let internal: Option<(i64,)> = sqlx::query_as("SELECT id FROM history WHERE public_id = $1")
                .bind(&pid).fetch_optional(&state.db).await.map_err(AppError::Database)?;
            if let Some((id,)) = internal {
                sqlx::query("DELETE FROM history WHERE id = $1").bind(id).execute(&state.db).await.map_err(AppError::Database)?;
            }
        }
    }

    let server_time = chrono::Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    sqlx::query("UPDATE sync_status SET last_sync_time=$1, pending_changes=0 WHERE id=1")
        .bind(&server_time).execute(&state.db).await.map_err(AppError::Database)?;

    Ok(Json(SyncPushResponse { success: true, conflicts, server_time }))
}

pub async fn sync_status(
    State(state): State<AppState>,
) -> Result<Json<SyncStatusResponse>, AppError> {
    let row: (Option<String>, i32) = sqlx::query_as("SELECT last_sync_time, pending_changes FROM sync_status WHERE id = 1")
        .fetch_one(&state.db).await.map_err(AppError::Database)?;
    let server_time = chrono::Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    Ok(Json(SyncStatusResponse { last_sync_time: row.0, pending_changes: row.1, server_time }))
}

#[derive(Debug, Deserialize)]
pub struct SyncPullParams { last_sync_time: Option<String> }
