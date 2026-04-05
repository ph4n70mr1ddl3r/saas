use crate::models::role::{CreateRole, PermissionResponse, RoleResponse, UpdateRole};
use crate::repository::role_repo::RoleRepo;
use saas_common::error::{AppError, AppResult};
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;
use validator::Validate;

#[derive(Clone)]
pub struct RoleService {
    repo: RoleRepo,
    #[allow(dead_code)]
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
        self.repo
            .create_role(&input.name, input.description.as_deref())
            .await
    }

    pub async fn update(&self, id: &str, input: UpdateRole) -> AppResult<RoleResponse> {
        input
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;
        self.repo
            .update_role(id, input.name.as_deref(), input.description.as_deref())
            .await
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
            .await
    }
}
