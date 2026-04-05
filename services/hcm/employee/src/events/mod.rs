// Event subscriber registration - subscribes to cross-service events
use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;

pub async fn register(_bus: &NatsBus, _pool: SqlitePool) -> anyhow::Result<()> {
    // Subscribe to recruiting.application.status_changed
    // When an application status changes to "hired", create a new employee
    Ok(())
}
