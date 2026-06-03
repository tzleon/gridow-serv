use axum::extract::{Query, State};
use axum::Json;

use crate::auth::AuthUser;
use crate::models::category::{Category, SyncCategoryChange};
use crate::models::error::AppError;
use crate::models::history::HistoryRecord;
use crate::models::item::Item;
use crate::models::space::Space;
use crate::models::sync::*;
use crate::models::tag::{Tag, SyncTagChange};
use crate::state::AppState;

pub async fn sync_pull(
    State(state): State<AppState>, auth: AuthUser, Query(params): Query<SyncPullParams>,
) -> Result<Json<SyncPullResponse>, AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let local_version = params.local_version.unwrap_or(0);

    let created_items: Vec<Item> = sqlx::query_as(
        r#"SELECT * FROM items WHERE version > $1 AND is_deleted = 0 AND (owner_id = $2 OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'item' AND user_id = $2))"#
    ).bind(local_version).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;

    let updated_items: Vec<Item> = sqlx::query_as(
        r#"SELECT * FROM items WHERE version > $1 AND is_deleted = 0 AND id IN (SELECT id FROM items WHERE version <= $1) AND (owner_id = $2 OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'item' AND user_id = $2))"#
    ).bind(local_version).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;

    let deleted_items: Vec<(String,)> = sqlx::query_as(
        r#"SELECT public_id FROM items WHERE version > $1 AND is_deleted = 1 AND (owner_id = $2 OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'item' AND user_id = $2))"#
    ).bind(local_version).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;
    let deleted_items: Vec<String> = deleted_items.into_iter().map(|r| r.0).collect();

    let created_spaces: Vec<Space> = sqlx::query_as(
        r#"SELECT s.*, p.public_id AS parent_public_id FROM spaces s LEFT JOIN spaces p ON s.parent_id = p.id WHERE s.version > $1 AND s.is_deleted = 0 AND (s.owner_id = $2 OR s.id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'space' AND user_id = $2))"#
    ).bind(local_version).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;

    let updated_spaces: Vec<Space> = sqlx::query_as(
        r#"SELECT s.*, p.public_id AS parent_public_id FROM spaces s LEFT JOIN spaces p ON s.parent_id = p.id WHERE s.version > $1 AND s.is_deleted = 0 AND s.id IN (SELECT id FROM spaces WHERE version <= $1) AND (s.owner_id = $2 OR s.id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'space' AND user_id = $2))"#
    ).bind(local_version).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;

    let deleted_spaces: Vec<(String,)> = sqlx::query_as(
        r#"SELECT public_id FROM spaces WHERE version > $1 AND is_deleted = 1 AND (owner_id = $2 OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'space' AND user_id = $2))"#
    ).bind(local_version).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;
    let deleted_spaces: Vec<String> = deleted_spaces.into_iter().map(|r| r.0).collect();

    let created_history: Vec<HistoryRecord> = sqlx::query_as(
        r#"SELECT h.*, i.public_id AS item_public_id FROM history h JOIN items i ON h.item_id = i.id WHERE h.version > $1 AND h.is_deleted = 0"#
    ).bind(local_version).fetch_all(&state.db).await.map_err(AppError::Database)?;

    let deleted_history: Vec<(String,)> = sqlx::query_as(
        r#"SELECT public_id FROM history WHERE version > $1 AND is_deleted = 1"#
    ).bind(local_version).fetch_all(&state.db).await.map_err(AppError::Database)?;
    let deleted_history: Vec<String> = deleted_history.into_iter().map(|r| r.0).collect();

    let created_categories: Vec<Category> = sqlx::query_as(
        r#"SELECT * FROM categories WHERE version > $1 AND is_deleted = 0 AND owner_id = $2"#
    ).bind(local_version).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;

    let updated_categories: Vec<Category> = sqlx::query_as(
        r#"SELECT * FROM categories WHERE version > $1 AND is_deleted = 0 AND id IN (SELECT id FROM categories WHERE version <= $1) AND owner_id = $2"#
    ).bind(local_version).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;

    let deleted_categories: Vec<(String,)> = sqlx::query_as(
        r#"SELECT public_id FROM categories WHERE version > $1 AND is_deleted = 1 AND owner_id = $2"#
    ).bind(local_version).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;
    let deleted_categories: Vec<String> = deleted_categories.into_iter().map(|r| r.0).collect();

    let created_tags: Vec<Tag> = sqlx::query_as(
        r#"SELECT * FROM tags WHERE version > $1 AND is_deleted = 0 AND owner_id = $2"#
    ).bind(local_version).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;

    let updated_tags: Vec<Tag> = sqlx::query_as(
        r#"SELECT * FROM tags WHERE version > $1 AND is_deleted = 0 AND id IN (SELECT id FROM tags WHERE version <= $1) AND owner_id = $2"#
    ).bind(local_version).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;

    let deleted_tags: Vec<(String,)> = sqlx::query_as(
        r#"SELECT public_id FROM tags WHERE version > $1 AND is_deleted = 1 AND owner_id = $2"#
    ).bind(local_version).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;
    let deleted_tags: Vec<String> = deleted_tags.into_iter().map(|r| r.0).collect();

    Ok(Json(SyncPullResponse {
        items: SyncEntityChange { created: created_items, updated: updated_items, deleted: deleted_items }.opt(),
        spaces: SyncEntityChange { created: created_spaces, updated: updated_spaces, deleted: deleted_spaces }.opt(),
        history: SyncHistoryChange { created: created_history, deleted: deleted_history }.opt(),
        categories: SyncCategoryChange { created: created_categories, updated: updated_categories, deleted: deleted_categories }.opt(),
        tags: SyncTagChange { created: created_tags, updated: updated_tags, deleted: deleted_tags }.opt(),
        has_more: false,
    }))
}

