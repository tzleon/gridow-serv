mod handlers;
mod logging;
mod models;
mod routes;
mod state;

use std::path::PathBuf;

use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() {
    let log_dir = std::env::var("LOG_DIR")
        .unwrap_or_else(|_| "./logs".to_string());
    
    logging::init_logging(&log_dir);

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:dfER%40123123@156.238.229.131:54328/gridow-web".to_string());

    let upload_dir = std::env::var("UPLOAD_DIR")
        .unwrap_or_else(|_| "./uploads".to_string());

    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "gridow_jwt_secret_key_2024".to_string());

    let upload_path = PathBuf::from(&upload_dir);
    if !upload_path.exists() {
        std::fs::create_dir_all(&upload_path).expect("Failed to create upload directory");
    }

    let pool = state::init_database(&database_url)
        .await
        .expect("Failed to initialize database");

    let app_state = state::AppState::new(pool, upload_dir, jwt_secret);

    let app = routes::create_router(app_state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr = std::env::var("LISTEN_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    tracing::info!("Gridow server listening on {}", addr);

    axum::serve(listener, app)
        .await
        .expect("Server error");
}