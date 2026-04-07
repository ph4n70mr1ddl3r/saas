use crate::service::OrderManagementService;
use saas_nats_bus::NatsBus;
use saas_proto::events::WorkOrderCompleted;
use sqlx::SqlitePool;

pub async fn register(bus: &NatsBus, service: OrderManagementService) -> anyhow::Result<()> {
    // Work order completed -> log for order fulfillment tracking
    // When a work order for an item completes, it means manufactured items are now available.
    // The inventory service handles stock addition; here we log for visibility.
    let svc = service.clone();
    bus.subscribe::<WorkOrderCompleted, _, _>("scm.manufacturing.work_order.completed", move |envelope| {
        let _svc = svc.clone();
        let work_order_id = envelope.payload.work_order_id.clone();
        let item_id = envelope.payload.item_id.clone();
        let quantity = envelope.payload.quantity;
        async move {
            tracing::info!(
                "Work order {} completed: item {} qty {} now available for fulfillment",
                work_order_id, item_id, quantity
            );
            // Future: could check pending sales orders for this item and update status
        }
    }).await.ok();

    Ok(())
}
