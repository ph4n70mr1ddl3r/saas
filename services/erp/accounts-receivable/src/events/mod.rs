use crate::routes::AppState;
use saas_common::error::AppResult;
use saas_nats_bus::NatsBus;
use saas_proto::events::OrderFulfilled;

pub async fn register(bus: &NatsBus, state: &AppState) -> AppResult<()> {
    // Order Fulfilled -> auto-create AR invoice
    let svc = state.service.clone();
    bus.subscribe::<OrderFulfilled, _, _>("scm.orders.order.fulfilled", move |envelope| {
        let svc = svc.clone();
        let order_id = envelope.payload.order_id.clone();
        let order_number = envelope.payload.order_number.clone();
        let customer_id = envelope.payload.customer_id.clone();
        let lines: Vec<(String, i64)> = envelope.payload.lines.iter()
            .map(|l| (l.item_id.clone(), l.quantity))
            .collect();
        let line_count = lines.len();
        async move {
            tracing::info!(
                "Order fulfilled: order={}, customer={}, {} lines - creating auto-invoice",
                order_id, customer_id, line_count
            );
            if let Err(e) = svc.handle_order_fulfilled(&order_id, &order_number, &customer_id, &lines).await {
                tracing::error!("Failed to create auto-invoice for order {}: {}", order_number, e);
            }
        }
    }).await.ok();

    tracing::info!("Accounts Receivable event subscribers registered");
    Ok(())
}
