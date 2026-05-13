//! 空间管理处理器
//!
//! 提供空间（Space）的树形结构管理：
//! * 增删改查（含按父节点筛选的子空间列表）
//! * 空间树（返回当前用户授权的完整嵌套结构）
//! * 子空间 / 空间下物品 / 空间路径查询
//!
//! # 权限模型
//! * **查询** — 仅返回当前用户拥有或协管的空间
//! * **修改** — owner 或协管可操作
//! * **删除** — 仅 owner 可操作（递归删除子空间，释放关联物品）
//! * **创建** — 任何已登录用户可创建

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::models::error::AppError;
use crate::models::item::Item;
use crate::models::space::*;
use crate::state::AppState;

/// 权限校验：检查用户是否为实体的 owner 或协管
async fn check_access(pool: &PgPool, user_id: &str, entity_type: &str, entity_id: &str, owner_id: &str) -> Result<(), AppError> {
    if owner_id == user_id {
        return Ok(());
    }
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM collaborators WHERE entity_type = $1 AND entity_id = $2 AND user_id = $3"
    )
    .bind(entity_type)
    .bind(entity_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(AppError::Database)?;

    if count > 0 {
        return Ok(());
    }
    Err(AppError::Forbidden)
}

/// 获取空间列表
///
/// 可通过 `parentId` 过滤指定父节点下的子空间；
/// 不传则返回顶级空间。
pub async fn list_spaces(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<SpaceListParams>,
) -> Result<Json<Vec<Space>>, AppError> {
    let owner_condition = format!(
        " AND (owner_id = '{}' OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'space' AND user_id = '{}'))",
        auth.user_id, auth.user_id
    );

    let spaces = if let Some(ref parent_id) = params.parent_id {
        let query = format!(
            "SELECT * FROM spaces WHERE parent_id = $1{} ORDER BY sort_order, name",
            owner_condition
        );
        sqlx::query_as::<_, Space>(&query)
            .bind(parent_id)
            .fetch_all(&state.db)
            .await
            .map_err(AppError::Database)?
    } else {
        let query = format!(
            "SELECT * FROM spaces WHERE parent_id IS NULL{} ORDER BY sort_order, name",
            owner_condition
        );
        sqlx::query_as::<_, Space>(&query)
            .fetch_all(&state.db)
            .await
            .map_err(AppError::Database)?
    };

    Ok(Json(spaces))
}

/// 创建空间
///
/// 若指定了父空间，则自动计算深度（parent.depth + 1）。
/// owner 设置为当前登录用户。
pub async fn create_space(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<SpaceCreateRequest>,
) -> Result<(axum::http::StatusCode, Json<Space>), AppError> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    let (depth, parent_id) = if let Some(ref pid) = req.parent_id {
        let parent: Space = sqlx::query_as("SELECT * FROM spaces WHERE id = $1")
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
        r#"INSERT INTO spaces (id, name, icon, count, parent_id, depth, sort_order, photo_uri, owner_id, created_at, updated_at)
           VALUES ($1, $2, $3, 0, $4, $5, 0, $6, $7, $8, $9)
           RETURNING *"#,
    )
    .bind(&id)
    .bind(&req.name)
    .bind(&req.icon)
    .bind(&parent_id)
    .bind(depth)
    .bind(&req.photo_uri)
    .bind(&auth.user_id)
    .bind(&now)
    .bind(&now)
    .fetch_one(&state.db)
    .await
    .map_err(AppError::Database)?;

    Ok((axum::http::StatusCode::CREATED, Json(space)))
}

