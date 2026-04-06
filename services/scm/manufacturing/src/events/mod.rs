use saas_nats_bus::NatsBus;
use saas_proto::events::WorkOrderCompleted;
use sqlx::SqlitePool;

pub async fn register(bus: &NatsBus, _pool: SqlitePool) -> anyhow::Result<()> {
    // Work Order Completed -> log for inventory integration
    bus.subscribe::<WorkOrderCompleted, _, _>("scm.manufacturing.work_order.completed", move |envelope| {
        let item_id = envelope.payload.item_id.clone();
        let quantity = envelope.payload.quantity;
        let wo_id = envelope.payload.work_order_id.clone();
        async move {
            tracing::info!(
                "Work order completed: wo={}, item={}, qty={} - inventory should be updated",
                wo_id, item_id, quantity
            );
        }
    }).await.ok();

    Ok(())
}
