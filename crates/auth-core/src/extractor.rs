use crate::jwt::{decode_token, read_jwt_secret};
use crate::revocation::RevocationCache;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use std::sync::Arc;

/// Error type returned when authentication fails.
#[derive(Debug)]
pub enum AuthError {
    MissingHeader,
    InvalidToken(String),
    ExpiredToken,
    RevokedToken,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message): (StatusCode, String) = match self {
            AuthError::MissingHeader => {
                (StatusCode::UNAUTHORIZED, "Missing authorization header".into())
            }
            AuthError::InvalidToken(msg) => (StatusCode::UNAUTHORIZED, msg),
            AuthError::ExpiredToken => {
                (StatusCode::UNAUTHORIZED, "Token has expired".into())
            }
            AuthError::RevokedToken => {
                (StatusCode::UNAUTHORIZED, "Token has been revoked".into())
            }
        };
        (status, message).into_response()
    }
}

/// The authenticated user extracted from a JWT in the Authorization header.
#[derive(Debug, Clone, Serialize)]
pub struct AuthUser {
    pub user_id: String,
    pub username: String,
    pub roles: Vec<String>,
    #[serde(skip_serializing)]
    pub jti: Option<String>,
}

/// Global revocation cache, set once at service startup.
static REVOCATION_CACHE: std::sync::OnceLock<Arc<RevocationCache>> = std::sync::OnceLock::new();

/// Set the global revocation cache. Call once at service startup.
pub fn set_revocation_cache(cache: Arc<RevocationCache>) {
    let _ = REVOCATION_CACHE.set(cache);
}

/// Get a reference to the global revocation cache, if initialized.
pub fn get_revocation_cache() -> Option<&'static Arc<RevocationCache>> {
    REVOCATION_CACHE.get()
}

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = AuthError;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        async move {
            let header = auth_header.ok_or(AuthError::MissingHeader)?;

            let token = header
                .strip_prefix("Bearer ")
                .ok_or(AuthError::InvalidToken(
                    "Invalid authorization scheme".into(),
                ))?;

            let secret = read_jwt_secret();
            let claims = decode_token(token, secret)
                .map_err(|e| match e.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                        AuthError::ExpiredToken
                    }
                    _ => AuthError::InvalidToken(format!("Invalid token: {}", e)),
                })?;

            // Check revocation cache
            if let Some(ref jti) = claims.jti {
                if let Some(cache) = REVOCATION_CACHE.get() {
                    if cache.is_revoked(jti) {
                        return Err(AuthError::RevokedToken);
                    }
                }
            }

            Ok(AuthUser {
                user_id: claims.sub,
                username: claims.username,
                roles: claims.roles,
                jti: claims.jti,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jwt::{decode_token, encode_token, read_jwt_secret};

    const TEST_SECRET: &str = "test-secret-that-is-at-least-32-characters-long!";

    fn init_test_secret() {
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| {
            std::env::set_var("JWT_SECRET", TEST_SECRET);
            crate::jwt::init_jwt_secret();
        });
    }

    #[tokio::test]
    async fn test_encode_decode_roundtrip() {
        init_test_secret();
        let token = encode_token("user-1", "alice", vec!["admin".into()], TEST_SECRET).unwrap();
        let claims = decode_token(&token, read_jwt_secret()).unwrap();
        assert_eq!(claims.sub, "user-1");
        assert_eq!(claims.username, "alice");
        assert_eq!(claims.roles, vec!["admin".to_string()]);
    }

    #[tokio::test]
    async fn test_invalid_token_rejected() {
        init_test_secret();
        let result = decode_token("not.a.valid.token", read_jwt_secret());
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_revocation_cache() {
        let cache = RevocationCache::new();
        assert!(!cache.is_revoked("jti-123"));

        cache.revoke("jti-123".into());
        assert!(cache.is_revoked("jti-123"));
        assert!(!cache.is_revoked("jti-456"));
    }

    #[tokio::test]
    async fn test_revoked_token_detected() {
        init_test_secret();
        let cache = Arc::new(RevocationCache::new());

        let token = encode_token("user-2", "bob", vec!["viewer".into()], TEST_SECRET).unwrap();
        let claims = decode_token(&token, read_jwt_secret()).unwrap();

        if let Some(ref jti) = claims.jti {
            assert!(!cache.is_revoked(jti));
            cache.revoke(jti.clone());
            assert!(cache.is_revoked(jti));
        }
    }

    #[tokio::test]
    async fn test_auth_user_extraction() {
        init_test_secret();
        let token =
            encode_token("user-3", "charlie", vec!["erp_admin".into()], TEST_SECRET).unwrap();

        let req = axum::http::Request::builder()
            .header("Authorization", format!("Bearer {}", token))
            .body(())
            .unwrap();

        let (mut parts, _) = req.into_parts();
        let result = AuthUser::from_request_parts(&mut parts, &()).await;
        assert!(result.is_ok());
        let user = result.unwrap();
        assert_eq!(user.user_id, "user-3");
        assert_eq!(user.username, "charlie");
        assert_eq!(user.roles, vec!["erp_admin"]);
    }

    #[tokio::test]
    async fn test_missing_auth_header() {
        let req = axum::http::Request::builder().body(()).unwrap();
        let (mut parts, _) = req.into_parts();
        let result = AuthUser::from_request_parts(&mut parts, &()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_invalid_bearer_format() {
        let req = axum::http::Request::builder()
            .header("Authorization", "Basic abc123")
            .body(())
            .unwrap();
        let (mut parts, _) = req.into_parts();
        let result = AuthUser::from_request_parts(&mut parts, &()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_set_and_get_revocation_cache() {
        let cache = Arc::new(RevocationCache::new());
        set_revocation_cache(cache.clone());

        let retrieved = get_revocation_cache();
        assert!(retrieved.is_some());

        retrieved.unwrap().revoke("jti-test".into());
        assert!(retrieved.unwrap().is_revoked("jti-test"));
    }
}
