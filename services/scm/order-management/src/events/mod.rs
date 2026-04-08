use crate::service::OrderManagementService;
use saas_nats_bus::NatsBus;
use saas_proto::events::{StockReserved, WorkOrderCompleted};

pub async fn register(bus: &NatsBus, service: OrderManagementService) -> anyhow::Result<()> {
    let svc = service.clone();
    bus.subscribe::<WorkOrderCompleted, _, _>("scm.manufacturing.work_order.completed", move |envelope| {
        let svc = svc.clone();
        let work_order_id = envelope.payload.work_order_id.clone();
        let item_id = envelope.payload.item_id.clone();
        let quantity = envelope.payload.quantity;
        async move {
            svc.handle_work_order_completed(&work_order_id, &item_id, quantity).await;
        }
    }).await.ok();

    // Subscribe to stock reserved events to auto-advance sales orders
    let svc = service.clone();
    bus.subscribe::<StockReserved, _, _>("scm.inventory.stock.reserved", move |envelope| {
        let svc = svc.clone();
        let item_id = envelope.payload.item_id.clone();
        let warehouse_id = envelope.payload.warehouse_id.clone();
        let quantity = envelope.payload.quantity;
        let reference_type = envelope.payload.reference_type.clone();
        let reference_id = envelope.payload.reference_id.clone();
        async move {
            svc.handle_stock_reserved(&item_id, &warehouse_id, quantity, &reference_type, &reference_id).await;
        }
    }).await.ok();

    Ok(())
}
