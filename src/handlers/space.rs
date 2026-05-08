use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::error::AppError;
use crate::models::item::Item;
use crate::models::space::*;
use crate::state::AppState;

pub async fn list_spaces(
    State(state): State<AppState>,
    Query(params): Query<SpaceListParams>,
) -> Result<Json<Vec<Space>>, AppError> {
    let spaces = if let Some(ref parent_id) = params.parent_id {
        sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE parent_id = ? ORDER BY sort_order, name")
            .bind(parent_id)
            .fetch_all(&state.db)
            .await
            .map_err(AppError::Database)?
    } else {
        sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE parent_id IS NULL ORDER BY sort_order, name")
            .fetch_all(&state.db)
            .await
            .map_err(AppError::Database)?
    };

    Ok(Json(spaces))
}

pub async fn create_space(
    State(state): State<AppState>,
    Json(req): Json<SpaceCreateRequest>,
) -> Result<(axum::http::StatusCode, Json<Space>), AppError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    let (depth, parent_id) = if let Some(ref pid) = req.parent_id {
        let parent: Space = sqlx::query_as("SELECT * FROM spaces WHERE id = ?")
            .bind(pid)
            .fetch_optional(&state.db)
            .await
            .map_err(AppError::Database)?
            .ok_or(AppError::BadRequest("父空间不存在".to_string()))?;
        (parent.depth + 1, Some(pid.clone()))
    } else {
        (0, None)
    };

    let space = sqlx::query_as::<_, Space>(
        r#"INSERT INTO spaces (id, name, icon, count, parent_id, depth, sort_order, photo_uri, created_at, updated_at)
           VALUES (?, ?, ?, 0, ?, ?, 0, ?, ?, ?)
           RETURNING *"#,
    )
    .bind(&id)
    .bind(&req.name)
    .bind(&req.icon)
    .bind(&parent_id)
    .bind(depth)
    .bind(&req.photo_uri)
    .bind(&now)
    .bind(&now)
    .fetch_one(&state.db)
    .await
    .map_err(AppError::Database)?;

    Ok((axum::http::StatusCode::CREATED, Json(space)))
}

