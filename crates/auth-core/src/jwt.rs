use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

static JWT_SECRET: OnceLock<String> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub roles: Vec<String>,
    pub exp: u64,
    pub iat: u64,
}

pub fn encode_token(
    user_id: &str,
    username: &str,
    roles: Vec<String>,
    secret: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        roles,
        iat: now.timestamp() as u64,
        exp: (now + chrono::Duration::hours(24)).timestamp() as u64,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

pub fn decode_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 60;
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )?;
    Ok(token_data.claims)
}

/// Initialize the JWT secret from the environment. Call once at startup.
/// Panics if JWT_SECRET is not set.
pub fn init_jwt_secret() {
    let secret =
        std::env::var("JWT_SECRET").expect("FATAL: JWT_SECRET environment variable must be set");
    if secret.len() < 32 {
        panic!(
            "FATAL: JWT_SECRET must be at least 32 characters, got {}",
            secret.len()
        );
    }
    JWT_SECRET
        .set(secret)
        .expect("JWT_SECRET already initialized");
}

/// Read the cached JWT secret. Must call `init_jwt_secret()` first.
pub fn read_jwt_secret() -> &'static str {
    JWT_SECRET
        .get()
        .expect("JWT_SECRET not initialized -- call init_jwt_secret() at startup")
}
