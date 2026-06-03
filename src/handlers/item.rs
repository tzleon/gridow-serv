use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;

use crate::auth::AuthUser;
use crate::models::error::AppError;
use crate::models::item::*;
use crate::state::AppState;

pub async fn list_items(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<ItemQueryParams>,
) -> Result<Json<ItemListResponse>, AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;

    let sort_column = match params.sort_by.as_str() {
        "createdAt" => "created_at", "name" => "name", "qty" => "qty", "expiry" => "expiry", _ => "updated_at",
    };
    let sort_dir = if params.sort_order.eq_ignore_ascii_case("asc") { "ASC" } else { "DESC" };
    let offset = (params.page - 1) * params.page_size;

    let mut count_builder = sqlx::QueryBuilder::new(
        "SELECT COUNT(*) FROM items WHERE is_deleted = 0 AND (owner_id = "
    );
    count_builder.push_bind(user_internal);
    count_builder.push(" OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'item' AND user_id = ");
    count_builder.push_bind(user_internal);
    count_builder.push("))");

    let mut data_builder = sqlx::QueryBuilder::new(
        "SELECT * FROM items WHERE is_deleted = 0 AND (owner_id = "
    );
    data_builder.push_bind(user_internal);
    data_builder.push(" OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'item' AND user_id = ");
    data_builder.push_bind(user_internal);
    data_builder.push("))");

    if let Some(ref cat) = params.category {
        count_builder.push(" AND category = "); count_builder.push_bind(cat.clone());
        data_builder.push(" AND category = "); data_builder.push_bind(cat.clone());
    }

    if let Some(ref kw) = params.keyword {
        let k = format!("%{}%", kw);
        count_builder.push(" AND (name LIKE "); count_builder.push_bind(k.clone());
        count_builder.push(" OR tags LIKE "); count_builder.push_bind(k.clone());
        count_builder.push(" OR location LIKE "); count_builder.push_bind(k.clone()); count_builder.push(")");

        data_builder.push(" AND (name LIKE "); data_builder.push_bind(k.clone());
        data_builder.push(" OR tags LIKE "); data_builder.push_bind(k.clone());
        data_builder.push(" OR location LIKE "); data_builder.push_bind(k); data_builder.push(")");
    }

    if let Some(ref sid) = params.space_id {
        let space_internal = state.resolve_space_id(sid).await?;
        count_builder.push(" AND location_id = "); count_builder.push_bind(space_internal);
        data_builder.push(" AND location_id = "); data_builder.push_bind(space_internal);
    }

    let total: i64 = count_builder.build_query_scalar().fetch_one(&state.db).await.map_err(AppError::Database)?;

    data_builder.push(format!(" ORDER BY {} {}", sort_column, sort_dir));
    data_builder.push(" LIMIT "); data_builder.push_bind(params.page_size);
    data_builder.push(" OFFSET "); data_builder.push_bind(offset);

    let items = data_builder.build_query_as::<Item>().fetch_all(&state.db).await.map_err(AppError::Database)?;

    Ok(Json(ItemListResponse { items, total: total as i32, page: params.page, page_size: params.page_size }))
}

pub async fn create_item(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<ItemCreateRequest>,
) -> Result<(axum::http::StatusCode, Json<Item>), AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let (id, public_id) = state.new_id();
    let now = AppState::now_string();
    let version = state.next_version().await.map_err(AppError::Database)?;
    let tags_json = serde_json::to_string(&req.tags).unwrap_or_else(|_| "[]".to_string());

    let location_internal = if let Some(ref loc_pid) = req.location_id {
        Some(state.resolve_space_id(loc_pid).await?)
    } else { None };

    let location = if let Some(ref li) = location_internal {
        get_space_path_string(&state, *li).await?
    } else { String::new() };

    let item = sqlx::query_as::<_, Item>(
        r#"INSERT INTO items (id, public_id, name, icon, qty, location, location_id, category, tags, barcode, photos, photo_uri, buy_date, expiry, remark, track_low_stock, owner_id, created_at, updated_at, version, is_deleted)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21)
           RETURNING *"#,
    )
    .bind(id).bind(&public_id).bind(&req.name).bind(&req.icon).bind(req.qty)
    .bind(&location).bind(location_internal).bind(&req.category).bind(&tags_json)
    .bind(&req.barcode).bind("[]").bind(&req.photo_uri).bind(&req.buy_date)
    .bind(&req.expiry).bind(&req.remark).bind(req.track_low_stock)
    .bind(user_internal).bind(&now).bind(&now)
    .bind(version).bind(0i16)
    .fetch_one(&state.db).await.map_err(AppError::Database)?;

    if let Some(li) = location_internal {
        update_space_count(&state, li).await?;
    }

    create_history_record(&state, "in", id, &req.name, req.qty, None, Some(&location), None, None).await?;

    Ok((axum::http::StatusCode::CREATED, Json(item)))
}

