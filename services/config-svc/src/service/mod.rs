use crate::models::ConfigEntry;
use crate::repository::ConfigRepo;
use saas_common::error::AppResult;
use saas_nats_bus::NatsBus;
use saas_proto::events::ConfigUpdated;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct ConfigService {
    repo: ConfigRepo,
    bus: NatsBus,
}

impl ConfigService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self {
            repo: ConfigRepo::new(pool),
            bus,
        }
    }

    pub async fn list(&self) -> AppResult<Vec<ConfigEntry>> {
        self.repo.list().await
    }

    pub async fn get(&self, key: &str) -> AppResult<ConfigEntry> {
        self.repo.get(key).await
    }

    pub async fn set(&self, key: &str, value: &str) -> AppResult<ConfigEntry> {
        let entry = self.repo.set(key, value).await?;
        if let Err(e) = self
            .bus
            .publish(
                "config.updated",
                ConfigUpdated {
                    key: key.to_string(),
                    value: value.to_string(),
                },
            )
            .await
        {
            tracing::error!(
                "CRITICAL: Failed to publish event '{}': {}. Data may be inconsistent.",
                "config.updated",
                e
            );
        }
        Ok(entry)
    }

    /// Handle config updated event: log the change for audit awareness.
    pub async fn handle_config_updated(&self, key: &str, value: &str) -> AppResult<()> {
        tracing::info!(
            "Config updated propagated: key='{}', value='{}'",
            key, value
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use saas_db::test_helpers::create_test_pool;
    use sqlx::SqlitePool;

    async fn setup() -> SqlitePool {
        let pool = create_test_pool().await;
        let sql_files = [
            include_str!("../../migrations/001_create_config.sql"),
        ];
        let migration_names = [
            "001_create_config.sql",
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

    async fn setup_repo() -> ConfigRepo {
        let pool = setup().await;
        ConfigRepo::new(pool)
    }

    #[tokio::test]
    async fn test_config_set_and_get() {
        let repo = setup_repo().await;

        let entry = repo.set("app.name", "My SaaS").await.unwrap();
        assert_eq!(entry.key, "app.name");
        assert_eq!(entry.value, "My SaaS");
        assert!(!entry.updated_at.is_empty());

        let fetched = repo.get("app.name").await.unwrap();
        assert_eq!(fetched.value, "My SaaS");
    }

    #[tokio::test]
    async fn test_config_upsert() {
        let repo = setup_repo().await;

        repo.set("app.version", "1.0.0").await.unwrap();
        let updated = repo.set("app.version", "2.0.0").await.unwrap();
        assert_eq!(updated.value, "2.0.0");

        let fetched = repo.get("app.version").await.unwrap();
        assert_eq!(fetched.value, "2.0.0");
    }

    #[tokio::test]
    async fn test_config_list() {
        let repo = setup_repo().await;

        repo.set("a.key", "value-a").await.unwrap();
        repo.set("b.key", "value-b").await.unwrap();
        repo.set("c.key", "value-c").await.unwrap();

        let list = repo.list().await.unwrap();
        assert_eq!(list.len(), 3);
        // Should be ordered by key
        assert_eq!(list[0].key, "a.key");
        assert_eq!(list[1].key, "b.key");
        assert_eq!(list[2].key, "c.key");
    }

    #[tokio::test]
    async fn test_config_get_not_found() {
        let repo = setup_repo().await;
        let result = repo.get("nonexistent.key").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_config_empty_list() {
        let repo = setup_repo().await;
        let list = repo.list().await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_config_special_characters() {
        let repo = setup_repo().await;

        let entry = repo
            .set("app.features", r#"{"dark_mode":true,"beta":false}"#)
            .await
            .unwrap();
        assert_eq!(entry.value, r#"{"dark_mode":true,"beta":false}"#);

        let fetched = repo.get("app.features").await.unwrap();
        assert_eq!(fetched.value, r#"{"dark_mode":true,"beta":false}"#);
    }

    #[tokio::test]
    async fn test_handle_config_updated() {
        let pool = setup().await;
        let svc = ConfigService {
            repo: ConfigRepo::new(pool),
            bus: saas_nats_bus::NatsBus::connect("nats://localhost:4222", "test")
                .await
                .unwrap(),
        };

        // Handler should succeed and log
        let result = svc.handle_config_updated("app.name", "My Updated SaaS").await;
        assert!(result.is_ok());
    }
}
