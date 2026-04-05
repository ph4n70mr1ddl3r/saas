use crate::jwt::decode_token;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

pub struct AuthUser {
    pub user_id: String,
    pub username: String,
    pub roles: Vec<String>,
}

#[derive(Debug)]
pub enum AuthError {
    MissingToken,
    InvalidToken,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let message = match self {
            AuthError::MissingToken => "Missing authorization token",
            AuthError::InvalidToken => "Invalid or expired token",
        };
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": { "code": "UNAUTHORIZED", "message": message } })),
        )
            .into_response()
    }
}

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::MissingToken)?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(AuthError::InvalidToken)?;

        let token = token.trim();
        if token.is_empty() {
            return Err(AuthError::InvalidToken);
        }

        let secret = crate::jwt::read_jwt_secret();

        let claims = decode_token(token, &secret).map_err(|_| AuthError::InvalidToken)?;

        Ok(AuthUser {
            user_id: claims.sub,
            username: claims.username,
            roles: claims.roles,
        })
    }
}