pub async fn get_item(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(item_public_id): Path<String>,
) -> Result<Json<Item>, AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let item = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE public_id = $1")
        .bind(&item_public_id).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;

    state.check_access(user_internal, "item", item.id, item.owner_id).await?;
    Ok(Json(item))
}

pub async fn update_item(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(item_public_id): Path<String>,
    Json(req): Json<ItemUpdateRequest>,
) -> Result<Json<Item>, AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let existing = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE public_id = $1")
        .bind(&item_public_id).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;

    state.check_access(user_internal, "item", existing.id, existing.owner_id).await?;

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

    let old_location_id = existing.location_id;
    let location_id = match req.location_id {
        Some(lid) => match lid {
            Some(ref pid) => Some(state.resolve_space_id(pid).await?),
            None => None,
        },
        None => existing.location_id,
    };

    let location = if let Some(ref loc_id) = location_id {
        get_space_path_string(&state, *loc_id).await?
    } else { String::new() };

    let now = AppState::now_string();
    let version = state.next_version().await.map_err(AppError::Database)?;

    let item = sqlx::query_as::<_, Item>(
        r#"UPDATE items SET name=$1, icon=$2, qty=$3, location=$4, location_id=$5, category=$6, tags=$7, barcode=$8, photo_uri=$9, buy_date=$10, expiry=$11, remark=$12, track_low_stock=$13, updated_at=$14, version=$15 WHERE id=$16 RETURNING *"#,
    )
    .bind(&name).bind(&icon).bind(qty).bind(&location).bind(location_id)
    .bind(&category).bind(&tags).bind(&barcode).bind(&photo_uri)
    .bind(&buy_date).bind(&expiry).bind(&remark).bind(track_low_stock)
    .bind(&now).bind(version).bind(existing.id)
    .fetch_one(&state.db).await.map_err(AppError::Database)?;

    if old_location_id != location_id {
        if let Some(old_loc) = old_location_id { update_space_count(&state, old_loc).await?; }
        if let Some(new_loc) = location_id { update_space_count(&state, new_loc).await?; }
    }

    Ok(Json(item))
}

pub async fn delete_item(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(item_public_id): Path<String>,
) -> Result<axum::http::StatusCode, AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let item = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE public_id = $1")
        .bind(&item_public_id).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;

    if item.owner_id != user_internal { return Err(AppError::Forbidden); }

    let now = AppState::now_string();
    let version = state.next_version().await.map_err(AppError::Database)?;

    sqlx::query("UPDATE items SET is_deleted=1, version=$1, updated_at=$2 WHERE id=$3")
        .bind(version).bind(&now).bind(item.id).execute(&state.db).await.map_err(AppError::Database)?;
    sqlx::query("DELETE FROM history WHERE item_id = $1").bind(item.id).execute(&state.db).await.map_err(AppError::Database)?;
    sqlx::query("DELETE FROM collaborators WHERE entity_type = 'item' AND entity_id = $1").bind(item.id).execute(&state.db).await.map_err(AppError::Database)?;

    if let Some(loc_id) = item.location_id { update_space_count(&state, loc_id).await?; }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn outbound_item(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(item_public_id): Path<String>,
    Json(req): Json<OutboundRequest>,
) -> Result<Json<Item>, AppError> {
    if req.qty < 1 { return Err(AppError::BadRequest("出库数量必须大于0".to_string())); }

    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let existing = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE public_id = $1")
        .bind(&item_public_id).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;

    state.check_access(user_internal, "item", existing.id, existing.owner_id).await?;

    if existing.qty < req.qty {
        return Err(AppError::BadRequest(format!("库存不足，当前库存: {}", existing.qty)));
    }

    let new_qty = existing.qty - req.qty;
    let now = AppState::now_string();
    let version = state.next_version().await.map_err(AppError::Database)?;

    let item = sqlx::query_as::<_, Item>("UPDATE items SET qty=$1, updated_at=$2, version=$3 WHERE id=$4 RETURNING *")
        .bind(new_qty).bind(&now).bind(version).bind(existing.id)
        .fetch_one(&state.db).await.map_err(AppError::Database)?;

    create_history_record(&state, "out", existing.id, &existing.name, req.qty, Some(&existing.location), None, Some(&req.reason), None).await?;

    if let Some(loc_id) = existing.location_id { update_space_count(&state, loc_id).await?; }

    Ok(Json(item))
}

