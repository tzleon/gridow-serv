//! API 路由定义
//!
//! 所有路由统一挂载在 `/v1` 前缀下，按业务模块组织嵌套路由。
//! 认证规则见各 Handler 文件的参数签名（`AuthUser` = 需登录）。
//!
//! # 路由结构
//! ```text
//! /v1/users/*     — 用户注册/登录/信息/升级/忘记密码
//! /v1/items/*     — 物品 CRUD / 出库 / 转移 / 协管
//! /v1/spaces/*    — 空间 CRUD / 树 / 子节点 / 路径 / 协管
//! /v1/history/*   — 操作历史查询
//! /v1/categories/* — 分类 CRUD
//! /v1/tags/*      — 标签 CRUD
//! /v1/images/*    — 图片上传与访问
//! /v1/sync/*      — 离线增量同步
//! ```

use axum::routing::{get, post, put};
use axum::Router;

use crate::handlers;
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    let api_v1 = Router::new()
        // ── 用户模块 ─────────────────────────────────────────
        .nest(
            "/users",
            Router::new()
                .route("/register", post(handlers::user::register_user))
                .route("/login", post(handlers::user::login_user))
                .route("/logout", post(handlers::user::logout_user))
                .route("/{user_id}", get(handlers::user::get_user_info).put(handlers::user::update_user))
                .route("/{user_id}/upgrade", post(handlers::user::upgrade_vip))
                .route("/{user_id}/password", put(handlers::user::change_password))
                .route("/forgot-password", post(handlers::user::send_reset_code))
                .route("/verify-code", post(handlers::user::verify_reset_code))
                .route("/reset-password", post(handlers::user::reset_password)),
        )
        // ── 物品模块 ─────────────────────────────────────────
        .nest(
            "/items",
            Router::new()
                .route("/", get(handlers::item::list_items).post(handlers::item::create_item))
                .route("/expiring", get(handlers::item::get_expiring_items))
                .route("/low-stock", get(handlers::item::get_low_stock_items))
                .route(
                    "/barcode/{barcode}",
                    get(handlers::item::get_item_by_barcode),
                )
                .route(
                    "/{item_id}",
                    get(handlers::item::get_item)
                        .put(handlers::item::update_item)
                        .delete(handlers::item::delete_item),
                )
                .route(
                    "/{item_id}/outbound",
                    post(handlers::item::outbound_item),
                )
                .route(
                    "/{item_id}/transfer",
                    post(handlers::item::transfer_item),
                )
                .route(
                    "/{item_id}/collaborators",
                    get(handlers::collaborator::list_item_collaborators)
                        .post(handlers::collaborator::add_item_collaborator),
                )
                .route(
                    "/{item_id}/collaborators/{user_id}",
                    axum::routing::delete(handlers::collaborator::remove_item_collaborator),
                ),
        )
        // ── 空间模块 ─────────────────────────────────────────
        .nest(
            "/spaces",
            Router::new()
                .route("/", get(handlers::space::list_spaces).post(handlers::space::create_space))
                .route("/tree", get(handlers::space::get_space_tree))
                .route(
                    "/{space_id}",
                    get(handlers::space::get_space)
                        .put(handlers::space::update_space)
                        .delete(handlers::space::delete_space),
                )
                .route(
                    "/{space_id}/children",
                    get(handlers::space::get_space_children),
                )
                .route(
                    "/{space_id}/items",
                    get(handlers::space::get_space_items),
                )
                .route(
                    "/{space_id}/path",
                    get(handlers::space::get_space_path),
                )
                .route(
                    "/{space_id}/collaborators",
                    get(handlers::collaborator::list_space_collaborators)
                        .post(handlers::collaborator::add_space_collaborator),
                )
                .route(
                    "/{space_id}/collaborators/{user_id}",
                    axum::routing::delete(handlers::collaborator::remove_space_collaborator),
                ),
        )
        // ── 操作历史模块 ─────────────────────────────────────
        .nest(
            "/history",
            Router::new()
                .route("/", get(handlers::history::list_history))
                .route("/item/{item_id}", get(handlers::history::get_item_history)),
        )
        // ── 分类模块 ─────────────────────────────────────────
        .nest(
            "/categories",
            Router::new()
                .route("/", get(handlers::category::list_categories).post(handlers::category::create_category))
                .route(
                    "/{category_id}",
                    put(handlers::category::update_category).delete(handlers::category::delete_category),
                ),
        )
        // ── 标签模块 ─────────────────────────────────────────
        .nest(
            "/tags",
            Router::new()
                .route("/", get(handlers::tag::list_tags).post(handlers::tag::create_tag))
                .route(
                    "/{tag_id}",
                    put(handlers::tag::update_tag).delete(handlers::tag::delete_tag),
                ),
        )
        // ── 图片模块 ─────────────────────────────────────────
        .nest(
            "/images",
            Router::new()
                .route("/upload", post(handlers::image::upload_image))
                .route("/{filename}", get(handlers::image::get_image)),
        )
        // ── 数据同步模块 ─────────────────────────────────────
        .nest(
            "/sync",
            Router::new()
                .route("/pull", get(handlers::sync::sync_pull))
                .route("/push", post(handlers::sync::sync_push))
                .route("/status", get(handlers::sync::sync_status)),
        );

    Router::new()
        .nest("/v1", api_v1)
        .with_state(state)
}
