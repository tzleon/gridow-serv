use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use sqlx::PgPool;

use crate::auth::AuthUser;
use crate::models::error::AppError;
use crate::models::item::Item;
use crate::models::space::*;
use crate::state::AppState;

pub async fn list_spaces(
    State(state): State<AppState>, auth: AuthUser, Query(params): Query<SpaceListParams>,
) -> Result<Json<Vec<Space>>, AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;

    let spaces = if let Some(ref parent_pid) = params.parent_id {
        let parent_internal = state.resolve_space_id(parent_pid).await?;
        sqlx::query_as::<_, Space>(
            "SELECT * FROM spaces WHERE parent_id = $1 AND is_deleted = 0 AND (owner_id = $2 OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'space' AND user_id = $3)) ORDER BY sort_order, name"
        ).bind(parent_internal).bind(user_internal).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?
    } else {
        sqlx::query_as::<_, Space>(
            "SELECT * FROM spaces WHERE parent_id IS NULL AND is_deleted = 0 AND (owner_id = $1 OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'space' AND user_id = $2)) ORDER BY sort_order, name"
        ).bind(user_internal).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?
    };

    Ok(Json(spaces))
}

pub async fn create_space(
    State(state): State<AppState>, auth: AuthUser, Json(req): Json<SpaceCreateRequest>,
) -> Result<(axum::http::StatusCode, Json<Space>), AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let (id, public_id) = state.new_id();
    let now = AppState::now_string();
    let version = state.next_version().await.map_err(AppError::Database)?;

    let (depth, parent_internal) = if let Some(ref pid) = req.parent_id {
        let pi = state.resolve_space_id(pid).await?;
        let parent: Space = sqlx::query_as("SELECT * FROM spaces WHERE id = $1")
            .bind(pi).fetch_optional(&state.db).await
            .map_err(AppError::Database)?.ok_or(AppError::BadRequest("父空间不存在".to_string()))?;
        (parent.depth + 1, Some(pi))
    } else { (0, None) };

    let space = sqlx::query_as::<_, Space>(
        r#"INSERT INTO spaces (id, public_id, name, icon, count, parent_id, depth, sort_order, photo_uri, owner_id, created_at, updated_at, version, is_deleted)
           VALUES ($1, $2, $3, $4, 0, $5, $6, 0, $7, $8, $9, $10, $11, $12) RETURNING *, (SELECT public_id FROM spaces WHERE id = $5) AS parent_public_id"#,
    )
    .bind(id).bind(&public_id).bind(&req.name).bind(&req.icon)
    .bind(parent_internal).bind(depth).bind(&req.photo_uri)
    .bind(user_internal).bind(&now).bind(&now)
    .bind(version).bind(0i16)
    .fetch_one(&state.db).await.map_err(AppError::Database)?;

    Ok((axum::http::StatusCode::CREATED, Json(space)))
}

pub async fn get_space(
    State(state): State<AppState>, auth: AuthUser, Path(space_public_id): Path<String>,
) -> Result<Json<Space>, AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let space = sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE public_id = $1")
        .bind(&space_public_id).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;

    state.check_access(user_internal, "space", space.id, space.owner_id).await?;
    Ok(Json(space))
}

pub async fn update_space(
    State(state): State<AppState>, auth: AuthUser, Path(space_public_id): Path<String>,
    Json(req): Json<SpaceUpdateRequest>,
) -> Result<Json<Space>, AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let existing = sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE public_id = $1")
        .bind(&space_public_id).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;

    state.check_access(user_internal, "space", existing.id, existing.owner_id).await?;

    let name = req.name.unwrap_or(existing.name);
    let icon = req.icon.unwrap_or(existing.icon);
    let photo_uri = req.photo_uri.unwrap_or(existing.photo_uri);
    let sort_order = req.sort_order.unwrap_or(existing.sort_order);

    let (parent_internal, depth) = if let Some(ref parent_pid) = req.parent_id {
        if parent_pid == &space_public_id {
            return Err(AppError::BadRequest("不能将自己设为上级空间".to_string()));
        }
        let pi = state.resolve_space_id(parent_pid).await?;
        if is_descendant(&state.db, existing.id, pi).await {
            return Err(AppError::BadRequest("不能将子空间设为上级空间".to_string()));
        }
        let parent: Space = sqlx::query_as("SELECT * FROM spaces WHERE id = $1")
            .bind(pi).fetch_optional(&state.db).await
            .map_err(AppError::Database)?.ok_or(AppError::BadRequest("上级空间不存在".to_string()))?;
        state.check_access(user_internal, "space", parent.id, parent.owner_id).await?;
        (Some(pi), parent.depth + 1)
    } else {
        (None, 0)
    };

    let now = AppState::now_string();
    let version = state.next_version().await.map_err(AppError::Database)?;

    let space = sqlx::query_as::<_, Space>(
        "UPDATE spaces SET name=$1, icon=$2, photo_uri=$3, sort_order=$4, parent_id=$5, depth=$6, updated_at=$7, version=$8 WHERE id=$9 RETURNING *, (SELECT public_id FROM spaces WHERE id = $5) AS parent_public_id",
    )
    .bind(&name).bind(&icon).bind(&photo_uri).bind(sort_order).bind(parent_internal).bind(depth).bind(&now).bind(version).bind(existing.id)
    .fetch_one(&state.db).await.map_err(AppError::Database)?;

    Ok(Json(space))
}

