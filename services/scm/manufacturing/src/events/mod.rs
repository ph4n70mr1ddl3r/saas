use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;

pub async fn register(_bus: &NatsBus, _pool: SqlitePool) -> anyhow::Result<()> {
    Ok(())
}
