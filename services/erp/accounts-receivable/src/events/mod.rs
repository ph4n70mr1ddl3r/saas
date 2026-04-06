use crate::routes::AppState;
use saas_common::error::AppResult;
use saas_nats_bus::NatsBus;
use saas_proto::events::OrderFulfilled;

pub async fn register(bus: &NatsBus, _state: &AppState) -> AppResult<()> {
    // Order Fulfilled -> log for auto-invoice creation
    bus.subscribe::<OrderFulfilled, _, _>("scm.orders.order.fulfilled", move |envelope| {
        let order_id = envelope.payload.order_id.clone();
        let customer_id = envelope.payload.customer_id.clone();
        let line_count = envelope.payload.lines.len();
        async move {
            tracing::info!(
                "Order fulfilled: order={}, customer={}, {} lines - AR invoice should be created",
                order_id, customer_id, line_count
            );
        }
    }).await.ok();

    tracing::info!("Accounts Receivable event subscribers registered");
    Ok(())
}
