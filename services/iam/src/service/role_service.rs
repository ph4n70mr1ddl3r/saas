use crate::models::role::{CreateRole, PermissionResponse, RoleResponse, UpdateRole};
use crate::repository::role_repo::RoleRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use saas_proto::events::{RoleCreated, RoleUpdated, RolePermissionsChanged};
use sqlx::SqlitePool;
use validator::Validate;

#[derive(Clone)]
pub struct RoleService {
    repo: RoleRepo,
    bus: NatsBus,
}

impl RoleService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            repo: RoleRepo::new(pool),
            bus,
        }
    }

    pub async fn list(&self) -> AppResult<Vec<RoleResponse>> {
        self.repo.list_roles().await
    }

    pub async fn get(&self, id: &str) -> AppResult<RoleResponse> {
        self.repo.get_role(id).await
    }

    pub async fn create(&self, input: CreateRole) -> AppResult<RoleResponse> {
        input
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;
        let role = self.repo
            .create_role(&input.name, input.description.as_deref())
            .await?;

        if let Err(e) = self
            .bus
            .publish(
                "iam.role.created",
                RoleCreated {
                    role_id: role.id.clone(),
                    name: role.name.clone(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "iam.role.created",
                e
            );
        }

        Ok(role)
    }

    pub async fn update(&self, id: &str, input: UpdateRole) -> AppResult<RoleResponse> {
        input
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;
        let role = self.repo
            .update_role(id, input.name.as_deref(), input.description.as_deref())
            .await?;

        if let Err(e) = self
            .bus
            .publish(
                "iam.role.updated",
                RoleUpdated {
                    role_id: role.id.clone(),
                    name: input.name,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "iam.role.updated",
                e
            );
        }

        Ok(role)
    }

    pub async fn list_permissions(&self) -> AppResult<Vec<PermissionResponse>> {
        self.repo.list_permissions().await
    }

    pub async fn set_permissions(
        &self,
        role_id: &str,
        permission_ids: Vec<String>,
    ) -> AppResult<()> {
        self.repo
            .set_role_permissions(role_id, &permission_ids)
            .await?;

        if let Err(e) = self
            .bus
            .publish(
                "iam.role.permissions.changed",
                RolePermissionsChanged {
                    role_id: role_id.to_string(),
                    permission_count: permission_ids.len() as u32,
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "iam.role.permissions.changed",
                e
            );
        }

        Ok(())
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        // Verify role exists
        self.repo.get_role(id).await?;
        self.repo.delete_role(id).await
    }
}
mod tests {
    use super::*;
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
    async fn test_role_crud() {
        let pool = setup().await;
        let repo = RoleRepo::new(pool);

        let role = repo
            .create_role("test_role", Some("A test role"))
            .await
            .unwrap();
        assert_eq!(role.name, "test_role");
        assert_eq!(role.description.as_deref(), Some("A test role"));

        let fetched = repo.get_role(&role.id).await.unwrap();
        assert_eq!(fetched.name, "test_role");

        let updated = repo
            .update_role(
                &role.id,
                Some("updated_role"),
                Some("Updated desc"),
            )
            .await
            .unwrap();
        assert_eq!(updated.name, "updated_role");
        assert_eq!(updated.description.as_deref(), Some("Updated desc"));

        let roles = repo.list_roles().await.unwrap();
        assert_eq!(roles.len(), 1);
    }

    #[tokio::test]
    async fn test_permissions_are_seeded() {
        let pool = setup().await;
        let repo = RoleRepo::new(pool);
        let perms = repo.list_permissions().await.unwrap();
        assert!(!perms.is_empty(), "Seed permissions should exist");
        let codes: Vec<&str> = perms.iter().map(|p| p.code.as_str()).collect();
        assert!(codes.contains(&"iam:user:read"));
        assert!(codes.contains(&"iam:role:write"));
    }

