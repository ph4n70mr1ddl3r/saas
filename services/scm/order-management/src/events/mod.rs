use saas_nats_bus::NatsBus;
use saas_proto::events::OrderFulfilled;
use sqlx::SqlitePool;

pub async fn register(bus: &NatsBus, _pool: SqlitePool) -> anyhow::Result<()> {
    // Order Fulfilled -> log for inventory deduction
    bus.subscribe::<OrderFulfilled, _, _>("scm.orders.order.fulfilled", move |envelope| {
        let order_id = envelope.payload.order_id.clone();
        let line_count = envelope.payload.lines.len();
        async move {
            tracing::info!(
                "Order fulfilled: order={}, {} lines - inventory should be deducted",
                order_id, line_count
            );
        }
    }).await.ok();

    Ok(())
}
