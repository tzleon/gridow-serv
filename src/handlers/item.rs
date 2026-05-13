//! 物品管理处理器
//!
//! 提供物品的完整生命周期管理：
//! * 增删改查（含分页、排序、分类/关键字筛选）
//! * 出库（减少库存，记录操作历史）
//! * 转移（变更所属空间，同步更新空间计数）
//! * 过期预警 / 低库存预警
//! * 条码查询
//!
//! # 权限模型
//! * **查询** — 仅返回当前用户拥有或协管的物品
//! * **修改/出库/转移** — owner 或协管可操作
//! * **删除** — 仅 owner 可操作

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use sqlx::PgPool;

use crate::auth::AuthUser;
use crate::models::error::AppError;
use crate::models::item::*;
use crate::state::AppState;

/// 权限校验：检查用户是否为实体的 owner 或协管
///
/// 若均不是则返回 `AppError::Forbidden`。
async fn check_access(pool: &PgPool, user_id: &str, entity_type: &str, entity_id: &str, owner_id: &str) -> Result<(), AppError> {
    // owner 直接通过
    if owner_id == user_id {
        return Ok(());
    }
    // 查询协管表
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

/// 物品列表查询
///
/// 支持条件组合：分类、关键字（模糊匹配 name/tags/location）、空间 ID。
/// 分页参数：`page`（默认 1）、`pageSize`（默认 20）。
/// 排序：`sortBy` 可选 `createdAt`/`name`/`qty`/`expiry`/`updatedAt`，`sortOrder` 可选 `asc`/`desc`。
pub async fn list_items(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<ItemQueryParams>,
) -> Result<Json<ItemListResponse>, AppError> {
    // 前端排序参数映射到数据库列名
    let sort_column = match params.sort_by.as_str() {
        "createdAt" => "created_at",
        "name" => "name",
        "qty" => "qty",
        "expiry" => "expiry",
        _ => "updated_at",
    };
    let sort_dir = match params.sort_order.as_str() {
        "asc" => "ASC",
        _ => "DESC",
    };
    let offset = (params.page - 1) * params.page_size;

    // 构建 owner + 协管过滤条件
    let owner_condition = format!(
        " AND (owner_id = '{}' OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'item' AND user_id = '{}'))",
        auth.user_id, auth.user_id
    );

    // 使用 QueryBuilder 动态拼接 SQL
    let mut count_builder = sqlx::QueryBuilder::new(
        format!("SELECT COUNT(*) FROM items WHERE 1=1{}", owner_condition)
    );
    let mut data_builder = sqlx::QueryBuilder::new(
        format!("SELECT * FROM items WHERE 1=1{}", owner_condition)
    );

    // 分类筛选
    if let Some(ref category) = params.category {
        count_builder.push(" AND category = ");
        count_builder.push_bind(category.clone());
        data_builder.push(" AND category = ");
        data_builder.push_bind(category.clone());
    }

    // 关键字模糊搜索
    if let Some(ref keyword) = params.keyword {
        let kw = format!("%{}%", keyword);
        count_builder.push(" AND (name LIKE ");
        count_builder.push_bind(kw.clone());
        count_builder.push(" OR tags LIKE ");
        count_builder.push_bind(kw.clone());
        count_builder.push(" OR location LIKE ");
        count_builder.push_bind(kw.clone());
        count_builder.push(")");

        data_builder.push(" AND (name LIKE ");
        data_builder.push_bind(kw.clone());
        data_builder.push(" OR tags LIKE ");
        data_builder.push_bind(kw.clone());
        data_builder.push(" OR location LIKE ");
        data_builder.push_bind(kw);
        data_builder.push(")");
    }

    // 空间筛选
    if let Some(ref space_id) = params.space_id {
        count_builder.push(" AND location_id = ");
        count_builder.push_bind(space_id.clone());
        data_builder.push(" AND location_id = ");
        data_builder.push_bind(space_id.clone());
    }

    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await
        .map_err(AppError::Database)?;

    data_builder.push(format!(" ORDER BY {} {}", sort_column, sort_dir));
    data_builder.push(" LIMIT ");
    data_builder.push_bind(params.page_size);
    data_builder.push(" OFFSET ");
    data_builder.push_bind(offset);

    let items = data_builder
        .build_query_as::<Item>()
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Database)?;

    Ok(Json(ItemListResponse {
        items,
        total: total as i32,
        page: params.page,
        page_size: params.page_size,
    }))
}