pub async fn transfer_item(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(item_public_id): Path<String>,
    Json(req): Json<TransferRequest>,
) -> Result<Json<Item>, AppError> {
    if req.qty < 1 { return Err(AppError::BadRequest("转移数量必须大于0".to_string())); }

    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let existing = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE public_id = $1")
        .bind(&item_public_id).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;

    state.check_access(user_internal, "item", existing.id, existing.owner_id).await?;

    let target_internal = state.resolve_space_id(&req.target_space_id).await?;

    let _target = sqlx::query_as::<_, crate::models::space::Space>("SELECT * FROM spaces WHERE id = $1")
        .bind(target_internal).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::BadRequest("目标空间不存在".to_string()))?;

    let from_location = existing.location.clone();
    let old_location_id = existing.location_id;
    let to_location = get_space_path_string(&state, target_internal).await?;
    let now = AppState::now_string();
    let version = state.next_version().await.map_err(AppError::Database)?;

    let item = sqlx::query_as::<_, Item>("UPDATE items SET location=$1, location_id=$2, updated_at=$3, version=$4 WHERE id=$5 RETURNING *")
        .bind(&to_location).bind(target_internal).bind(&now).bind(version).bind(existing.id)
        .fetch_one(&state.db).await.map_err(AppError::Database)?;

    create_history_record(&state, "move", existing.id, &existing.name, req.qty, Some(&from_location), Some(&to_location), None, None).await?;

    if let Some(old_loc) = old_location_id { update_space_count(&state, old_loc).await?; }
    update_space_count(&state, target_internal).await?;

    Ok(Json(item))
}

pub async fn get_item_by_barcode(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(barcode): Path<String>,
) -> Result<Json<Item>, AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let item = sqlx::query_as::<_, Item>("SELECT * FROM items WHERE barcode = $1")
        .bind(&barcode).fetch_optional(&state.db).await
        .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;

    state.check_access(user_internal, "item", item.id, item.owner_id).await?;
    Ok(Json(item))
}

pub async fn get_expiring_items(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<ExpiringQueryParams>,
) -> Result<Json<Vec<Item>>, AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let days = params.days.unwrap_or(30);
    let target_date = chrono::Local::now()
        .checked_add_signed(chrono::Duration::days(days as i64))
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "2099-12-31".to_string());

    let items = sqlx::query_as::<_, Item>(
        r#"SELECT * FROM items
           WHERE (owner_id = $1 OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'item' AND user_id = $1))
           AND expiry != '-' AND expiry != '' AND expiry <= $2 ORDER BY expiry ASC"#,
    ).bind(user_internal).bind(&target_date).fetch_all(&state.db).await.map_err(AppError::Database)?;

    Ok(Json(items))
}

pub async fn get_low_stock_items(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(params): Query<LowStockQueryParams>,
) -> Result<Json<Vec<Item>>, AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let threshold = params.threshold.unwrap_or(1);

    let items = sqlx::query_as::<_, Item>(
        r#"SELECT * FROM items
           WHERE (owner_id = $1 OR id IN (SELECT entity_id FROM collaborators WHERE entity_type = 'item' AND user_id = $1))
           AND track_low_stock = TRUE AND qty <= $2 ORDER BY qty ASC"#,
    ).bind(user_internal).bind(threshold).fetch_all(&state.db).await.map_err(AppError::Database)?;

    Ok(Json(items))
}

#[derive(Debug, Deserialize)]
pub struct ExpiringQueryParams { days: Option<i32> }

#[derive(Debug, Deserialize)]
pub struct LowStockQueryParams { threshold: Option<i32> }

async fn get_space_path_string(state: &AppState, space_internal: i64) -> Result<String, AppError> {
    let segments = crate::handlers::space::get_space_path_segments(&state.db, space_internal).await?;
    Ok(segments.iter().map(|s| format!("{} {}", s.icon, s.name)).collect::<Vec<_>>().join(" > "))
}

async fn update_space_count(state: &AppState, space_internal: i64) -> Result<(), AppError> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM items WHERE location_id = $1")
        .bind(space_internal).fetch_one(&state.db).await.map_err(AppError::Database)?;

    sqlx::query("UPDATE spaces SET count=$1 WHERE id=$2")
        .bind(count.0 as i32).bind(space_internal).execute(&state.db).await.map_err(AppError::Database)?;

    Ok(())
}

async fn create_history_record(
    state: &AppState, history_type: &str, item_internal: i64, item_name: &str,
    qty: i32, from_location: Option<&str>, to_location: Option<&str>,
    reason: Option<&str>, remark: Option<&str>,
) -> Result<(), AppError> {
    let (id, public_id) = state.new_id();
    let time = AppState::now_string();
    let version = state.next_version().await.map_err(AppError::Database)?;

    sqlx::query(
        r#"INSERT INTO history (id, public_id, type, item_id, item_name, qty, from_location, to_location, reason, remark, time, version, is_deleted)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)"#,
    )
    .bind(id).bind(&public_id).bind(history_type).bind(item_internal).bind(item_name)
    .bind(qty).bind(from_location).bind(to_location).bind(reason).bind(remark).bind(&time)
    .bind(version).bind(0i16)
    .execute(&state.db).await.map_err(AppError::Database)?;

    Ok(())
}
