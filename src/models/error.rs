//! 统一错误处理
//!
//! `AppError` 枚举覆盖所有业务异常，实现 `IntoResponse` 以自动转换为 HTTP 响应。
//! 使用 `thiserror` 派生 `Display` / `Error` trait，
//! 使用 `sqlx::Error` 的 `From` 自动转换数据库错误。

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// 应用层错误枚举
///
/// 各变体映射到对应的 HTTP 状态码和业务错误码。
/// 数据库错误（`sqlx::Error`）通过 `#[from]` 自动转换。
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("资源不存在")]
    NotFound,

    #[error("请求参数错误: {0}")]
    BadRequest(String),

    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),

    #[error("文件过大")]
    PayloadTooLarge,

    #[error("内部错误: {0}")]
    Internal(String),

    #[error("访问被拒绝")]
    Forbidden,
}

/// 统一错误响应体
///
/// 所有错误接口返回的 JSON 格式均为 `{ "code": 4xx/5xx, "message": "..." }`
#[derive(Serialize)]
struct ErrorBody {
    code: i32,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, 404, self.to_string()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, 400, msg.clone()),
            AppError::Database(e) => {
                // 数据库错误只向客户端返回通用消息，详细日志由 tracing 记录
                tracing::error!("Database error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    500,
                    "数据库错误".to_string(),
                )
            }
            AppError::PayloadTooLarge => (StatusCode::PAYLOAD_TOO_LARGE, 413, self.to_string()),
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, 500, msg.clone())
            }
            AppError::Forbidden => (StatusCode::FORBIDDEN, 403, self.to_string()),
        };

        (status, axum::Json(ErrorBody { code, message })).into_response()
    }
}
