use saas_nats_bus::NatsBus;
use sqlx::SqlitePool;

// Order fulfillment events are now consumed by the Inventory service directly
// via scm.orders.order.fulfilled subscription for stock deduction.

pub async fn register(_bus: &NatsBus, _pool: SqlitePool) -> anyhow::Result<()> {
    Ok(())
}
