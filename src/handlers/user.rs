use axum::extract::{Json, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json as AxumJson};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::Utc;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde_json::json;

use crate::models::error::AppError;
use crate::models::user::{
    new_user_id, UpgradeVIPRequest, UpgradeVIPResponse, User, UserInfo, UserLoginRequest,
    UserLoginResponse, UserRegisterRequest, UserUpdateRequest,
};
use crate::state::AppState;

pub async fn register_user(
    State(state): State<AppState>,
    Json(req): Json<UserRegisterRequest>,
) -> Result<AxumJson<UserInfo>, AppError> {
    let exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE email = ?")
        .bind(&req.email)
        .fetch_one(&state.db)
        .await
        .map_err(AppError::Database)?;

    if exists.0 > 0 {
        return Err(AppError::BadRequest("邮箱已被注册".to_string()));
    }

    let password_hash = hash(&req.password, DEFAULT_COST)
        .map_err(|_| AppError::Internal("密码加密失败".to_string()))?;

    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let user_id = new_user_id();

    sqlx::query(
        r#"INSERT INTO users (id, username, email, password_hash, avatar, role, status, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, 'user', 'active', ?, ?)"#,
    )
    .bind(&user_id)
    .bind(&req.username)
    .bind(&req.email)
    .bind(&password_hash)
    .bind(&req.avatar)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(AppError::Database)?;

    let user_info = UserInfo {
        id: user_id,
        username: req.username,
        email: req.email,
        avatar: req.avatar,
        role: "user".to_string(),
        status: "active".to_string(),
        created_at: now,
    };

    Ok(AxumJson(user_info))
}

pub async fn login_user(
    State(state): State<AppState>,
    Json(req): Json<UserLoginRequest>,
) -> Result<AxumJson<UserLoginResponse>, AppError> {
    let user: User = sqlx::query_as("SELECT * FROM users WHERE email = ?")
        .bind(&req.email)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    if !verify(&req.password, &user.password_hash)
        .map_err(|_| AppError::Internal("密码验证失败".to_string()))?
    {
        return Err(AppError::BadRequest("邮箱或密码错误".to_string()));
    }

    if user.status != "active" {
        return Err(AppError::Forbidden);
    }

    let token = generate_token(&user.id, &state.jwt_secret).await?;

    let user_info = UserInfo {
        id: user.id,
        username: user.username,
        email: user.email,
        avatar: user.avatar,
        role: user.role,
        status: user.status,
        created_at: user.created_at,
    };

    Ok(AxumJson(UserLoginResponse { user: user_info, token }))
}

pub async fn logout_user() -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

pub async fn get_user_info(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<AxumJson<UserInfo>, AppError> {
    let user: User = sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(&user_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    let user_info = UserInfo {
        id: user.id,
        username: user.username,
        email: user.email,
        avatar: user.avatar,
        role: user.role,
        status: user.status,
        created_at: user.created_at,
    };

    Ok(AxumJson(user_info))
}

pub async fn update_user(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    Json(req): Json<UserUpdateRequest>,
) -> Result<AxumJson<UserInfo>, AppError> {
    let mut user: User = sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(&user_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    if let Some(username) = req.username {
        user.username = username;
    }
    if let Some(avatar) = req.avatar {
        user.avatar = avatar;
    }

    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    sqlx::query(
        r#"UPDATE users SET username=?, avatar=?, updated_at=? WHERE id=?"#,
    )
    .bind(&user.username)
    .bind(&user.avatar)
    .bind(&now)
    .bind(&user_id)
    .execute(&state.db)
    .await
    .map_err(AppError::Database)?;

    let user_info = UserInfo {
        id: user.id,
        username: user.username,
        email: user.email,
        avatar: user.avatar,
        role: user.role,
        status: user.status,
        created_at: user.created_at,
    };

    Ok(AxumJson(user_info))
}

pub async fn upgrade_vip(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    Json(req): Json<UpgradeVIPRequest>,
) -> Result<AxumJson<UpgradeVIPResponse>, AppError> {
    let plan = req.plan.to_lowercase();
    let new_role = match plan.as_str() {
        "vip" => "vip",
        "vip_plus" => "vip_plus",
        _ => return Err(AppError::BadRequest("无效的会员计划".to_string())),
    };

    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    sqlx::query(r#"UPDATE users SET role=?, updated_at=? WHERE id=?"#)
        .bind(new_role)
        .bind(&now)
        .bind(&user_id)
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?;

    Ok(AxumJson(UpgradeVIPResponse {
        success: true,
        message: "升级成功".to_string(),
        new_role: new_role.to_string(),
    }))
}

async fn generate_token(user_id: &str, secret: &str) -> Result<String, AppError> {
    let payload = json!({
        "user_id": user_id,
        "exp": Utc::now().timestamp() + 86400 * 7
    });

    encode(
        &Header::new(Algorithm::HS256),
        &payload,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| AppError::Internal("Token 生成失败".to_string()))
}