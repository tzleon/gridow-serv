use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, StatusCode},
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::Deserialize;

use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub public_id: String,
}

#[derive(Debug, Deserialize)]
struct JwtClaims {
    public_id: String,
    #[allow(dead_code)]
    exp: usize,
}

fn verify_token(token: &str, secret: &str) -> Result<String, (StatusCode, &'static str)> {
    let token_data = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid token"))?;

    Ok(token_data.claims.public_id)
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization header"))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or((StatusCode::UNAUTHORIZED, "Invalid Authorization format"))?;

        let public_id = verify_token(token, &state.jwt_secret)?;

        Ok(AuthUser { public_id })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snowflake::Snowflake;
    use axum::http::Request;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde_json::json;

    const TEST_JWT_EXP: usize = 9999999999;

    fn create_test_token(public_id: &str, secret: &str) -> String {
        let payload = json!({
            "public_id": public_id,
            "exp": TEST_JWT_EXP,
        });
        encode(
            &Header::new(jsonwebtoken::Algorithm::HS256),
            &payload,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap()
    }

    fn make_test_state(jwt_secret: &str) -> AppState {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://test:test@localhost:5432/test")
            .expect("connect_lazy should not fail");
        AppState::new(
            pool,
            "/tmp".into(),
            jwt_secret.into(),
            "http://localhost".into(),
            Snowflake::new(1),
        )
    }

    #[test]
    fn test_verify_token_valid() {
        let secret = "test_secret_key_123";
        let token = create_test_token("user_abc", secret);
        let result = verify_token(&token, secret);
        assert_eq!(result.unwrap(), "user_abc");
    }

    #[test]
    fn test_verify_token_invalid_secret() {
        let token = create_test_token("user_abc", "correct_secret");
        let result = verify_token(&token, "wrong_secret");
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_token_malformed() {
        let result = verify_token("not.a.token", "secret");
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_token_empty_string() {
        let result = verify_token("", "secret");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_from_request_parts_valid_token() {
        let state = make_test_state("my_secret");
        let token = create_test_token("user_xyz", "my_secret");

        let req = Request::builder()
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(())
            .unwrap();
        let (mut parts, _) = req.into_parts();

        let auth_user = AuthUser::from_request_parts(&mut parts, &state).await;
        assert!(auth_user.is_ok());
        assert_eq!(auth_user.unwrap().public_id, "user_xyz");
    }

    #[tokio::test]
    async fn test_from_request_parts_missing_header() {
        let state = make_test_state("secret");
        let req = Request::builder().body(()).unwrap();
        let (mut parts, _) = req.into_parts();

        let result = AuthUser::from_request_parts(&mut parts, &state).await;
        assert!(result.is_err());
        let (status, msg) = result.unwrap_err();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(msg, "Missing Authorization header");
    }

    #[tokio::test]
    async fn test_from_request_parts_invalid_format() {
        let state = make_test_state("secret");
        let req = Request::builder()
            .header(header::AUTHORIZATION, "Basic dXNlcjpwYXNz")
            .body(())
            .unwrap();
        let (mut parts, _) = req.into_parts();

        let result = AuthUser::from_request_parts(&mut parts, &state).await;
        assert!(result.is_err());
        let (status, msg) = result.unwrap_err();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(msg, "Invalid Authorization format");
    }

    #[tokio::test]
    async fn test_from_request_parts_bad_token() {
        let state = make_test_state("correct_secret");
        let token = create_test_token("user_abc", "wrong_secret");

        let req = Request::builder()
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(())
            .unwrap();
        let (mut parts, _) = req.into_parts();

        let result = AuthUser::from_request_parts(&mut parts, &state).await;
        assert!(result.is_err());
        let (status, msg) = result.unwrap_err();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(msg, "Invalid token");
    }
}
