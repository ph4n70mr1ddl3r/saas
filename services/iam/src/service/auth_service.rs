use crate::models::user::{LoginRequest, LoginResponse, UserResponse};
use crate::repository::user_repo::UserRepo;
use saas_auth_core::jwt;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AuthService {
    repo: UserRepo,
    bus: NatsBus,
}

impl AuthService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            repo: UserRepo::new(pool),
            bus,
        }
    }

    pub async fn login(&self, req: LoginRequest) -> AppResult<LoginResponse> {
        let user = match self.repo.get_by_username(&req.username).await {
            Ok(u) => u,
            Err(_) => {
                // Perform dummy hash verification to prevent timing attacks
                let dummy_hash = "$argon2id$v=19$m=19456,t=2,p=1$dummypass$dummypass";
                let _ = argon2::PasswordHash::new(dummy_hash).ok().map(|h| {
                    let _ = argon2::PasswordVerifier::verify_password(
                        &argon2::Argon2::default(),
                        req.password.as_bytes(),
                        &h,
                    );
                });
                return Err(AppError::Unauthorized);
            }
        };

        if !user.is_active {
            return Err(AppError::Unauthorized);
        }

        let parsed_hash = argon2::PasswordHash::new(&user.password_hash)
            .map_err(|_| AppError::Internal("Password hash error".into()))?;

        if argon2::PasswordVerifier::verify_password(
            &argon2::Argon2::default(),
            req.password.as_bytes(),
            &parsed_hash,
        )
        .is_err()
        {
            return Err(AppError::Unauthorized);
        }

        let roles = self.repo.get_user_roles(&user.id).await?;
        let secret = jwt::read_jwt_secret();
        let token = jwt::encode_token(&user.id, &user.username, roles.clone(), &secret)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(LoginResponse {
            access_token: token,
            token_type: "Bearer".to_string(),
            expires_in: 86400,
            user: UserResponse::from(user),
        })
    }

    pub async fn refresh(&self, user_id: &str) -> AppResult<LoginResponse> {
        let user = self.repo.get_by_id(user_id).await?;
        let roles = self.repo.get_user_roles(&user.id).await?;
        let secret = jwt::read_jwt_secret();
        let token = jwt::encode_token(&user.id, &user.username, roles, &secret)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        Ok(LoginResponse {
            access_token: token,
            token_type: "Bearer".to_string(),
            expires_in: 86400,
            user: UserResponse::from(user),
        })
    }

    pub async fn logout(&self, _user_id: &str) -> AppResult<()> {
        // TODO: Invalidate token via revocation list (e.g., Redis or NATS-backed store)
        Ok(())
    }
}
