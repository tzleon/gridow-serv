use axum::extract::{Json, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json as AxumJson};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::Utc;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use rand::Rng;
use serde_json::json;

use crate::auth::AuthUser;
use crate::models::error::AppError;
use crate::models::user::{
    ChangePasswordRequest, ForgotPasswordRequest, ResetPasswordRequest, SendCodeResponse,
    UpgradeVIPRequest, UpgradeVIPResponse, User, UserInfo, UserLoginRequest, UserLoginResponse,
    UserRegisterRequest, UserUpdateRequest, VerifyCodeRequest, VerifyCodeResponse,
};
use crate::state::AppState;

const JWT_EXPIRATION_SECONDS: i64 = 86400 * 7;
const RESET_CODE_EXPIRATION_SECONDS: i64 = 300;
const RESET_CODE_LENGTH: usize = 6;
const MIN_PASSWORD_LENGTH: usize = 6;

fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err(AppError::BadRequest(format!("密码长度不能少于{}位", MIN_PASSWORD_LENGTH)));
    }
    Ok(())
}

async fn get_user_by_public_id(state: &AppState, public_id: &str) -> Result<User, AppError> {
    sqlx::query_as("SELECT * FROM users WHERE public_id = $1")
        .bind(public_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or(AppError::NotFound)
}

async fn get_user_by_email(pool: &sqlx::PgPool, email: &str) -> Result<Option<User>, AppError> {
    sqlx::query_as("SELECT * FROM users WHERE email = $1")
        .bind(email)
        .fetch_optional(pool)
        .await
        .map_err(AppError::Database)
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

    validate_password(&req.password)?;

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
    let user = match get_user_by_email(&state.db, &req.email).await? {
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

    let token = generate_token(&user.public_id, &state.jwt_secret)?;

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
    let user = get_user_by_public_id(&state, &user_public_id).await?;

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
    let mut user = get_user_by_public_id(&state, &user_public_id).await?;

    let auth_internal_id = state.resolve_user_id(&auth.public_id).await?;
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
    let user = get_user_by_public_id(&state, &user_public_id).await?;

    let auth_internal_id = state.resolve_user_id(&auth.public_id).await?;
    if user.id != auth_internal_id {
        return Err(AppError::Forbidden);
    }

    if !verify(&req.old_password, &user.password_hash)
        .map_err(|_| AppError::Internal("密码验证失败".to_string()))?
    {
        return Err(AppError::BadRequest("旧密码错误".to_string()));
    }

    validate_password(&req.new_password)?;

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
    auth: AuthUser,
    Path(user_public_id): Path<String>,
    Json(req): Json<UpgradeVIPRequest>,
) -> Result<AxumJson<UpgradeVIPResponse>, AppError> {
    let plan = req.plan.to_lowercase();
    let new_role = match plan.as_str() {
        "vip" => "vip",
        "vip_plus" => "vip_plus",
        _ => return Err(AppError::BadRequest("无效的会员计划".to_string())),
    };

    let user = get_user_by_public_id(&state, &user_public_id).await?;
    let auth_internal_id = state.resolve_user_id(&auth.public_id).await?;
    
    if user.id != auth_internal_id {
        return Err(AppError::Forbidden);
    }

    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

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

fn generate_token(public_id: &str, secret: &str) -> Result<String, AppError> {
    let payload = json!({
        "public_id": public_id,
        "exp": Utc::now().timestamp() + JWT_EXPIRATION_SECONDS
    });

    encode(
        &Header::new(Algorithm::HS256),
        &payload,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| AppError::Internal("Token 生成失败".to_string()))
}

async fn resolve_user_id_by_email(pool: &sqlx::PgPool, email: &str) -> Result<Option<i64>, AppError> {
    let result: Option<(i64,)> = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_optional(pool)
        .await
        .map_err(AppError::Database)?;
    Ok(result.map(|r| r.0))
}

async fn verify_code_in_db(
    pool: &sqlx::PgPool, user_id: i64, code: &str,
) -> Result<bool, AppError> {
    let now = Utc::now().timestamp();
    let result: Option<(String,)> = sqlx::query_as(
        "SELECT code FROM password_reset_codes WHERE user_id = $1 AND code = $2 AND expires_at > $3"
    )
    .bind(user_id)
    .bind(code)
    .bind(now)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)?;
    Ok(result.is_some())
}

fn generate_reset_code() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(RESET_CODE_LENGTH)
        .map(|b: u8| b as char)
        .collect::<String>()
        .to_ascii_uppercase()
}

fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

pub async fn send_reset_code(
    State(state): State<AppState>,
    Json(req): Json<ForgotPasswordRequest>,
) -> Result<AxumJson<SendCodeResponse>, AppError> {
    let email = normalize_email(&req.email);

    let user_id = match resolve_user_id_by_email(&state.db, &email).await? {
        Some(id) => id,
        None => return Ok(AxumJson(SendCodeResponse {
            success: true,
            message: "验证码已发送，请查收邮箱".to_string(),
        })),
    };

    let code = generate_reset_code();

    let expires_at = Utc::now().timestamp() + RESET_CODE_EXPIRATION_SECONDS;
    let (id, _) = state.new_id();

    sqlx::query(
        "INSERT INTO password_reset_codes (id, user_id, code, expires_at) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (user_id) DO UPDATE SET code = $3, expires_at = $4"
    )
    .bind(id)
    .bind(user_id)
    .bind(&code)
    .bind(expires_at)
    .execute(&state.db)
    .await
    .map_err(AppError::Database)?;

    send_email(&email, &code).await;

    Ok(AxumJson(SendCodeResponse {
        success: true,
        message: "验证码已发送，请查收邮箱".to_string(),
    }))
}

pub async fn verify_reset_code(
    State(state): State<AppState>,
    Json(req): Json<VerifyCodeRequest>,
) -> Result<AxumJson<VerifyCodeResponse>, AppError> {
    let user_id = resolve_user_id_by_email(&state.db, &req.email)
        .await?
        .ok_or(AppError::BadRequest("验证码无效或已过期".to_string()))?;

    let valid = verify_code_in_db(&state.db, user_id, &req.code).await?;

    match valid {
        true => Ok(AxumJson(VerifyCodeResponse {
            success: true,
            message: "验证码验证成功".to_string(),
        })),
        false => Err(AppError::BadRequest("验证码无效或已过期".to_string())),
    }
}

pub async fn reset_password(
    State(state): State<AppState>,
    Json(req): Json<ResetPasswordRequest>,
) -> Result<AxumJson<SendCodeResponse>, AppError> {
    let user_id = resolve_user_id_by_email(&state.db, &req.email)
        .await?
        .ok_or(AppError::BadRequest("验证码无效或已过期".to_string()))?;

    let valid = verify_code_in_db(&state.db, user_id, &req.code).await?;
    if !valid {
        return Err(AppError::BadRequest("验证码无效或已过期".to_string()));
    }

    validate_password(&req.new_password)?;

    let new_password_hash = hash(&req.new_password, DEFAULT_COST)
        .map_err(|_| AppError::Internal("密码加密失败".to_string()))?;

    let now_str = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let mut tx = state.db.begin().await.map_err(AppError::Database)?;

    sqlx::query("UPDATE users SET password_hash=$1, updated_at=$2 WHERE id=$3")
        .bind(&new_password_hash)
        .bind(&now_str)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;

    sqlx::query("DELETE FROM password_reset_codes WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;

    tx.commit().await.map_err(AppError::Database)?;

    Ok(AxumJson(SendCodeResponse {
        success: true,
        message: "密码重置成功".to_string(),
    }))
}

async fn send_email(email: &str, _code: &str) {
    tracing::info!("邮件发送未实现: email={} — 请配置真实邮件服务", email);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_password_valid() {
        assert!(validate_password("123456").is_ok());
        assert!(validate_password("abcdef").is_ok());
        assert!(validate_password("longpassword123!@#").is_ok());
    }

    #[test]
    fn test_validate_password_too_short() {
        assert!(validate_password("12345").is_err());
        assert!(validate_password("a").is_err());
        assert!(validate_password("").is_err());
    }

    #[test]
    fn test_validate_password_exact_minimum() {
        assert!(validate_password("123456").is_ok());
        assert!(validate_password("12345").is_err());
    }

    #[test]
    fn test_validate_password_error_message() {
        let err = validate_password("abc").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(&MIN_PASSWORD_LENGTH.to_string()));
    }

    #[test]
    fn test_generate_reset_code_length() {
        for _ in 0..20 {
            let code = generate_reset_code();
            assert_eq!(code.len(), RESET_CODE_LENGTH, "Code length mismatch: {}", code);
        }
    }

    #[test]
    fn test_generate_reset_code_uppercase_alphanumeric() {
        for _ in 0..20 {
            let code = generate_reset_code();
            for ch in code.chars() {
                assert!(ch.is_ascii_alphanumeric(),
                    "Character '{}' is not alphanumeric", ch);
                if ch.is_ascii_alphabetic() {
                    assert!(ch.is_ascii_uppercase(),
                        "Alphabetic character '{}' is not uppercase", ch);
                }
            }
        }
    }

    #[test]
    fn test_generate_reset_code_randomness() {
        let codes: std::collections::HashSet<String> = (0..20)
            .map(|_| generate_reset_code())
            .collect();
        assert!(codes.len() > 1, "Generated codes should not all be identical");
    }

    #[test]
    fn test_normalize_email_trims_whitespace() {
        assert_eq!(normalize_email("  user@example.com  "), "user@example.com");
        assert_eq!(normalize_email("\tuser@example.com\n"), "user@example.com");
    }

    #[test]
    fn test_normalize_email_lowercases() {
        assert_eq!(normalize_email("User@Example.COM"), "user@example.com");
        assert_eq!(normalize_email("USER@EXAMPLE.COM"), "user@example.com");
    }

    #[test]
    fn test_normalize_email_combined() {
        assert_eq!(normalize_email("  User@Example.COM  "), "user@example.com");
    }

    #[test]
    fn test_normalize_email_empty() {
        assert_eq!(normalize_email(""), "");
        assert_eq!(normalize_email("   "), "");
    }

    #[test]
    fn test_constants_sensible() {
        assert_eq!(RESET_CODE_LENGTH, 6);
        assert_eq!(RESET_CODE_EXPIRATION_SECONDS, 300);
        assert!(MIN_PASSWORD_LENGTH >= 6);
        assert!(JWT_EXPIRATION_SECONDS > 0);
    }
}
