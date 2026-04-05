use sqlx::SqlitePool;
use saas_nats_bus::NatsBus;
use saas_common::error::AppResult;
use crate::models::ConfigEntry;
use crate::repository::ConfigRepo;

#[derive(Clone)]
pub struct ConfigService {
    repo: ConfigRepo,
    #[allow(dead_code)]
    bus: NatsBus,
}

impl ConfigService {
    pub fn new(pool: SqlitePool, bus: NatsBus) -> Self {
        Self { repo: ConfigRepo::new(pool), bus }
    }

    pub async fn list(&self) -> AppResult<Vec<ConfigEntry>> {
        self.repo.list().await
    }

    pub async fn get(&self, key: &str) -> AppResult<ConfigEntry> {
        self.repo.get(key).await
    }

    pub async fn set(&self, key: &str, value: &str) -> AppResult<ConfigEntry> {
        self.repo.set(key, value).await
    }
}
