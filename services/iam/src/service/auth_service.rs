use crate::models::user::{LoginRequest, LoginResponse, UserResponse};
use crate::repository::token_repo::TokenRepo;
use crate::repository::user_repo::UserRepo;
use saas_auth_core::jwt;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AuthService {
    repo: UserRepo,
    token_repo: TokenRepo,
    bus: NatsBus,
}

impl AuthService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            repo: UserRepo::new(pool.clone()),
            token_repo: TokenRepo::new(pool),
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

    pub async fn logout(&self, user_id: &str, jti: &str, exp: u64) -> AppResult<()> {
        let expires_at = chrono::DateTime::from_timestamp(exp as i64, 0)
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

        self.token_repo.revoke_token(jti, user_id, &expires_at).await?;

        if let Err(e) = self
            .bus
            .publish(
                "iam.token.revoked",
                saas_proto::events::TokenRevoked {
                    jti: jti.to_string(),
                    user_id: user_id.to_string(),
                    expires_at: expires_at.clone(),
                },
            )
            .await
        {
            tracing::warn!("Failed to publish token revocation event: {}", e);
        }

        tracing::info!("Token {} revoked for user {}", jti, user_id);
        Ok(())
    }

    pub async fn is_token_revoked(&self, jti: &str) -> AppResult<bool> {
        self.token_repo.is_revoked(jti).await
    }

    pub async fn cleanup_expired_revocations(&self) -> AppResult<u64> {
        let removed = self.token_repo.cleanup_expired().await?;
        if removed > 0 {
            tracing::info!("Cleaned up {} expired token revocations", removed);
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::user_repo::UserRepo;
    use saas_db::test_helpers::create_test_pool;
    use sqlx::SqlitePool;

    async fn setup() -> SqlitePool {
        let pool = create_test_pool().await;
        let sql_files = [
            include_str!("../../migrations/001_create_users.sql"),
            include_str!("../../migrations/002_create_roles.sql"),
            include_str!("../../migrations/003_create_permissions.sql"),
            include_str!("../../migrations/004_create_user_roles.sql"),
            include_str!("../../migrations/005_create_role_permissions.sql"),
            include_str!("../../migrations/006_create_revoked_tokens.sql"),
        ];
        let migration_names = [
            "001_create_users.sql",
            "002_create_roles.sql",
            "003_create_permissions.sql",
            "004_create_user_roles.sql",
            "005_create_role_permissions.sql",
            "006_create_revoked_tokens.sql",
        ];
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS _migrations (filename TEXT PRIMARY KEY, applied_at TEXT NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();

        for (i, sql) in sql_files.iter().enumerate() {
            let name = migration_names[i];
            let already_applied: bool =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _migrations WHERE filename = ?")
                    .bind(name)
                    .fetch_one(&pool)
                    .await
                    .unwrap()
                    > 0;
            if !already_applied {
                let mut tx = pool.begin().await.unwrap();
                sqlx::raw_sql(sql).execute(&mut *tx).await.unwrap();
                let now = chrono::Utc::now().to_rfc3339();
                sqlx::query("INSERT INTO _migrations (filename, applied_at) VALUES (?, ?)")
                    .bind(name)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await
                    .unwrap();
                tx.commit().await.unwrap();
            }
        }
        pool
    }

    #[tokio::test]
    async fn test_token_revocation_store_and_check() {
        let pool = setup().await;
        let token_repo = TokenRepo::new(pool);

        let jti = uuid::Uuid::new_v4().to_string();
        let user_id = "user-123";
        let expires_at = (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339();

        // Not revoked initially
        let is_revoked = token_repo.is_revoked(&jti).await.unwrap();
        assert!(!is_revoked, "Token should not be revoked initially");

        // Revoke the token
        token_repo
            .revoke_token(&jti, user_id, &expires_at)
            .await
            .unwrap();

        // Now it should be revoked
        let is_revoked = token_repo.is_revoked(&jti).await.unwrap();
        assert!(is_revoked, "Token should be revoked after revocation");
    }

    #[tokio::test]
    async fn test_duplicate_revocation_is_idempotent() {
        let pool = setup().await;
        let token_repo = TokenRepo::new(pool);

        let jti = uuid::Uuid::new_v4().to_string();
        let expires_at = (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339();

        // Revoke twice — should not error
        token_repo
            .revoke_token(&jti, "user-1", &expires_at)
            .await
            .unwrap();
        token_repo
            .revoke_token(&jti, "user-1", &expires_at)
            .await
            .unwrap();

        let is_revoked = token_repo.is_revoked(&jti).await.unwrap();
        assert!(is_revoked, "Token should still be revoked");
    }

    #[tokio::test]
    async fn test_cleanup_expired_revocations() {
        let pool = setup().await;
        let token_repo = TokenRepo::new(pool);

        // Insert an already-expired token
        let expired_jti = uuid::Uuid::new_v4().to_string();
        let past_expiry = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        token_repo
            .revoke_token(&expired_jti, "user-1", &past_expiry)
            .await
            .unwrap();

        // Insert a valid (future) token
        let valid_jti = uuid::Uuid::new_v4().to_string();
        let future_expiry = (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339();
        token_repo
            .revoke_token(&valid_jti, "user-2", &future_expiry)
            .await
            .unwrap();

        // Cleanup
        let removed = token_repo.cleanup_expired().await.unwrap();
        assert_eq!(removed, 1, "Should have removed 1 expired token");

        // Expired is gone, valid remains
        assert!(
            !token_repo.is_revoked(&expired_jti).await.unwrap(),
            "Expired token should be cleaned up"
        );
        assert!(
            token_repo.is_revoked(&valid_jti).await.unwrap(),
            "Valid token should remain"
        );
    }

    #[tokio::test]
    async fn test_jwt_contains_jti_claim() {
        std::env::set_var("JWT_SECRET", "test-secret-that-is-at-least-32-characters-long!!");
        saas_auth_core::jwt::init_jwt_secret();

        let secret = saas_auth_core::jwt::read_jwt_secret();
        let token = saas_auth_core::jwt::encode_token("user-1", "testuser", vec!["Admin".into()], secret).unwrap();

        let claims = saas_auth_core::jwt::decode_token(&token, secret).unwrap();
        assert!(claims.jti.is_some(), "Token should contain a jti claim");
        assert!(!claims.jti.unwrap().is_empty(), "jti should not be empty");
    }

    #[tokio::test]
    async fn test_multiple_tokens_independent_revocation() {
        let pool = setup().await;
        let token_repo = TokenRepo::new(pool);

        let jti1 = uuid::Uuid::new_v4().to_string();
        let jti2 = uuid::Uuid::new_v4().to_string();
        let expires_at = (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339();

        // Revoke only token 1
        token_repo
            .revoke_token(&jti1, "user-1", &expires_at)
            .await
            .unwrap();

        assert!(token_repo.is_revoked(&jti1).await.unwrap(), "Token 1 should be revoked");
        assert!(!token_repo.is_revoked(&jti2).await.unwrap(), "Token 2 should NOT be revoked");
    }
}
