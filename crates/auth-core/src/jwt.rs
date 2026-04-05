use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use chrono::Utc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub roles: Vec<String>,
    pub exp: u64,
    pub iat: u64,
}

pub fn encode_token(user_id: &str, username: &str, roles: Vec<String>, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        roles,
        iat: now.timestamp() as u64,
        exp: (now + chrono::Duration::hours(24)).timestamp() as u64,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
}

pub fn decode_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::default())?;
    Ok(token_data.claims)
}

/// Read JWT_SECRET from environment, panicking at startup if not set.
/// This should be called once during service initialization.
pub fn read_jwt_secret() -> String {
    std::env::var("JWT_SECRET")
        .expect("FATAL: JWT_SECRET environment variable must be set")
}
