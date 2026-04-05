use sqlx::SqlitePool;
use saas_nats_bus::NatsBus;
use saas_common::error::{AppError, AppResult};
use saas_common::pagination::PaginationParams;
use saas_common::response::ApiListResponse;
use saas_auth_core::rbac::is_admin;
use validator::Validate;
use crate::repository::user_repo::UserRepo;
use crate::models::user::{CreateUser, UpdateUser, ChangePassword, UserResponse};

#[derive(Clone)]
pub struct UserService {
    repo: UserRepo,
    bus: NatsBus,
}

impl UserService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self { repo: UserRepo::new(pool), bus }
    }

    pub async fn list(&self, pag: &PaginationParams) -> AppResult<ApiListResponse<UserResponse>> {
        let (users, total) = self.repo.list(pag).await?;
        Ok(ApiListResponse {
            data: users.into_iter().map(UserResponse::from).collect(),
            total,
            page: pag.page(),
            per_page: pag.per_page(),
        })
    }

    pub async fn get(&self, id: &str) -> AppResult<UserResponse> {
        let user = self.repo.get_by_id(id).await?;
        Ok(UserResponse::from(user))
    }

    pub async fn create(&self, input: CreateUser) -> AppResult<UserResponse> {
        input.validate().map_err(|e| AppError::Validation(e.to_string()))?;

        let salt = argon2::password_hash::SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
        let hash = argon2::PasswordHasher::hash_password(
            &argon2::Argon2::default(),
            input.password.as_bytes(),
            &salt,
        ).map_err(|e| AppError::Internal(e.to_string()))?;

        let user = self.repo.create(&input, &hash.to_string()).await?;
        Ok(UserResponse::from(user))
    }

    pub async fn update(&self, id: &str, input: UpdateUser) -> AppResult<UserResponse> {
        input.validate().map_err(|e| AppError::Validation(e.to_string()))?;
        let user = self.repo.update(
            id,
            input.email.as_deref(),
            input.display_name.as_deref(),
            input.is_active,
        ).await?;
        Ok(UserResponse::from(user))
    }

    pub async fn change_password(
        &self,
        actor_id: &str,
        actor_roles: &[String],
        target_id: &str,
        input: ChangePassword,
    ) -> AppResult<()> {
        input.validate().map_err(|e| AppError::Validation(e.to_string()))?;

        let admin = is_admin(actor_roles);

        // Only admins can change other users' passwords
        if actor_id != target_id && !admin {
            return Err(AppError::Forbidden("Cannot change another user's password".into()));
        }

        // Non-admins must verify their current password
        if !admin || actor_id == target_id {
            let user = self.repo.get_by_id(target_id).await?;
            let parsed_hash = argon2::PasswordHash::new(&user.password_hash)
                .map_err(|_| AppError::Internal("Password hash error".into()))?;
            if argon2::PasswordVerifier::verify_password(
                &argon2::Argon2::default(),
                input.current_password.as_bytes(),
                &parsed_hash,
            ).is_err() {
                return Err(AppError::Unauthorized);
            }
        }

        let salt = argon2::password_hash::SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
        let hash = argon2::PasswordHasher::hash_password(
            &argon2::Argon2::default(),
            input.new_password.as_bytes(),
            &salt,
        ).map_err(|e| AppError::Internal(e.to_string()))?;
        self.repo.update_password(target_id, &hash.to_string()).await
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        self.repo.soft_delete(id).await
    }

    pub async fn assign_roles(&self, user_id: &str, role_ids: Vec<String>) -> AppResult<()> {
        self.repo.set_user_roles(user_id, &role_ids).await
    }
}
