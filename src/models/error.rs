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

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    async fn collect_body(resp: Response) -> (StatusCode, serde_json::Value) {
        let status = resp.status();
        let body = resp.into_body();
        let body_bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        (status, json)
    }

    #[tokio::test]
    async fn test_app_error_not_found() {
        let err = AppError::NotFound;
        assert_eq!(err.to_string(), "资源不存在");
        let (status, body) = collect_body(err.into_response()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], 404);
        assert_eq!(body["message"], "资源不存在");
    }

    #[tokio::test]
    async fn test_app_error_bad_request() {
        let err = AppError::BadRequest("参数无效".into());
        assert_eq!(err.to_string(), "请求参数错误: 参数无效");
        let (status, body) = collect_body(err.into_response()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], 400);
        assert_eq!(body["message"], "参数无效");
    }

    #[tokio::test]
    async fn test_app_error_forbidden() {
        let err = AppError::Forbidden;
        assert_eq!(err.to_string(), "访问被拒绝");
        let (status, body) = collect_body(err.into_response()).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], 403);
        assert_eq!(body["message"], "访问被拒绝");
    }

    #[tokio::test]
    async fn test_app_error_payload_too_large() {
        let err = AppError::PayloadTooLarge;
        assert_eq!(err.to_string(), "文件过大");
        let (status, body) = collect_body(err.into_response()).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(body["code"], 413);
        assert_eq!(body["message"], "文件过大");
    }

    #[tokio::test]
    async fn test_app_error_internal() {
        let err = AppError::Internal("内部错误测试".into());
        assert_eq!(err.to_string(), "内部错误: 内部错误测试");
        let (status, body) = collect_body(err.into_response()).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["code"], 500);
        assert_eq!(body["message"], "内部错误测试");
    }

    #[test]
    fn test_app_error_from_sqlx_error() {
        let sqlx_err = sqlx::Error::Protocol("test".into());
        let app_err: AppError = sqlx_err.into();
        match app_err {
            AppError::Database(_) => {}
            _ => panic!("Expected AppError::Database"),
        }
    }

    #[test]
    fn test_app_error_display() {
        assert_eq!(format!("{}", AppError::NotFound), "资源不存在");
        assert_eq!(format!("{}", AppError::Forbidden), "访问被拒绝");
        assert_eq!(format!("{}", AppError::PayloadTooLarge), "文件过大");
    }

    #[test]
    fn test_app_error_implements_std_error() {
        fn check_error(_e: &dyn Error) {}
        check_error(&AppError::NotFound);
        check_error(&AppError::BadRequest("test".into()));
    }
}
