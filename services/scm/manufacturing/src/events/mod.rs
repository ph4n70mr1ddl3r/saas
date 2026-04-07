use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;

// Work order completion events are now consumed by the Inventory service directly
// via scm.manufacturing.work_order.completed subscription for stock addition.

pub async fn register(_bus: &NatsBus, _pool: SqlitePool) -> anyhow::Result<()> {
    Ok(())
}
