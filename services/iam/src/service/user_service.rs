use crate::models::user::{ChangePassword, CreateUser, UpdateUser, UserResponse};
use crate::repository::user_repo::UserRepo;
use saas_auth_core::rbac::is_admin;
use saas_common::error::{AppError, AppResult};
use saas_common::pagination::PaginationParams;
use saas_common::response::ApiListResponse;
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;
use validator::Validate;

#[derive(Clone)]
pub struct UserService {
    repo: UserRepo,
    bus: NatsBus,
}

impl UserService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            repo: UserRepo::new(pool),
            bus,
        }
    }

    pub async fn list(&self, pag: &PaginationParams) -> AppResult<ApiListResponse<UserResponse>> {
        let (users, total) = self.repo.list_safe(pag).await?;
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
        input
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;

        let salt = argon2::password_hash::SaltString::generate(
            &mut argon2::password_hash::rand_core::OsRng,
        );
        let hash = argon2::PasswordHasher::hash_password(
            &argon2::Argon2::default(),
            input.password.as_bytes(),
            &salt,
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;

        let user = self.repo.create(&input, &hash.to_string()).await?;
        Ok(UserResponse::from(user))
    }

    pub async fn update(&self, id: &str, input: UpdateUser) -> AppResult<UserResponse> {
        input
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;
        let user = self
            .repo
            .update(
                id,
                input.email.as_deref(),
                input.display_name.as_deref(),
                input.is_active,
            )
            .await?;
        Ok(UserResponse::from(user))
    }

    pub async fn change_password(
        &self,
        actor_id: &str,
        actor_roles: &[String],
        target_id: &str,
        input: ChangePassword,
    ) -> AppResult<()> {
        input
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;

        let admin = is_admin(actor_roles);

        // Only admins can change other users' passwords
        if actor_id != target_id && !admin {
            return Err(AppError::Forbidden(
                "Cannot change another user's password".into(),
            ));
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
            )
            .is_err()
            {
                return Err(AppError::Unauthorized);
            }
        }

        let salt = argon2::password_hash::SaltString::generate(
            &mut argon2::password_hash::rand_core::OsRng,
        );
        let hash = argon2::PasswordHasher::hash_password(
            &argon2::Argon2::default(),
            input.new_password.as_bytes(),
            &salt,
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;
        self.repo
            .update_password(target_id, &hash.to_string())
            .await
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        self.repo.soft_delete(id).await
    }

    pub async fn assign_roles(&self, user_id: &str, role_ids: Vec<String>) -> AppResult<()> {
        self.repo.set_user_roles(user_id, &role_ids).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::role_repo::RoleRepo;
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
        ];
        let migration_names = [
            "001_create_users.sql",
            "002_create_roles.sql",
            "003_create_permissions.sql",
            "004_create_user_roles.sql",
            "005_create_role_permissions.sql",
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
    async fn test_user_creation() {
        let pool = setup().await;
        let repo = UserRepo::new(pool);

        let input = CreateUser {
            username: "testuser".into(),
            email: "test@example.com".into(),
            password: "securepassword123".into(),
            display_name: "Test User".into(),
        };

        // Hash password with argon2
        let salt =
            argon2::password_hash::SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
        let hash = argon2::PasswordHasher::hash_password(
            &argon2::Argon2::default(),
            input.password.as_bytes(),
            &salt,
        )
        .unwrap();

        let user = repo.create(&input, &hash.to_string()).await.unwrap();
        assert_eq!(user.username, "testuser");
        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.display_name, "Test User");
        assert!(user.is_active);
        assert!(!user.password_hash.is_empty());
    }

    #[tokio::test]
    async fn test_password_hashing_and_verification() {
        let password = "mypassword123";

        // Hash
        let salt =
            argon2::password_hash::SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
        let hash = argon2::PasswordHasher::hash_password(
            &argon2::Argon2::default(),
            password.as_bytes(),
            &salt,
        )
        .unwrap();
        let hash_str = hash.to_string();

        // Verify correct password
        let parsed_hash = argon2::PasswordHash::new(&hash_str).unwrap();
        let result = argon2::PasswordVerifier::verify_password(
            &argon2::Argon2::default(),
            password.as_bytes(),
            &parsed_hash,
        );
        assert!(result.is_ok());

        // Verify wrong password fails
        let wrong_result = argon2::PasswordVerifier::verify_password(
            &argon2::Argon2::default(),
            "wrongpassword".as_bytes(),
            &parsed_hash,
        );
        assert!(wrong_result.is_err());
    }

    #[tokio::test]
    async fn test_user_update() {
        let pool = setup().await;
        let repo = UserRepo::new(pool);

        let input = CreateUser {
            username: "updateuser".into(),
            email: "update@example.com".into(),
            password: "password123".into(),
            display_name: "Original Name".into(),
        };
        let salt =
            argon2::password_hash::SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
        let hash = argon2::PasswordHasher::hash_password(
            &argon2::Argon2::default(),
            input.password.as_bytes(),
            &salt,
        )
        .unwrap();

        let user = repo.create(&input, &hash.to_string()).await.unwrap();

        // Update email and display_name
        let updated = repo
            .update(
                &user.id,
                Some("newemail@example.com"),
                Some("Updated Name"),
                None,
            )
            .await
            .unwrap();
        assert_eq!(updated.email, "newemail@example.com");
        assert_eq!(updated.display_name, "Updated Name");
    }

    #[tokio::test]
    async fn test_user_soft_delete() {
        let pool = setup().await;
        let repo = UserRepo::new(pool);

        let input = CreateUser {
            username: "deleteuser".into(),
            email: "delete@example.com".into(),
            password: "password123".into(),
            display_name: "Delete Me".into(),
        };
        let salt =
            argon2::password_hash::SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
        let hash = argon2::PasswordHasher::hash_password(
            &argon2::Argon2::default(),
            input.password.as_bytes(),
            &salt,
        )
        .unwrap();

        let user = repo.create(&input, &hash.to_string()).await.unwrap();
        assert!(user.is_active);

        // Soft delete (deactivate)
        repo.soft_delete(&user.id).await.unwrap();
        let user = repo.get_by_id(&user.id).await.unwrap();
        assert!(!user.is_active);
    }

    #[tokio::test]
    async fn test_user_lookup_by_username() {
        let pool = setup().await;
        let repo = UserRepo::new(pool);

        let input = CreateUser {
            username: "findme".into(),
            email: "find@example.com".into(),
            password: "password123".into(),
            display_name: "Find Me".into(),
        };
        let salt =
            argon2::password_hash::SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
        let hash = argon2::PasswordHasher::hash_password(
            &argon2::Argon2::default(),
            input.password.as_bytes(),
            &salt,
        )
        .unwrap();

        repo.create(&input, &hash.to_string()).await.unwrap();

        let found = repo.get_by_username("findme").await.unwrap();
        assert_eq!(found.username, "findme");
        assert_eq!(found.email, "find@example.com");
    }

    #[tokio::test]
    async fn test_user_password_update() {
        let pool = setup().await;
        let repo = UserRepo::new(pool);

        let input = CreateUser {
            username: "pwchange".into(),
            email: "pwchange@example.com".into(),
            password: "oldpassword".into(),
            display_name: "PW Change".into(),
        };
        let salt =
            argon2::password_hash::SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
        let old_hash = argon2::PasswordHasher::hash_password(
            &argon2::Argon2::default(),
            input.password.as_bytes(),
            &salt,
        )
        .unwrap();

        let user = repo.create(&input, &old_hash.to_string()).await.unwrap();

        // New password hash
        let new_salt =
            argon2::password_hash::SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
        let new_hash = argon2::PasswordHasher::hash_password(
            &argon2::Argon2::default(),
            "newpassword".as_bytes(),
            &new_salt,
        )
        .unwrap();

        repo.update_password(&user.id, &new_hash.to_string())
            .await
            .unwrap();

        // Verify new password works
        let user = repo.get_by_id(&user.id).await.unwrap();
        let parsed = argon2::PasswordHash::new(&user.password_hash).unwrap();
        let verify = argon2::PasswordVerifier::verify_password(
            &argon2::Argon2::default(),
            "newpassword".as_bytes(),
            &parsed,
        );
        assert!(verify.is_ok());

        // Verify old password fails
        let old_verify = argon2::PasswordVerifier::verify_password(
            &argon2::Argon2::default(),
            "oldpassword".as_bytes(),
            &parsed,
        );
        assert!(old_verify.is_err());
    }

    #[tokio::test]
    async fn test_user_role_assignment() {
        let pool = setup().await;
        let user_repo = UserRepo::new(pool.clone());
        let role_repo = RoleRepo::new(pool);

        // Create user
        let input = CreateUser {
            username: "roleuser".into(),
            email: "roleuser@example.com".into(),
            password: "password123".into(),
            display_name: "Role User".into(),
        };
        let salt =
            argon2::password_hash::SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
        let hash = argon2::PasswordHasher::hash_password(
            &argon2::Argon2::default(),
            input.password.as_bytes(),
            &salt,
        )
        .unwrap();
        let user = user_repo.create(&input, &hash.to_string()).await.unwrap();

        // Create roles
        let admin_role = role_repo
            .create_role("admin", Some("Administrator"))
            .await
            .unwrap();
        let viewer_role = role_repo
            .create_role("viewer", Some("Read-only"))
            .await
            .unwrap();

        // Assign roles
        user_repo
            .set_user_roles(
                &user.id,
                &[admin_role.id.clone(), viewer_role.id.clone()],
            )
            .await
            .unwrap();

        // Verify roles
        let roles = user_repo.get_user_roles(&user.id).await.unwrap();
        assert_eq!(roles.len(), 2);
        assert!(roles.contains(&"admin".to_string()));
        assert!(roles.contains(&"viewer".to_string()));

        // Reassign to single role
        user_repo
            .set_user_roles(&user.id, &[viewer_role.id.clone()])
            .await
            .unwrap();

        let roles = user_repo.get_user_roles(&user.id).await.unwrap();
        assert_eq!(roles.len(), 1);
        assert!(roles.contains(&"viewer".to_string()));
    }

    #[tokio::test]
    async fn test_user_not_found() {
        let pool = setup().await;
        let repo = UserRepo::new(pool);
        let result = repo.get_by_id("nonexistent").await;
        assert!(result.is_err());
    }
}