/// 创建物品
///
/// 自动生成 UUID，设置 owner 为当前用户，
/// 若指定了 `location_id` 则计算空间路径冗余字段并更新空间计数。
/// 同时写入一条入库历史记录（type="in"）。
pub async fn create_item(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<ItemCreateRequest>,
) -> Result<(axum::http::StatusCode, Json<Item>), AppError> {
    let id = new_item_id();
    let now = now_string();
    let tags_json = serde_json::to_string(&req.tags).unwrap_or_else(|_| "[]".to_string());

    // 计算空间路径字符串（如 "🏠 客厅 > 🗄️ 电视柜"）
    let location = if let Some(ref loc_id) = req.location_id {
        get_space_path_string(&state, loc_id).await?
    } else {
        String::new()
    };

    sqlx::query(
        r#"INSERT INTO items (id, name, icon, qty, location, location_id, category, tags, barcode, photos, photo_uri, buy_date, expiry, remark, track_low_stock, owner_id, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)"#,
    )
    .bind(&id)
    .bind(&req.name)
    .bind(&req.icon)
    .bind(req.qty)
    .bind(&location)
    .bind(&req.location_id)
    .bind(&req.category)
    .bind(&tags_json)
    .bind(&req.barcode)
    .bind("[]")
    .bind(&req.photo_uri)
    .bind(&req.buy_date)
    .bind(&req.expiry)
    .bind(&req.remark)
    .bind(req.track_low_stock)
    .bind(&auth.user_id)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(AppError::Database)?;

    let item = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE id = $1")
        .bind(&id)
        .fetch_one(&state.db)
        .await
        .map_err(AppError::Database)?;

    // 更新空间计数
    if let Some(ref loc_id) = req.location_id {
        update_space_count(&state, loc_id).await?;
    }

    // 写入入库历史
    create_history_record(
        &state,
        "in",
        &id,
        &req.name,
        req.qty,
        None,
        Some(&location),
        None,
        None,
    )
    .await?;

    Ok((axum::http::StatusCode::CREATED, Json(item)))
}