/// 获取单个空间详情
pub async fn get_space(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(space_id): Path<String>,
) -> Result<Json<Space>, AppError> {
    let space = sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = $1")
        .bind(&space_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    check_access(&state.db, &auth.user_id, "space", &space_id, &space.owner_id).await?;

    Ok(Json(space))
}

/// 更新空间
pub async fn update_space(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(space_id): Path<String>,
    Json(req): Json<SpaceUpdateRequest>,
) -> Result<Json<Space>, AppError> {
    let existing = sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = $1")
        .bind(&space_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    check_access(&state.db, &auth.user_id, "space", &space_id, &existing.owner_id).await?;

    let name = req.name.unwrap_or(existing.name);
    let icon = req.icon.unwrap_or(existing.icon);
    let photo_uri = req.photo_uri.unwrap_or(existing.photo_uri);
    let sort_order = req.sort_order.unwrap_or(existing.sort_order);
    let now = chrono::Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    let space = sqlx::query_as::<_, Space>(
        "UPDATE spaces SET name=$1, icon=$2, photo_uri=$3, sort_order=$4, updated_at=$5 WHERE id=$6 RETURNING *",
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

/// 删除空间（仅 owner 可操作）
///
/// 递归删除所有子空间，并将关联物品的 `location_id` 置空。
/// 同时清理协管关系。
pub async fn delete_space(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(space_id): Path<String>,
) -> Result<axum::http::StatusCode, AppError> {
    let space = sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = $1")
        .bind(&space_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    if space.owner_id != auth.user_id {
        return Err(AppError::Forbidden);
    }

    delete_space_recursive(&state, &space_id).await?;

    sqlx::query("DELETE FROM collaborators WHERE entity_type = 'space' AND entity_id = $1")
        .bind(&space_id)
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// 递归删除空间树（栈式后序遍历，避免递归栈溢出）
async fn delete_space_recursive(state: &AppState, space_id: &str) -> Result<(), AppError> {
    let mut stack = vec![space_id.to_string()];

    while let Some(current_id) = stack.pop() {
        // 收集子空间入栈
        let children: Vec<Space> =
            sqlx::query_as("SELECT * FROM spaces WHERE parent_id = $1")
                .bind(&current_id)
                .fetch_all(&state.db)
                .await
                .map_err(AppError::Database)?;

        for child in children {
            stack.push(child.id);
        }

        // 释放关联物品
        sqlx::query("UPDATE items SET location_id=NULL, location='' WHERE location_id = $1")
            .bind(&current_id)
            .execute(&state.db)
            .await
            .map_err(AppError::Database)?;

        // 删除空间自身
        sqlx::query("DELETE FROM spaces WHERE id = $1")
            .bind(&current_id)
            .execute(&state.db)
            .await
            .map_err(AppError::Database)?;
    }

    Ok(())
}

/// 获取空间树
///
/// 返回当前用户授权的所有空间构成的树形结构（递归嵌套）。
/// 每个节点附带了直接关联的物品 ID 列表。
pub async fn get_space_tree(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<SpaceNode>>, AppError> {
    let query = format!(
        "SELECT * FROM spaces WHERE (owner_id = '{}' OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'space' AND user_id = '{}')) ORDER BY sort_order, name",
        auth.user_id, auth.user_id
    );
    let all_spaces: Vec<Space> = sqlx::query_as(&query)
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Database)?;

    // 批量查询所有空间的物品关联
    let owner_ids: Vec<String> = all_spaces.iter().map(|s| s.id.clone()).collect();
    let item_locations: Vec<(String, Option<String>)> = if owner_ids.is_empty() {
        vec![]
    } else {
        let placeholders: Vec<String> = (1..=owner_ids.len()).map(|i| format!("${}", i)).collect();
        let mut query_builder = sqlx::QueryBuilder::new(
            format!("SELECT id, location_id FROM items WHERE location_id IN ({})", placeholders.join(","))
        );
        for oid in &owner_ids {
            query_builder.push_bind(oid.clone());
        }
        query_builder
            .build_query_as()
            .fetch_all(&state.db)
            .await
            .map_err(AppError::Database)?
    };

    let mut item_id_map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for (item_id, location_id) in item_locations {
        if let Some(loc_id) = location_id {
            item_id_map.entry(loc_id).or_default().push(item_id);
        }
    }

    let root_nodes = build_space_tree(&all_spaces, None, &item_id_map);
    Ok(Json(root_nodes))
}

/// 递归构建空间树
///
/// `parent_id == None` 匹配顶级节点，`Some(pid)` 匹配指定父节点的子节点。
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
                owner_id: space.owner_id.clone(),
            };
            nodes.push(node);
        }
    }

    nodes
}

/// 获取空间的直接子空间列表
pub async fn get_space_children(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(space_id): Path<String>,
) -> Result<Json<Vec<Space>>, AppError> {
    let space = sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = $1")
        .bind(&space_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    check_access(&state.db, &auth.user_id, "space", &space_id, &space.owner_id).await?;

    let children =
        sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE parent_id = $1 ORDER BY sort_order, name")
            .bind(&space_id)
            .fetch_all(&state.db)
            .await
            .map_err(AppError::Database)?;

    Ok(Json(children))
}

/// 获取空间下的物品列表
pub async fn get_space_items(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(space_id): Path<String>,
) -> Result<Json<Vec<Item>>, AppError> {
    let space = sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = $1")
        .bind(&space_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    check_access(&state.db, &auth.user_id, "space", &space_id, &space.owner_id).await?;

    let items = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE location_id = $1")
        .bind(&space_id)
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Database)?;

    Ok(Json(items))
}

/// 获取空间路径（从根到当前空间的完整路径）
pub async fn get_space_path(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(space_id): Path<String>,
) -> Result<Json<SpacePathResponse>, AppError> {
    let space = sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = $1")
        .bind(&space_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    check_access(&state.db, &auth.user_id, "space", &space_id, &space.owner_id).await?;

    let segments = get_space_path_segments(&state.db, &space_id).await?;

    let path = segments
        .iter()
        .map(|s| format!("{} {}", s.icon, s.name))
        .collect::<Vec<_>>()
        .join(" > ");

    Ok(Json(SpacePathResponse { path, segments }))
}

/// 查询空间路径段（从根向上追溯，不含权限校验）
pub async fn get_space_path_segments(
    pool: &PgPool,
    space_id: &str,
) -> Result<Vec<SpacePathSegment>, AppError> {
    let mut segments = Vec::new();
    let mut current_id = Some(space_id.to_string());

    while let Some(id) = current_id {
        let space: Space = sqlx::query_as("SELECT * FROM spaces WHERE id = $1")
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
