//! 用户数据模型
//!
//! 包含数据库实体 `User`、对外暴露的 `UserInfo`（不含密码哈希）、
//! 以及注册/登录/更新/升级等接口的请求响应结构体。

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 用户数据库实体（全字段）
///
/// `password_hash` 仅在认证流程中使用，对外接口返回 `UserInfo`。
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    /// bcrypt 哈希后的密码
    pub password_hash: String,
    pub avatar: String,
    /// 角色：`user` / `vip` / `vip_plus`
    pub role: String,
    /// 状态：`active` / `disabled`
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 用户注册请求
#[derive(Debug, Deserialize)]
pub struct UserRegisterRequest {
    pub username: String,
    pub email: String,
    /// 明文密码，服务端使用 bcrypt 哈希后存储
    pub password: String,
    #[serde(default)]
    pub avatar: String,
}

/// 用户登录请求
#[derive(Debug, Deserialize)]
pub struct UserLoginRequest {
    pub email: String,
    pub password: String,
}

/// 登录响应（含 JWT Token）
#[derive(Debug, Serialize)]
pub struct UserLoginResponse {
    pub user: UserInfo,
    /// Bearer Token，客户端后续请求需携带
    pub token: String,
}

/// 对外暴露的用户信息（不含密码哈希）
#[derive(Debug, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub email: String,
    pub avatar: String,
    pub role: String,
    pub status: String,
    pub created_at: String,
}

/// 用户信息更新请求（仅允许修改 username 和 avatar）
#[derive(Debug, Deserialize)]
pub struct UserUpdateRequest {
    pub username: Option<String>,
    pub avatar: Option<String>,
}

/// VIP 升级请求
#[derive(Debug, Deserialize)]
pub struct UpgradeVIPRequest {
    /// 会员计划：`vip` 或 `vip_plus`
    pub plan: String,
}

/// VIP 升级响应
#[derive(Debug, Serialize)]
pub struct UpgradeVIPResponse {
    pub success: bool,
    pub message: String,
    pub new_role: String,
}

/// 生成 UUID v4 作为用户 ID
pub fn new_user_id() -> String {
    Uuid::new_v4().to_string()
}