pub async fn sync_push(
    State(state): State<AppState>, auth: AuthUser, Json(req): Json<SyncPushRequest>,
) -> Result<Json<SyncPushResponse>, AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let mut assigned_items = Vec::new();
    let mut assigned_spaces = Vec::new();
    let mut assigned_history = Vec::new();
    let mut assigned_categories = Vec::new();
    let mut assigned_tags = Vec::new();

    if let Some(items) = req.items {
        for item in items.created {
            let (id, public_id) = state.new_id();
            let version = state.next_version().await.map_err(AppError::Database)?;

            sqlx::query(
                r#"INSERT INTO items (id, public_id, name, icon, qty, location, location_id, category, tags, barcode, photos, photo_uri, buy_date, expiry, remark, track_low_stock, owner_id, created_at, updated_at, version, is_deleted)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)"#,
            )
            .bind(id).bind(&public_id).bind(&item.name).bind(&item.icon).bind(item.qty)
            .bind(&item.location).bind(item.location_id).bind(&item.category).bind(&item.tags)
            .bind(&item.barcode).bind(&item.photos).bind(&item.photo_uri).bind(&item.buy_date)
            .bind(&item.expiry).bind(&item.remark).bind(item.track_low_stock)
            .bind(user_internal).bind(&item.created_at).bind(&item.updated_at)
            .bind(version).bind(0i16)
            .execute(&state.db).await.map_err(AppError::Database)?;

            assigned_items.push(IdVersionMapping {
                client_id: item.public_id.clone(),
                server_id: public_id,
                version,
            });
        }
    }

    if let Some(spaces) = req.spaces {
        for space in spaces.created {
            let (id, public_id) = state.new_id();
            let version = state.next_version().await.map_err(AppError::Database)?;

            let parent_internal: Option<i64> = if let Some(ref parent_pid) = space.parent_public_id {
                sqlx::query_as("SELECT id FROM spaces WHERE public_id = $1")
                    .bind(parent_pid).fetch_optional(&state.db).await.map_err(AppError::Database)?
                    .map(|(id,): (i64,)| id)
            } else { None };

            sqlx::query(
                r#"INSERT INTO spaces (id, public_id, name, icon, count, parent_id, depth, sort_order, photo_uri, owner_id, created_at, updated_at, version, is_deleted)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)"#,
            )
            .bind(id).bind(&public_id).bind(&space.name).bind(&space.icon).bind(space.count)
            .bind(parent_internal).bind(space.depth).bind(space.sort_order).bind(&space.photo_uri)
            .bind(user_internal).bind(&space.created_at).bind(&space.updated_at)
            .bind(version).bind(0i16)
            .execute(&state.db).await.map_err(AppError::Database)?;

            assigned_spaces.push(IdVersionMapping {
                client_id: space.public_id.clone(),
                server_id: public_id,
                version,
            });
        }
    }

    if let Some(history) = req.history {
        for record in history.created {
            let item_internal = state.resolve_item_id(&record.item_public_id).await.ok();

            if let Some(ii) = item_internal {
                let existing: Option<(i64, String, i64)> = sqlx::query_as(
                    r#"SELECT id, public_id, version FROM history WHERE item_id = $1 AND type = $2 AND time = $3 AND is_deleted = 0 LIMIT 1"#,
                )
                .bind(ii).bind(&record.r#type).bind(&record.time)
                .fetch_optional(&state.db).await.map_err(AppError::Database)?;

                if let Some((_existing_id, existing_public_id, existing_version)) = existing {
                    assigned_history.push(IdVersionMapping {
                        client_id: record.public_id.clone(),
                        server_id: existing_public_id,
                        version: existing_version,
                    });
                    continue;
                }
            } else {
                tracing::warn!("sync_push: item not found for public_id={}, skipping history record", record.item_public_id);
                continue;
            }

            let (id, public_id) = state.new_id();
            let version = state.next_version().await.map_err(AppError::Database)?;

            let item_id = item_internal.unwrap();

            sqlx::query(
                r#"INSERT INTO history (id, public_id, type, item_id, item_name, qty, from_location, to_location, reason, remark, time, version, is_deleted)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)"#,
            )
            .bind(id).bind(&public_id).bind(&record.r#type).bind(item_id)
            .bind(&record.item_name).bind(record.qty).bind(&record.from_location).bind(&record.to_location)
            .bind(&record.reason).bind(&record.remark).bind(&record.time)
            .bind(version).bind(0i16)
            .execute(&state.db).await.map_err(AppError::Database)?;

            assigned_history.push(IdVersionMapping {
                client_id: record.public_id.clone(),
                server_id: public_id,
                version,
            });
        }
    }

    if let Some(categories) = req.categories {
        for cat in categories.created {
            let (id, public_id) = state.new_id();
            let version = state.next_version().await.map_err(AppError::Database)?;

            sqlx::query(
                r#"INSERT INTO categories (id, public_id, name, icon, sort_order, owner_id, created_at, version, is_deleted)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
            )
            .bind(id).bind(&public_id).bind(&cat.name).bind(&cat.icon)
            .bind(cat.sort_order).bind(user_internal).bind(&cat.created_at)
            .bind(version).bind(0i16)
            .execute(&state.db).await.map_err(AppError::Database)?;

            assigned_categories.push(IdVersionMapping {
                client_id: cat.public_id.clone(),
                server_id: public_id,
                version,
            });
        }
    }

    if let Some(tags) = req.tags {
        for tag in tags.created {
            let (id, public_id) = state.new_id();
            let version = state.next_version().await.map_err(AppError::Database)?;

            sqlx::query(
                r#"INSERT INTO tags (id, public_id, name, owner_id, created_at, version, is_deleted)
                   VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
            )
            .bind(id).bind(&public_id).bind(&tag.name)
            .bind(user_internal).bind(&tag.created_at)
            .bind(version).bind(0i16)
            .execute(&state.db).await.map_err(AppError::Database)?;

            assigned_tags.push(IdVersionMapping {
                client_id: tag.public_id.clone(),
                server_id: public_id,
                version,
            });
        }
    }

    Ok(Json(SyncPushResponse {
        success: true,
        conflicts: Vec::new(),
        assigned_items,
        assigned_spaces,
        assigned_history,
        assigned_categories,
        assigned_tags,
    }))
}

pub async fn sync_status(
    State(state): State<AppState>, auth: AuthUser,
) -> Result<Json<SyncStatusResponse>, AppError> {
    let _user_internal = state.resolve_user_id(&auth.public_id).await?;
    let row: (Option<String>, i32) = sqlx::query_as("SELECT last_sync_time, pending_changes FROM sync_status WHERE id = 1")
        .fetch_one(&state.db).await.map_err(AppError::Database)?;

    Ok(Json(SyncStatusResponse { last_sync_time: row.0, pending_changes: row.1 }))
}
