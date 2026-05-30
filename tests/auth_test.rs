use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Serialize, Deserialize)]
struct TestClaims {
    public_id: String,
    exp: usize,
}

#[test]
fn test_jwt_roundtrip() {
    let secret = "test_jwt_secret_for_integration_test";

    let claims = TestClaims {
        public_id: "user_abc".into(),
        exp: 9999999999,
    };

    let token = encode(
        &Header::new(jsonwebtoken::Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .expect("Failed to create token");

    let decoded = decode::<TestClaims>(
        &token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .expect("Failed to decode token");

    assert_eq!(decoded.claims.public_id, "user_abc");
}

#[test]
fn test_jwt_with_wrong_secret_fails() {
    let token = encode(
        &Header::new(jsonwebtoken::Algorithm::HS256),
        &json!({"public_id": "u1", "exp": 9999999999u64}),
        &EncodingKey::from_secret(b"correct_secret"),
    )
    .expect("Failed to create token");

    let result = decode::<serde_json::Value>(
        &token,
        &DecodingKey::from_secret(b"wrong_secret"),
        &Validation::default(),
    );
    assert!(result.is_err());
}

#[test]
fn test_jwt_with_malformed_token_fails() {
    let result = decode::<serde_json::Value>(
        "not.a.valid.jwt",
        &DecodingKey::from_secret(b"secret"),
        &Validation::default(),
    );
    assert!(result.is_err());
}

#[test]
fn test_jwt_expired_token_fails() {
    let token = encode(
        &Header::new(jsonwebtoken::Algorithm::HS256),
        &json!({"public_id": "u1", "exp": 1000000}),
        &EncodingKey::from_secret(b"secret"),
    )
    .expect("Failed to create token");

    let result = decode::<serde_json::Value>(
        &token,
        &DecodingKey::from_secret(b"secret"),
        &Validation::default(),
    );
    assert!(result.is_err());
}