pub async fn get_space(
    State(state): State<AppState>,
    Path(space_id): Path<String>,
) -> Result<Json<Space>, AppError> {
    let space = sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = ?")
        .bind(&space_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    Ok(Json(space))
}

pub async fn update_space(
    State(state): State<AppState>,
    Path(space_id): Path<String>,
    Json(req): Json<SpaceUpdateRequest>,
) -> Result<Json<Space>, AppError> {
    let existing = sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = ?")
        .bind(&space_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    let name = req.name.unwrap_or(existing.name);
    let icon = req.icon.unwrap_or(existing.icon);
    let photo_uri = req.photo_uri.unwrap_or(existing.photo_uri);
    let sort_order = req.sort_order.unwrap_or(existing.sort_order);
    let now = chrono::Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    let space = sqlx::query_as::<_, Space>(
        "UPDATE spaces SET name=?, icon=?, photo_uri=?, sort_order=?, updated_at=? WHERE id=? RETURNING *",
    )
    .bind(&name)
    .bind(&icon)
    .bind(&photo_uri)
    .bind(sort_order)
    .bind(&now)
    .bind(&space_id)
    .fetch_one(&state.db)
    .await
    .map_err(AppError::Database)?;

    Ok(Json(space))
}

pub async fn delete_space(
    State(state): State<AppState>,
    Path(space_id): Path<String>,
) -> Result<axum::http::StatusCode, AppError> {
    let _space = sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = ?")
        .bind(&space_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    delete_space_recursive(&state, &space_id).await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn delete_space_recursive(state: &AppState, space_id: &str) -> Result<(), AppError> {
    let mut stack = vec![space_id.to_string()];

    while let Some(current_id) = stack.pop() {
        let children: Vec<Space> =
            sqlx::query_as("SELECT * FROM spaces WHERE parent_id = ?")
                .bind(&current_id)
                .fetch_all(&state.db)
                .await
                .map_err(AppError::Database)?;

        for child in children {
            stack.push(child.id);
        }

        sqlx::query("UPDATE items SET location_id=NULL, location='' WHERE location_id = ?")
            .bind(&current_id)
            .execute(&state.db)
            .await
            .map_err(AppError::Database)?;

        sqlx::query("DELETE FROM spaces WHERE id = ?")
            .bind(&current_id)
            .execute(&state.db)
            .await
            .map_err(AppError::Database)?;
    }

    Ok(())
}

pub async fn get_space_tree(
    State(state): State<AppState>,
) -> Result<Json<Vec<SpaceNode>>, AppError> {
    let all_spaces: Vec<Space> =
        sqlx::query_as("SELECT * FROM spaces ORDER BY sort_order, name")
            .fetch_all(&state.db)
            .await
            .map_err(AppError::Database)?;

    let item_locations: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT id, location_id FROM items")
            .fetch_all(&state.db)
            .await
            .map_err(AppError::Database)?;

    let mut item_id_map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for (item_id, location_id) in item_locations {
        if let Some(loc_id) = location_id {
            item_id_map.entry(loc_id).or_default().push(item_id);
        }
    }

    let root_nodes = build_space_tree(&all_spaces, None, &item_id_map);
    Ok(Json(root_nodes))
}

fn build_space_tree(
    spaces: &[Space],
    parent_id: Option<&str>,
    item_id_map: &std::collections::HashMap<String, Vec<String>>,
) -> Vec<SpaceNode> {
    let mut nodes = Vec::new();

    for space in spaces {
        let is_child = match (parent_id, &space.parent_id) {
            (None, None) => true,
            (Some(pid), Some(spid)) => pid == spid,
            _ => false,
        };

        if is_child {
            let children = build_space_tree(spaces, Some(&space.id), item_id_map);

            let item_ids = item_id_map.get(&space.id).cloned().unwrap_or_default();

            let node = SpaceNode {
                id: space.id.clone(),
                name: space.name.clone(),
                icon: space.icon.clone(),
                count: space.count,
                parent_id: space.parent_id.clone(),
                depth: space.depth,
                photo_uri: space.photo_uri.clone(),
                children,
                item_ids,
            };
            nodes.push(node);
        }
    }

    nodes
}

pub async fn get_space_children(
    State(state): State<AppState>,
    Path(space_id): Path<String>,
) -> Result<Json<Vec<Space>>, AppError> {
    let _space = sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = ?")
        .bind(&space_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    let children =
        sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE parent_id = ? ORDER BY sort_order, name")
            .bind(&space_id)
            .fetch_all(&state.db)
            .await
            .map_err(AppError::Database)?;

    Ok(Json(children))
}

pub async fn get_space_items(
    State(state): State<AppState>,
    Path(space_id): Path<String>,
) -> Result<Json<Vec<Item>>, AppError> {
    let _space = sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = ?")
        .bind(&space_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    let items = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE location_id = ?")
        .bind(&space_id)
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Database)?;

    Ok(Json(items))
}

pub async fn get_space_path(
    State(state): State<AppState>,
    Path(space_id): Path<String>,
) -> Result<Json<SpacePathResponse>, AppError> {
    let segments = get_space_path_segments(&state.db, &space_id).await?;

    let path = segments
        .iter()
        .map(|s| format!("{} {}", s.icon, s.name))
        .collect::<Vec<_>>()
        .join(" > ");

    Ok(Json(SpacePathResponse { path, segments }))
}

pub async fn get_space_path_segments(
    pool: &PgPool,
    space_id: &str,
) -> Result<Vec<SpacePathSegment>, AppError> {
    let mut segments = Vec::new();
    let mut current_id = Some(space_id.to_string());

    while let Some(id) = current_id {
        let space: Space = sqlx::query_as("SELECT * FROM spaces WHERE id = ?")
            .bind(&id)
            .fetch_optional(pool)
            .await
            .map_err(AppError::Database)?
            .ok_or(AppError::NotFound)?;

        segments.push(SpacePathSegment {
            id: space.id.clone(),
            name: space.name.clone(),
            icon: space.icon.clone(),
        });

        current_id = space.parent_id;
    }

    segments.reverse();
    Ok(segments)
}

#[derive(Debug, Deserialize)]
pub struct SpaceListParams {
    parent_id: Option<String>,
}
