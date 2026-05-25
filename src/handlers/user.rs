use axum::extract::{Json, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json as AxumJson};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::Utc;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde_json::json;

use crate::auth::AuthUser;
use crate::models::error::AppError;
use crate::models::user::{
    ChangePasswordRequest, UpgradeVIPRequest, UpgradeVIPResponse, User, UserInfo,
    UserLoginRequest, UserLoginResponse, UserRegisterRequest, UserUpdateRequest,
};
use crate::state::AppState;

async fn resolve_internal_user_id(state: &AppState, public_id: &str) -> Result<i64, AppError> {
    let (id,): (i64,) = sqlx::query_as("SELECT id FROM users WHERE public_id = $1")
        .bind(public_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;
    Ok(id)
}

pub async fn register_user(
    State(state): State<AppState>,
    Json(req): Json<UserRegisterRequest>,
) -> Result<AxumJson<UserInfo>, AppError> {
    let exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE email = $1")
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
    let (id, public_id) = state.new_id();

    sqlx::query(
        r#"INSERT INTO users (id, public_id, username, email, password_hash, avatar, role, status, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, 'user', 'active', $7, $8)"#,
    )
    .bind(id)
    .bind(&public_id)
    .bind(&req.username)
    .bind(&req.email)
    .bind(&password_hash)
    .bind(&req.avatar)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(AppError::Database)?;

    let defaults = vec![("日用品", "🧴"), ("食品", "🍎"), ("工具", "🔧"), ("药品", "💊"), ("服装", "👕"), ("电子", "🔌")];
    let cat_now = chrono::Utc::now().naive_utc().format("%Y-%m-%d %H:%M:%S").to_string();
    for (i, (name, icon)) in defaults.iter().enumerate() {
        let (cat_id, cat_public_id) = state.new_id();
        let cat_version = state.next_version().await.map_err(AppError::Database)?;
        sqlx::query(
            r#"INSERT INTO categories (id, public_id, name, icon, sort_order, owner_id, created_at, version, is_deleted)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
        )
        .bind(cat_id).bind(&cat_public_id).bind(name).bind(icon)
        .bind(i as i32).bind(id).bind(&cat_now)
        .bind(cat_version).bind(0i16)
        .execute(&state.db).await.map_err(AppError::Database)?;
    }

    Ok(AxumJson(UserInfo {
        id: public_id,
        username: req.username,
        email: req.email,
        avatar: req.avatar,
        role: "user".to_string(),
        status: "active".to_string(),
        created_at: now,
    }))
}

pub async fn login_user(
    State(state): State<AppState>,
    Json(req): Json<UserLoginRequest>,
) -> Result<AxumJson<UserLoginResponse>, AppError> {
    let user: User = match sqlx::query_as("SELECT * FROM users WHERE email = $1")
        .bind(&req.email)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
    {
        Some(u) => u,
        None => return Err(AppError::BadRequest("邮箱或密码错误".to_string())),
    };

    if !verify(&req.password, &user.password_hash)
        .map_err(|_| AppError::Internal("密码验证失败".to_string()))?
    {
        return Err(AppError::BadRequest("邮箱或密码错误".to_string()));
    }

    if user.status != "active" {
        return Err(AppError::Forbidden);
    }

    let token = generate_token(&user.public_id, &state.jwt_secret).await?;

    Ok(AxumJson(UserLoginResponse {
        user: UserInfo {
            id: user.public_id,
            username: user.username,
            email: user.email,
            avatar: user.avatar,
            role: user.role,
            status: user.status,
            created_at: user.created_at,
        },
        token,
    }))
}

pub async fn logout_user() -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

pub async fn get_user_info(
    State(state): State<AppState>,
    Path(user_public_id): Path<String>,
) -> Result<AxumJson<UserInfo>, AppError> {
    let user: User = sqlx::query_as("SELECT * FROM users WHERE public_id = $1")
        .bind(&user_public_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    Ok(AxumJson(UserInfo {
        id: user.public_id,
        username: user.username,
        email: user.email,
        avatar: user.avatar,
        role: user.role,
        status: user.status,
        created_at: user.created_at,
    }))
}

pub async fn update_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(user_public_id): Path<String>,
    Json(req): Json<UserUpdateRequest>,
) -> Result<AxumJson<UserInfo>, AppError> {
    let mut user: User = sqlx::query_as("SELECT * FROM users WHERE public_id = $1")
        .bind(&user_public_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    let auth_internal_id = resolve_internal_user_id(&state, &auth.public_id).await?;
    if user.id != auth_internal_id {
        return Err(AppError::Forbidden);
    }

    if req.username.is_some() {
        let password = req
            .password
            .as_ref()
            .ok_or(AppError::BadRequest("修改用户名需要验证密码".to_string()))?;
        if !verify(password, &user.password_hash)
            .map_err(|_| AppError::Internal("密码验证失败".to_string()))?
        {
            return Err(AppError::BadRequest("密码错误".to_string()));
        }
    }

    if let Some(username) = req.username {
        user.username = username;
    }
    if let Some(avatar) = req.avatar {
        user.avatar = avatar;
    }

    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    sqlx::query("UPDATE users SET username=$1, avatar=$2, updated_at=$3 WHERE id=$4")
        .bind(&user.username)
        .bind(&user.avatar)
        .bind(&now)
        .bind(user.id)
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?;

    Ok(AxumJson(UserInfo {
        id: user.public_id,
        username: user.username,
        email: user.email,
        avatar: user.avatar,
        role: user.role,
        status: user.status,
        created_at: user.created_at,
    }))
}

pub async fn change_password(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(user_public_id): Path<String>,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<impl IntoResponse, AppError> {
    let user: User = sqlx::query_as("SELECT * FROM users WHERE public_id = $1")
        .bind(&user_public_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    let auth_internal_id = resolve_internal_user_id(&state, &auth.public_id).await?;
    if user.id != auth_internal_id {
        return Err(AppError::Forbidden);
    }

    if !verify(&req.old_password, &user.password_hash)
        .map_err(|_| AppError::Internal("密码验证失败".to_string()))?
    {
        return Err(AppError::BadRequest("旧密码错误".to_string()));
    }

    let new_password_hash = hash(&req.new_password, DEFAULT_COST)
        .map_err(|_| AppError::Internal("密码加密失败".to_string()))?;

    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    sqlx::query("UPDATE users SET password_hash=$1, updated_at=$2 WHERE id=$3")
        .bind(&new_password_hash)
        .bind(&now)
        .bind(user.id)
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?;

    Ok(StatusCode::OK)
}

pub async fn upgrade_vip(
    State(state): State<AppState>,
    Path(user_public_id): Path<String>,
    Json(req): Json<UpgradeVIPRequest>,
) -> Result<AxumJson<UpgradeVIPResponse>, AppError> {
    let plan = req.plan.to_lowercase();
    let new_role = match plan.as_str() {
        "vip" => "vip",
        "vip_plus" => "vip_plus",
        _ => return Err(AppError::BadRequest("无效的会员计划".to_string())),
    };

    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let user: User = sqlx::query_as("SELECT * FROM users WHERE public_id = $1")
        .bind(&user_public_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)?;

    sqlx::query("UPDATE users SET role=$1, updated_at=$2 WHERE id=$3")
        .bind(new_role)
        .bind(&now)
        .bind(user.id)
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?;

    Ok(AxumJson(UpgradeVIPResponse {
        success: true,
        message: "升级成功".to_string(),
        new_role: new_role.to_string(),
    }))
}

async fn generate_token(public_id: &str, secret: &str) -> Result<String, AppError> {
    let payload = json!({
        "public_id": public_id,
        "exp": Utc::now().timestamp() + 86400 * 7
    });

    encode(
        &Header::new(Algorithm::HS256),
        &payload,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| AppError::Internal("Token 生成失败".to_string()))
}
