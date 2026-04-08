use crate::service::OrderManagementService;
use saas_nats_bus::NatsBus;
use saas_proto::events::WorkOrderCompleted;

pub async fn register(bus: &NatsBus, service: OrderManagementService) -> anyhow::Result<()> {
    let svc = service.clone();
    bus.subscribe::<WorkOrderCompleted, _, _>("scm.manufacturing.work_order.completed", move |envelope| {
        let svc = svc.clone();
        let work_order_id = envelope.payload.work_order_id.clone();
        let item_id = envelope.payload.item_id.clone();
        let quantity = envelope.payload.quantity;
        async move {
            tracing::info!(
                "Work order {} completed: item {} qty {} now available for fulfillment",
                work_order_id, item_id, quantity
            );

            // Check confirmed/picking sales orders for lines with this item
            match svc.list_sales_orders().await {
                Ok(orders) => {
                    for order in &orders {
                        if order.status != "confirmed" && order.status != "picking" {
                            continue;
                        }
                        if let Ok(detail) = svc.get_sales_order(&order.id).await {
                            for line in &detail.lines {
                                if line.item_id == item_id {
                                    tracing::info!(
                                        "Order {} (status: {}) has pending line for item {} (qty: {}) - now fulfillable",
                                        order.order_number, order.status, item_id, line.quantity
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to list sales orders for fulfillment check: {}", e);
                }
            }
        }
    }).await.ok();

    Ok(())
}
