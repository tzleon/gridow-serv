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
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpgradeVIPRequest {
    pub plan: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct UpgradeVIPResponse {
    pub success: bool,
    pub message: String,
    pub new_role: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_register_request() {
        let json = r#"{"username": "test", "email": "test@example.com", "password": "123456"}"#;
        let req: UserRegisterRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.username, "test");
        assert_eq!(req.email, "test@example.com");
        assert_eq!(req.password, "123456");
        assert_eq!(req.avatar, ""); // default
    }

    #[test]
    fn test_user_register_request_with_avatar() {
        let json = r#"{"username": "test", "email": "test@example.com", "password": "123456", "avatar": "https://example.com/av.jpg"}"#;
        let req: UserRegisterRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.avatar, "https://example.com/av.jpg");
    }

    #[test]
    fn test_user_login_request() {
        let json = r#"{"email": "test@example.com", "password": "123456"}"#;
        let req: UserLoginRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.email, "test@example.com");
        assert_eq!(req.password, "123456");
    }

    #[test]
    fn test_user_login_response_serialization() {
        let resp = UserLoginResponse {
            user: UserInfo {
                id: "u1".into(),
                username: "test".into(),
                email: "test@example.com".into(),
                avatar: "".into(),
                role: "user".into(),
                status: "active".into(),
                created_at: "2024-01-01".into(),
            },
            token: "jwt_token".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("u1"));
        assert!(json.contains("jwt_token"));
        assert!(json.contains("test@example.com"));
    }

    #[test]
    fn test_user_info_serialization() {
        let info = UserInfo {
            id: "u1".into(),
            username: "test".into(),
            email: "test@example.com".into(),
            avatar: "".into(),
            role: "user".into(),
            status: "active".into(),
            created_at: "2024-01-01".into(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], "u1");
        assert_eq!(parsed["username"], "test");
        // password_hash 不应出现在 UserInfo 中
        assert!(parsed.get("password_hash").is_none());
    }

    #[test]
    fn test_user_update_request_partial() {
        let json = r#"{"username": "新用户名"}"#;
        let req: UserUpdateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.username, Some("新用户名".into()));
        assert_eq!(req.avatar, None);
        assert_eq!(req.password, None);
    }

    #[test]
    fn test_user_update_request_full() {
        let json = r#"{"username": "new", "avatar": "av.jpg", "password": "new_pwd"}"#;
        let req: UserUpdateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.username, Some("new".into()));
        assert_eq!(req.avatar, Some("av.jpg".into()));
        assert_eq!(req.password, Some("new_pwd".into()));
    }

    #[test]
    fn test_upgrade_vip_request() {
        let json = r#"{"plan": "vip"}"#;
        let req: UpgradeVIPRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.plan, "vip");
    }

    #[test]
    fn test_upgrade_vip_response_serialization() {
        let resp = UpgradeVIPResponse {
            success: true,
            message: "升级成功".into(),
            new_role: "vip".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("true"));
        assert!(json.contains("升级成功"));
    }

    #[test]
    fn test_change_password_request() {
        let json = r#"{"old_password": "old", "new_password": "new"}"#;
        let req: ChangePasswordRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.old_password, "old");
        assert_eq!(req.new_password, "new");
    }

    #[test]
    fn test_user_serialization_skips_internal_fields() {
        let user = User {
            id: 12345, public_id: "pub_u1".into(), username: "test".into(),
            email: "test@test.com".into(), password_hash: "secret_hash".into(),
            avatar: "".into(), role: "user".into(), status: "active".into(),
            created_at: "now".into(), updated_at: "now".into(),
        };
        let json = serde_json::to_string(&user).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["id"], "pub_u1");
        assert!(parsed.get("password_hash").is_none(), "password_hash should be skipped");
    }

    #[test]
    fn test_user_register_request_missing_username() {
        let json = r#"{"email": "a@b.com", "password": "123"}"#;
        let result = serde_json::from_str::<UserRegisterRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_user_register_request_missing_email() {
        let json = r#"{"username": "test", "password": "123"}"#;
        let result = serde_json::from_str::<UserRegisterRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_user_register_request_missing_password() {
        let json = r#"{"username": "test", "email": "a@b.com"}"#;
        let result = serde_json::from_str::<UserRegisterRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_user_login_request_missing_email() {
        let json = r#"{"password": "123"}"#;
        let result = serde_json::from_str::<UserLoginRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_user_login_request_missing_password() {
        let json = r#"{"email": "a@b.com"}"#;
        let result = serde_json::from_str::<UserLoginRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_user_register_request_invalid_json() {
        let json = r#"not valid json"#;
        let result = serde_json::from_str::<UserRegisterRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_user_update_request_empty() {
        let json = r#"{}"#;
        let req: UserUpdateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.username, None);
        assert_eq!(req.avatar, None);
        assert_eq!(req.password, None);
    }

    #[test]
    fn test_change_password_request_missing_field() {
        let json = r#"{"old_password": "old"}"#;
        let result = serde_json::from_str::<ChangePasswordRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_upgrade_vip_request_missing_plan() {
        let json = r#"{}"#;
        let result = serde_json::from_str::<UpgradeVIPRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_user_info_roundtrip() {
        let original = UserInfo {
            id: "uid_abc".into(),
            username: "roundtrip_user".into(),
            email: "rt@test.com".into(),
            avatar: "avatar.png".into(),
            role: "admin".into(),
            status: "active".into(),
            created_at: "2024-06-15".into(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: UserInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, original.id);
        assert_eq!(restored.username, original.username);
        assert_eq!(restored.email, original.email);
        assert_eq!(restored.role, original.role);
    }
}