pub async fn delete_space(
    State(state): State<AppState>, auth: AuthUser, Path(space_public_id): Path<String>,
) -> Result<axum::http::StatusCode, AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let space = sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE public_id = $1")
        .bind(&space_public_id).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;

    if space.owner_id != user_internal { return Err(AppError::Forbidden); }

    let now = AppState::now_string();
    let version = state.next_version().await.map_err(AppError::Database)?;

    soft_delete_space_recursive(&state.db, space.id, &now, version).await?;
    sqlx::query("DELETE FROM collaborators WHERE entity_type = 'space' AND entity_id = $1")
        .bind(space.id).execute(&state.db).await.map_err(AppError::Database)?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn is_descendant(db: &PgPool, ancestor: i64, target: i64) -> bool {
    if ancestor == target { return true; }
    let mut current = target;
    loop {
        let row: Option<(Option<i64>,)> = sqlx::query_as("SELECT parent_id FROM spaces WHERE id = $1")
            .bind(current).fetch_optional(db).await.unwrap_or(None);
        match row {
            Some((Some(pid),)) if pid == ancestor => return true,
            Some((Some(pid),)) => current = pid,
            _ => return false,
        }
    }
}

async fn soft_delete_space_recursive(db: &PgPool, space_internal: i64, now: &str, version: i64) -> Result<(), AppError> {
    let mut stack = vec![space_internal];
    while let Some(current_id) = stack.pop() {
        let children: Vec<Space> = sqlx::query_as("SELECT * FROM spaces WHERE parent_id = $1 AND is_deleted = 0")
            .bind(current_id).fetch_all(db).await.map_err(AppError::Database)?;
        for child in children { stack.push(child.id); }

        sqlx::query("UPDATE items SET location_id=NULL, location='' WHERE location_id = $1")
            .bind(current_id).execute(db).await.map_err(AppError::Database)?;
        sqlx::query("UPDATE spaces SET is_deleted=1, version=$1, updated_at=$2 WHERE id=$3")
            .bind(version).bind(now).bind(current_id).execute(db).await.map_err(AppError::Database)?;
    }
    Ok(())
}

pub async fn get_space_tree(
    State(state): State<AppState>, auth: AuthUser,
) -> Result<Json<Vec<SpaceNode>>, AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;

    let all_spaces: Vec<Space> = sqlx::query_as::<_, Space>(
        "SELECT * FROM spaces WHERE (owner_id = $1 OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'space' AND user_id = $2)) ORDER BY sort_order, name"
    ).bind(user_internal).bind(user_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;

    let space_internals: Vec<i64> = all_spaces.iter().map(|s| s.id).collect();
    let item_locations: Vec<(i64, Option<i64>)> = if space_internals.is_empty() { vec![] } else {
        let mut qb = sqlx::QueryBuilder::new("SELECT id, location_id FROM items WHERE location_id IN (");
        let mut sep = qb.separated(", ");
        for sid in &space_internals { sep.push_bind(sid); }
        sep.push_unseparated(")");
        qb.build_query_as().fetch_all(&state.db).await.map_err(AppError::Database)?
    };

    let public_id_map: std::collections::HashMap<i64, String> = all_spaces.iter().map(|s| (s.id, s.public_id.clone())).collect();
    let item_public_map: std::collections::HashMap<i64, String> = if space_internals.is_empty() { std::collections::HashMap::new() } else {
        let mut qb = sqlx::QueryBuilder::new("SELECT id, public_id FROM items WHERE location_id IN (");
        let mut sep = qb.separated(", ");
        for sid in &space_internals { sep.push_bind(sid); }
        sep.push_unseparated(") AND is_deleted = 0 AND (owner_id = ");
        qb.push_bind(user_internal);
        qb.push(" OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'item' AND user_id = ");
        qb.push_bind(user_internal);
        qb.push("))");
        let items: Vec<(i64, String)> = qb.build_query_as().fetch_all(&state.db).await.map_err(AppError::Database)?;
        items.into_iter().collect()
    };

    let mut item_id_map: std::collections::HashMap<i64, Vec<String>> = std::collections::HashMap::new();
    for (item_internal, loc_internal) in item_locations {
        if let Some(li) = loc_internal {
            if let Some(pid) = item_public_map.get(&item_internal) {
                item_id_map.entry(li).or_default().push(pid.clone());
            }
        }
    }

    let root_nodes = build_space_tree(&all_spaces, None, &item_id_map, &public_id_map);
    Ok(Json(root_nodes))
}

