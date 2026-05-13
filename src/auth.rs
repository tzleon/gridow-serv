use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, StatusCode},
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::Deserialize;

use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: String,
}

#[derive(Debug, Clone)]
pub struct OptionalAuthUser {
    pub user_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JwtClaims {
    user_id: String,
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

    Ok(token_data.claims.user_id)
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

        let user_id = verify_token(token, &state.jwt_secret)?;

        Ok(AuthUser { user_id })
    }
}

impl FromRequestParts<AppState> for OptionalAuthUser {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok());

        let user_id = match auth_header {
            Some(header_value) => {
                match header_value.strip_prefix("Bearer ") {
                    Some(token) => verify_token(token, &state.jwt_secret).ok(),
                    None => None,
                }
            }
            None => None,
        };

        Ok(OptionalAuthUser { user_id })
    }
}
