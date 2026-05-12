use axum::routing::{get, post};
use axum::Router;

use crate::handlers;
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    let api_v1 = Router::new()
        .nest(
            "/users",
            Router::new()
                .route("/register", post(handlers::user::register_user))
                .route("/login", post(handlers::user::login_user))
                .route("/logout", post(handlers::user::logout_user))
                .route("/{user_id}", get(handlers::user::get_user_info).put(handlers::user::update_user))
                .route("/{user_id}/upgrade", post(handlers::user::upgrade_vip)),
        )
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
                ),
        )
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
                ),
        )
        .nest(
            "/history",
            Router::new()
                .route("/", get(handlers::history::list_history))
                .route("/item/{item_id}", get(handlers::history::get_item_history)),
        )
        .nest(
            "/images",
            Router::new()
                .route("/upload", post(handlers::image::upload_image))
                .route("/{filename}", get(handlers::image::get_image)),
        )
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