fn build_space_tree(
    spaces: &[Space], parent_id: Option<i64>,
    item_id_map: &std::collections::HashMap<i64, Vec<String>>,
    public_id_map: &std::collections::HashMap<i64, String>,
) -> Vec<SpaceNode> {
    let mut nodes = Vec::new();
    for space in spaces {
        let is_child = match (parent_id, space.parent_id) {
            (None, None) => true, (Some(pid), Some(spid)) => pid == spid, _ => false,
        };
        if is_child {
            let children = build_space_tree(spaces, Some(space.id), item_id_map, public_id_map);
            let item_ids = item_id_map.get(&space.id).cloned().unwrap_or_default();
            nodes.push(SpaceNode {
                id: space.public_id.clone(),
                name: space.name.clone(),
                icon: space.icon.clone(),
                count: space.count,
                parent_id: space.parent_id.and_then(|pid| public_id_map.get(&pid).cloned()),
                depth: space.depth,
                photo_uri: space.photo_uri.clone(),
                children,
                item_ids,
                owner_id: public_id_map.get(&space.owner_id).cloned().unwrap_or_default(),
                version: space.version,
                is_deleted: space.is_deleted,
            });
        }
    }
    nodes
}

pub async fn get_space_children(
    State(state): State<AppState>, auth: AuthUser, Path(space_public_id): Path<String>,
) -> Result<Json<Vec<Space>>, AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let space_internal = state.resolve_space_id(&space_public_id).await?;
    let space = sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = $1")
        .bind(space_internal).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;

    state.check_access(user_internal, "space", space_internal, space.owner_id).await?;

    let children = sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE parent_id = $1 ORDER BY sort_order, name")
        .bind(space_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;

    Ok(Json(children))
}

pub async fn get_space_items(
    State(state): State<AppState>, auth: AuthUser, Path(space_public_id): Path<String>,
) -> Result<Json<Vec<Item>>, AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let space_internal = state.resolve_space_id(&space_public_id).await?;
    let space = sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = $1")
        .bind(space_internal).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;

    state.check_access(user_internal, "space", space_internal, space.owner_id).await?;

    let items = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE location_id = $1")
        .bind(space_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;

    Ok(Json(items))
}

pub async fn get_space_path(
    State(state): State<AppState>, auth: AuthUser, Path(space_public_id): Path<String>,
) -> Result<Json<SpacePathResponse>, AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let space_internal = state.resolve_space_id(&space_public_id).await?;
    let space = sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = $1")
        .bind(space_internal).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;

    state.check_access(user_internal, "space", space_internal, space.owner_id).await?;

    let segments = get_space_path_segments(&state.db, space_internal).await?;
    let path = segments.iter().map(|s| format!("{} {}", s.icon, s.name)).collect::<Vec<_>>().join(" > ");

    Ok(Json(SpacePathResponse { path, segments }))
}

pub async fn get_space_path_segments(pool: &PgPool, space_internal: i64) -> Result<Vec<SpacePathSegment>, AppError> {
    let mut segments = Vec::new();
    let mut current_id = Some(space_internal);
    while let Some(id) = current_id {
        let space: Space = sqlx::query_as("SELECT * FROM spaces WHERE id = $1")
            .bind(id).fetch_optional(pool).await
            .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;

        segments.push(SpacePathSegment { id: space.public_id, name: space.name.clone(), icon: space.icon.clone() });
        current_id = space.parent_id;
    }
    segments.reverse();
    Ok(segments)
}

#[derive(Debug, Deserialize)]
pub struct SpaceListParams { pub parent_id: Option<String> }
