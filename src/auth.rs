//! JWT 认证模块
//!
//! 提供基于 Axum `FromRequestParts` 的 JWT Token 自动提取器。
//! 所有需要认证的接口通过注入 `AuthUser` 参数即可获取当前登录用户 ID，
//! 无需认证的接口则无需声明该参数。

use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, StatusCode},
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::Deserialize;

use crate::state::AppState;

/// 认证用户提取器
///
/// 从 HTTP 请求的 `Authorization: Bearer <token>` 头中解析 JWT，
/// 提取其中的 `user_id`。若 Token 缺失或无效则返回 401。
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: String,
}

/// JWT 载荷结构
///
/// 包含用户标识和过期时间戳。`exp` 字段由 `jsonwebtoken` 库自动校验，
/// 此处仅需声明以供反序列化。
#[derive(Debug, Deserialize)]
struct JwtClaims {
    user_id: String,
    #[allow(dead_code)]
    exp: usize,
}

/// 验证 JWT Token 并提取用户 ID
///
/// # 参数
/// * `token` - Bearer 之后的 Token 字符串
/// * `secret` - 用于 HMAC-SHA256 签名的密钥
///
/// # 返回
/// * `Ok(user_id)` - 验证通过
/// * `Err((401, msg))` - Token 无效或已过期
fn verify_token(token: &str, secret: &str) -> Result<String, (StatusCode, &'static str)> {
    let token_data = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid token"))?;

    Ok(token_data.claims.user_id)
}

/// 实现 `FromRequestParts` 以支持在 Handler 参数中直接注入 `AuthUser`
///
/// Axum 会在调用 Handler 前自动调用此方法，从请求头中提取并验证 JWT。
/// 若验证失败，请求将返回 401 而不会进入 Handler 逻辑。
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // 提取 Authorization 头
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization header"))?;

        // 剥离 "Bearer " 前缀
        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or((StatusCode::UNAUTHORIZED, "Invalid Authorization format"))?;

        // 验证 Token 并提取用户 ID
        let user_id = verify_token(token, &state.jwt_secret)?;

        Ok(AuthUser { user_id })
    }
}
