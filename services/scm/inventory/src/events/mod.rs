use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;

pub async fn register(_bus: &NatsBus, _pool: SqlitePool) -> anyhow::Result<()> {
    // Subscribe to scm.procurement.po.received (to update stock)
    // Subscribe to scm.orders.order.confirmed (to reserve)
    Ok(())
}
