// Event subscriber registration - subscribes to IAM's own events for audit logging
use crate::service::{AuthService, RoleService};
use saas_nats_bus::NatsBus;
use saas_proto::events::{
    RoleCreated, RoleDeleted, RolePermissionsChanged, RoleUpdated, TokenRevoked,
};

pub async fn register(
    bus: &NatsBus,
    auth_service: AuthService,
    role_service: RoleService,
) -> anyhow::Result<()> {
    // Token revoked -> audit log
    let auth_svc = auth_service.clone();
    bus.subscribe::<TokenRevoked, _, _>("iam.token.revoked", move |envelope| {
        let auth_svc = auth_svc.clone();
        let jti = envelope.payload.jti.clone();
        let user_id = envelope.payload.user_id.clone();
        let expires_at = envelope.payload.expires_at.clone();
        async move {
            auth_svc.handle_token_revoked(&jti, &user_id, &expires_at);
        }
    })
    .await?;

    // Role created -> audit log
    let role_svc = role_service.clone();
    bus.subscribe::<RoleCreated, _, _>("iam.role.created", move |envelope| {
        let role_svc = role_svc.clone();
        let role_id = envelope.payload.role_id.clone();
        let name = envelope.payload.name.clone();
        async move {
            role_svc.handle_role_created(&role_id, &name);
        }
    })
    .await?;

    // Role updated -> audit log
    let role_svc = role_service.clone();
    bus.subscribe::<RoleUpdated, _, _>("iam.role.updated", move |envelope| {
        let role_svc = role_svc.clone();
        let role_id = envelope.payload.role_id.clone();
        let name = envelope.payload.name.clone();
        async move {
            role_svc.handle_role_updated(&role_id, &name);
        }
    })
    .await?;

    // Role permissions changed -> audit log + cache invalidation warning
    let role_svc = role_service.clone();
    bus.subscribe::<RolePermissionsChanged, _, _>("iam.role.permissions.changed", move |envelope| {
        let role_svc = role_svc.clone();
        let role_id = envelope.payload.role_id.clone();
        let permission_count = envelope.payload.permission_count;
        async move {
            role_svc.handle_role_permissions_changed(&role_id, permission_count);
        }
    })
    .await?;

    // Role deleted -> audit log
    let role_svc = role_service.clone();
    bus.subscribe::<RoleDeleted, _, _>("iam.role.deleted", move |envelope| {
        let role_svc = role_svc.clone();
        let role_id = envelope.payload.role_id.clone();
        let name = envelope.payload.name.clone();
        async move {
            role_svc.handle_role_deleted(&role_id, &name);
        }
    })
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::service::{AuthService, RoleService};
    use saas_db::test_helpers::create_test_pool;
    use saas_nats_bus::NatsBus;

    async fn create_services() -> (NatsBus, AuthService, RoleService) {
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

        let bus = NatsBus::connect("nats://localhost:4222", "saas-iam-test")
            .await
            .expect("NATS must be available for event registration tests");

        let auth_service = AuthService::new(pool.clone(), bus.clone());
        let role_service = RoleService::new(pool, bus.clone());

        (bus, auth_service, role_service)
    }

    #[tokio::test]
    async fn test_event_registration_succeeds() {
        let (bus, auth_service, role_service) = create_services().await;
        let result = super::register(&bus, auth_service, role_service).await;
        assert!(
            result.is_ok(),
            "Event registration should succeed when NATS is available"
        );
    }

    #[tokio::test]
    async fn test_auth_service_handle_token_revoked() {
        let pool = create_test_pool().await;
        let bus = NatsBus::connect("nats://localhost:4222", "saas-iam-test")
            .await
            .expect("NATS required");
        let auth_service = AuthService::new(pool, bus);
        // Should not panic
        auth_service.handle_token_revoked("test-jti", "user-123", "2025-01-01T00:00:00Z");
    }

    #[tokio::test]
    async fn test_role_service_handle_role_created() {
        let pool = create_test_pool().await;
        let bus = NatsBus::connect("nats://localhost:4222", "saas-iam-test")
            .await
            .expect("NATS required");
        let role_service = RoleService::new(pool, bus);
        role_service.handle_role_created("role-1", "Admin");
    }

    #[tokio::test]
    async fn test_role_service_handle_role_updated() {
        let pool = create_test_pool().await;
        let bus = NatsBus::connect("nats://localhost:4222", "saas-iam-test")
            .await
            .expect("NATS required");
        let role_service = RoleService::new(pool, bus);
        role_service.handle_role_updated("role-1", &Some("NewName".to_string()));
        role_service.handle_role_updated("role-1", &None);
    }

    #[tokio::test]
    async fn test_role_service_handle_role_permissions_changed() {
        let pool = create_test_pool().await;
        let bus = NatsBus::connect("nats://localhost:4222", "saas-iam-test")
            .await
            .expect("NATS required");
        let role_service = RoleService::new(pool, bus);
        role_service.handle_role_permissions_changed("role-1", 5);
    }

    #[tokio::test]
    async fn test_role_service_handle_role_deleted() {
        let pool = create_test_pool().await;
        let bus = NatsBus::connect("nats://localhost:4222", "saas-iam-test")
            .await
            .expect("NATS required");
        let role_service = RoleService::new(pool, bus);
        role_service.handle_role_deleted("role-1", "OldRole");
    }
}
