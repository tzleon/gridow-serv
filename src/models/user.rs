use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    #[serde(skip)]
    pub id: i64,
    #[serde(rename = "id")]
    pub public_id: String,
    pub username: String,
    pub email: String,
    #[serde(skip)]
    pub password_hash: String,
    pub avatar: String,
    pub role: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct UserRegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub avatar: String,
}

#[derive(Debug, Deserialize)]
pub struct UserLoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct UserLoginResponse {
    pub user: UserInfo,
    pub token: String,
}

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

#[derive(Debug, Deserialize)]
pub struct UserUpdateRequest {
    pub username: Option<String>,
    pub avatar: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpgradeVIPRequest {
    pub plan: String,
}

#[derive(Debug, Serialize)]
pub struct UpgradeVIPResponse {
    pub success: bool,
    pub message: String,
    pub new_role: String,
}
