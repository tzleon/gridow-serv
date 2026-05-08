use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

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
