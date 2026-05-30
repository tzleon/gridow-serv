//! Gridow 格物 — 物品管理后台服务
//!
//! 基于 Axum + SQLx + PostgreSQL 构建的 RESTful API 服务。
//! 支持物品 CRUD、空间树管理、用户认证、协管授权、数据同步等功能。
//! 使用雪花算法生成全局唯一 BigInt ID。
//!
//! # 启动
//! ```bash
//! DATABASE_URL=postgresql://... LISTEN_ADDR=0.0.0.0:8080 cargo run
//! ```
//!
//! # 环境变量
//! * `DATABASE_URL`         - PostgreSQL 连接字符串
//! * `LISTEN_ADDR`          - 监听地址（默认 0.0.0.0:8080）
//! * `UPLOAD_DIR`           - 图片上传目录（默认 ./uploads）
//! * `LOG_DIR`              - 日志文件目录（默认 ./logs）
//! * `JWT_SECRET`           - JWT 签名密钥
//! * `SNOWFLAKE_WORKER_ID`  - 雪花算法 Worker ID（0~1023，默认 0）

use std::path::PathBuf;

use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use gridow_web::{logging, routes, snowflake::Snowflake, state};

/// 应用程序入口
#[tokio::main]
async fn main() {
    let log_dir = std::env::var("LOG_DIR")
        .unwrap_or_else(|_| "./logs".to_string());

    logging::init_logging(&log_dir);

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL 未设置，请在 gridow.conf 或环境变量中配置");

    let upload_dir = std::env::var("UPLOAD_DIR")
        .unwrap_or_else(|_| "./uploads".to_string());

    let jwt_secret = std::env::var("JWT_SECRET")
        .expect("JWT_SECRET 未设置，请在 gridow.conf 或环境变量中配置");

    let worker_id: i64 = std::env::var("SNOWFLAKE_WORKER_ID")
        .unwrap_or_else(|_| "0".to_string())
        .parse()
        .expect("SNOWFLAKE_WORKER_ID 必须为 0~1023 的整数");

    let upload_path = PathBuf::from(&upload_dir);
    if !upload_path.exists() {
        std::fs::create_dir_all(&upload_path).expect("Failed to create upload directory");
    }

    let snowflake = Snowflake::new(worker_id);
    tracing::info!("Snowflake generator ready: worker_id={}", worker_id);

    let pool = state::init_database(&database_url)
        .await
        .expect("Failed to initialize database");

    let base_url = std::env::var("BASE_URL")
        .unwrap_or_else(|_| "https://gridow.richking.top".to_string());
    let app_state = state::AppState::new(pool, upload_dir, jwt_secret, base_url, snowflake);

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