    #[tokio::test]
    async fn test_role_permission_assignment() {
        let pool = setup().await;
        let repo = RoleRepo::new(pool);
        let role = repo
            .create_role("perm_test_role", None)
            .await
            .unwrap();
        let perms = repo.list_permissions().await.unwrap();
        let perm_ids: Vec<String> = perms.iter().take(3).map(|p| p.id.clone()).collect();
        repo.set_role_permissions(&role.id, &perm_ids)
            .await
            .unwrap();
        let role_perms = repo.get_role_permissions(&role.id).await.unwrap();
        assert_eq!(role_perms.len(), 3);
    }

    #[tokio::test]
    async fn test_role_permission_replacement() {
        let pool = setup().await;
        let repo = RoleRepo::new(pool);
        let role = repo
            .create_role("replace_role", None)
            .await
            .unwrap();
        let perms = repo.list_permissions().await.unwrap();
        let first_three: Vec<String> = perms.iter().take(3).map(|p| p.id.clone()).collect();
        let next_two: Vec<String> = perms.iter().skip(3).take(2).map(|p| p.id.clone()).collect();
        repo.set_role_permissions(&role.id, &first_three)
            .await
            .unwrap();
        let role_perms = repo.get_role_permissions(&role.id).await.unwrap();
        assert_eq!(role_perms.len(), 3);
        repo.set_role_permissions(&role.id, &next_two)
            .await
            .unwrap();
        let role_perms = repo.get_role_permissions(&role.id).await.unwrap();
        assert_eq!(role_perms.len(), 2);
    }

    #[tokio::test]
    async fn test_role_not_found() {
        let pool = setup().await;
        let repo = RoleRepo::new(pool);
        let result = repo.get_role("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_role_create_validation_empty_name() {
        let pool = setup().await;
        let repo = RoleRepo::new(pool);

        // Attempt to create with empty name should be caught by service validation
        let input = CreateRole {
            name: String::new(),
            description: None,
        };
        input.validate()
            .map_err(|e| AppError::Validation(e.to_string()))
            .expect_err("Empty name should fail validation");
    }

    #[tokio::test]
    async fn test_multiple_roles() {
        let pool = setup().await;
        let repo = RoleRepo::new(pool);

        repo.create_role("role_a", None).await.unwrap();
        repo.create_role("role_b", Some("Second role")).await.unwrap();
        repo.create_role("role_c", None).await.unwrap();

        let roles = repo.list_roles().await.unwrap();
        assert_eq!(roles.len(), 3);
    }

    #[tokio::test]
    async fn test_role_permission_clear() {
        let pool = setup().await;
        let repo = RoleRepo::new(pool);
        let role = repo.create_role("clear_perm_role", None).await.unwrap();
        let perms = repo.list_permissions().await.unwrap();
        let perm_ids: Vec<String> = perms.iter().take(2).map(|p| p.id.clone()).collect();

        repo.set_role_permissions(&role.id, &perm_ids).await.unwrap();
        assert_eq!(repo.get_role_permissions(&role.id).await.unwrap().len(), 2);

        // Clear all permissions
        repo.set_role_permissions(&role.id, &[]).await.unwrap();
        assert_eq!(repo.get_role_permissions(&role.id).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_role_delete() {
        let pool = setup().await;
        let repo = RoleRepo::new(pool);

        let role = repo.create_role("delete_me", Some("To be deleted")).await.unwrap();
        assert_eq!(role.name, "delete_me");

        let perms = repo.list_permissions().await.unwrap();
        let perm_ids: Vec<String> = perms.iter().take(2).map(|p| p.id.clone()).collect();
        repo.set_role_permissions(&role.id, &perm_ids).await.unwrap();

        // Delete should succeed
        repo.delete_role(&role.id).await.unwrap();

        // Verify role is gone
        let result = repo.get_role(&role.id).await;
        assert!(result.is_err());

        // Verify not in list
        let roles = repo.list_roles().await.unwrap();
        assert!(roles.iter().all(|r| r.id != role.id));
    }

    #[tokio::test]
    async fn test_role_delete_not_found() {
        let pool = setup().await;
        let repo = RoleRepo::new(pool);
        let result = repo.delete_role("nonexistent").await;
        assert!(result.is_err());
    }
}