/// 获取单个物品详情
///
/// 需要 owner 或协管权限。
pub async fn get_item(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(item_id): Path<String>,
) -> Result<Json<Item>, AppError> {
    let item = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE id = $1")
        .bind(&item_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    check_access(&state.db, &auth.user_id, "item", &item_id, &item.owner_id).await?;

    Ok(Json(item))
}

/// 更新物品
///
/// 所有字段可选，不传则保留原值。若空间变更，同步更新新旧两个空间的计数。
pub async fn update_item(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(item_id): Path<String>,
    Json(req): Json<ItemUpdateRequest>,
) -> Result<Json<Item>, AppError> {
    let existing = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE id = $1")
        .bind(&item_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    check_access(&state.db, &auth.user_id, "item", &item_id, &existing.owner_id).await?;

    // 合并新旧值
    let name = req.name.unwrap_or(existing.name);
    let icon = req.icon.unwrap_or(existing.icon);
    let qty = req.qty.unwrap_or(existing.qty);
    let category = req.category.unwrap_or(existing.category);
    let barcode = req.barcode.unwrap_or(existing.barcode);
    let photo_uri = req.photo_uri.unwrap_or(existing.photo_uri);
    let buy_date = req.buy_date.unwrap_or(existing.buy_date);
    let expiry = req.expiry.unwrap_or(existing.expiry);
    let remark = req.remark.unwrap_or(existing.remark);
    let track_low_stock = req.track_low_stock.unwrap_or(existing.track_low_stock);
    let tags = match req.tags {
        Some(t) => serde_json::to_string(&t).unwrap_or_else(|_| "[]".to_string()),
        None => existing.tags,
    };

    let old_location_id = existing.location_id.clone();

    let location_id = match req.location_id {
        Some(loc_id) => loc_id,
        None => existing.location_id,
    };

    let location = if let Some(ref loc_id) = location_id {
        get_space_path_string(&state, loc_id).await?
    } else {
        String::new()
    };

    let now = now_string();

    sqlx::query(
        r#"UPDATE items SET name=$1, icon=$2, qty=$3, location=$4, location_id=$5, category=$6, tags=$7, barcode=$8, photo_uri=$9, buy_date=$10, expiry=$11, remark=$12, track_low_stock=$13, updated_at=$14
           WHERE id=$15"#,
    )
    .bind(&name)
    .bind(&icon)
    .bind(qty)
    .bind(&location)
    .bind(&location_id)
    .bind(&category)
    .bind(&tags)
    .bind(&barcode)
    .bind(&photo_uri)
    .bind(&buy_date)
    .bind(&expiry)
    .bind(&remark)
    .bind(track_low_stock)
    .bind(&now)
    .bind(&item_id)
    .execute(&state.db)
    .await
    .map_err(AppError::Database)?;

    let item = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE id = $1")
        .bind(&item_id)
        .fetch_one(&state.db)
        .await
        .map_err(AppError::Database)?;

    // 空间变更时同步更新计数
    if old_location_id != location_id {
        if let Some(ref old_loc) = old_location_id {
            update_space_count(&state, old_loc).await?;
        }
        if let Some(ref new_loc) = location_id {
            update_space_count(&state, new_loc).await?;
        }
    }

    Ok(Json(item))
}

/// 删除物品（仅 owner 可操作）
///
/// 级联清理关联的 history 和 collaborators 记录。
pub async fn delete_item(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(item_id): Path<String>,
) -> Result<axum::http::StatusCode, AppError> {
    let item = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE id = $1")
        .bind(&item_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    // 仅 owner 可删除
    if item.owner_id != auth.user_id {
        return Err(AppError::Forbidden);
    }

    // 级联清理
    sqlx::query("DELETE FROM items WHERE id = $1")
        .bind(&item_id)
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?;

    sqlx::query("DELETE FROM history WHERE item_id = $1")
        .bind(&item_id)
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?;

    sqlx::query("DELETE FROM collaborators WHERE entity_type = 'item' AND entity_id = $1")
        .bind(&item_id)
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?;

    if let Some(ref loc_id) = item.location_id {
        update_space_count(&state, loc_id).await?;
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// 物品出库
///
/// 减少库存数量，写入出库历史记录。需 owner 或协管权限。
pub async fn outbound_item(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(item_id): Path<String>,
    Json(req): Json<OutboundRequest>,
) -> Result<Json<Item>, AppError> {
    if req.qty < 1 {
        return Err(AppError::BadRequest("出库数量必须大于0".to_string()));
    }

    let existing = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE id = $1")
        .bind(&item_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    check_access(&state.db, &auth.user_id, "item", &item_id, &existing.owner_id).await?;

    // 库存不足
    if existing.qty < req.qty {
        return Err(AppError::BadRequest(format!(
            "库存不足，当前库存: {}",
            existing.qty
        )));
    }

    let new_qty = existing.qty - req.qty;
    let now = now_string();

    sqlx::query("UPDATE items SET qty=$1, updated_at=$2 WHERE id=$3")
        .bind(new_qty)
        .bind(&now)
        .bind(&item_id)
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?;

    let item = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE id = $1")
        .bind(&item_id)
        .fetch_one(&state.db)
        .await
        .map_err(AppError::Database)?;

    create_history_record(
        &state,
        "out",
        &item_id,
        &existing.name,
        req.qty,
        Some(&existing.location),
        None,
        Some(&req.reason),
        None,
    )
    .await?;

    if let Some(ref loc_id) = existing.location_id {
        update_space_count(&state, loc_id).await?;
    }

    Ok(Json(item))
}

/// 物品转移
///
/// 更改物品所属空间，写入转移历史记录，同步更新新旧空间计数。
pub async fn transfer_item(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(item_id): Path<String>,
    Json(req): Json<TransferRequest>,
) -> Result<Json<Item>, AppError> {
    if req.qty < 1 {
        return Err(AppError::BadRequest("转移数量必须大于0".to_string()));
    }

    let existing = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE id = $1")
        .bind(&item_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    check_access(&state.db, &auth.user_id, "item", &item_id, &existing.owner_id).await?;

    // 验证目标空间存在
    let _target_space = sqlx::query_as::<_, crate::models::space::Space>(
        "SELECT * FROM spaces WHERE id = $1",
    )
    .bind(&req.target_space_id)
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Database)?
    .ok_or(AppError::BadRequest("目标空间不存在".to_string()))?;

    let from_location = existing.location.clone();
    let old_location_id = existing.location_id.clone();

    let to_location = get_space_path_string(&state, &req.target_space_id).await?;
    let now = now_string();

    sqlx::query("UPDATE items SET location=$1, location_id=$2, updated_at=$3 WHERE id=$4")
        .bind(&to_location)
        .bind(&req.target_space_id)
        .bind(&now)
        .bind(&item_id)
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?;

    let item = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE id = $1")
        .bind(&item_id)
        .fetch_one(&state.db)
        .await
        .map_err(AppError::Database)?;

    create_history_record(
        &state,
        "move",
        &item_id,
        &existing.name,
        req.qty,
        Some(&from_location),
        Some(&to_location),
        None,
        None,
    )
    .await?;

    // 更新新旧空间计数
    if let Some(ref old_loc) = old_location_id {
        update_space_count(&state, old_loc).await?;
    }
    update_space_count(&state, &req.target_space_id).await?;

    Ok(Json(item))
}

/// 按条码查询物品
///
/// 用于扫码枪 / 二维码扫描场景。
pub async fn get_item_by_barcode(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(barcode): Path<String>,
) -> Result<Json<Item>, AppError> {
    let item = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE barcode = $1")
        .bind(&barcode)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    check_access(&state.db, &auth.user_id, "item", &item.id, &item.owner_id).await?;

    Ok(Json(item))
}

/// 获取即将过期的物品列表
///
/// `days` 参数表示未来多少天内过期（默认 30）。
pub async fn get_expiring_items(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<ExpiringQueryParams>,
) -> Result<Json<Vec<Item>>, AppError> {
    let days = params.days.unwrap_or(30);
    let target_date = chrono::Local::now()
        .checked_add_signed(chrono::Duration::days(days as i64))
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "2099-12-31".to_string());

    let items = sqlx::query_as::<_, Item>(
        r#"SELECT * FROM items
           WHERE (owner_id = $1 OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'item' AND user_id = $1))
           AND expiry != '-' AND expiry != '' AND expiry <= $2
           ORDER BY expiry ASC"#,
    )
    .bind(&auth.user_id)
    .bind(&target_date)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Database)?;

    Ok(Json(items))
}

/// 获取低库存物品列表
///
/// 仅返回 `track_low_stock = TRUE` 且数量 ≤ threshold 的物品。
pub async fn get_low_stock_items(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<LowStockQueryParams>,
) -> Result<Json<Vec<Item>>, AppError> {
    let threshold = params.threshold.unwrap_or(1);

    let items = sqlx::query_as::<_, Item>(
        r#"SELECT * FROM items
           WHERE (owner_id = $1 OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'item' AND user_id = $1))
           AND track_low_stock = TRUE AND qty <= $2
           ORDER BY qty ASC"#,
    )
    .bind(&auth.user_id)
    .bind(threshold)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Database)?;

    Ok(Json(items))
}

// ── 辅助类型 ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ExpiringQueryParams {
    days: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct LowStockQueryParams {
    threshold: Option<i32>,
}

// ── 内部辅助函数 ────────────────────────────────────────────

/// 根据空间 ID 获取人类可读的路径字符串
async fn get_space_path_string(state: &AppState, space_id: &str) -> Result<String, AppError> {
    let segments = crate::handlers::space::get_space_path_segments(&state.db, space_id).await?;
    Ok(segments
        .iter()
        .map(|s| format!("{} {}", s.icon, s.name))
        .collect::<Vec<_>>()
        .join(" > "))
}

/// 重新计算空间的物品数量并更新
async fn update_space_count(state: &AppState, space_id: &str) -> Result<(), AppError> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM items WHERE location_id = $1")
        .bind(space_id)
        .fetch_one(&state.db)
        .await
        .map_err(AppError::Database)?;

    sqlx::query("UPDATE spaces SET count=$1 WHERE id=$2")
        .bind(count.0 as i32)
        .bind(space_id)
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?;

    Ok(())
}

/// 写入操作历史记录
async fn create_history_record(
    state: &AppState,
    history_type: &str,
    item_id: &str,
    item_name: &str,
    qty: i32,
    from_location: Option<&str>,
    to_location: Option<&str>,
    reason: Option<&str>,
    remark: Option<&str>,
) -> Result<(), AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let time = now_string();

    sqlx::query(
        r#"INSERT INTO history (id, type, item_id, item_name, qty, from_location, to_location, reason, remark, time)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
    )
    .bind(&id)
    .bind(history_type)
    .bind(item_id)
    .bind(item_name)
    .bind(qty)
    .bind(from_location)
    .bind(to_location)
    .bind(reason)
    .bind(remark)
    .bind(&time)
    .execute(&state.db)
    .await
    .map_err(AppError::Database)?;

    Ok(())
}
