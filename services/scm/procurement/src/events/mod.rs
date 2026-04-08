use crate::service::ProcurementService;
use saas_nats_bus::NatsBus;
use saas_proto::events::{ItemBelowReorderPoint, StockReceived};
use sqlx::SqlitePool;

pub async fn register(bus: &NatsBus, service: ProcurementService) -> anyhow::Result<()> {
    // Item below reorder point -> auto-create draft purchase order
    let svc = service.clone();
    bus.subscribe::<ItemBelowReorderPoint, _, _>("scm.inventory.item.below_reorder", move |envelope| {
        let svc = svc.clone();
        let item_id = envelope.payload.item_id.clone();
        let item_name = envelope.payload.item_name.clone();
        let suggested_qty = envelope.payload.suggested_order_quantity;
        async move {
            tracing::info!(
                "Item {} ({}) below reorder point (available: {}, reorder: {}), creating auto-PO for qty {}",
                item_id, item_name,
                envelope.payload.available_quantity,
                envelope.payload.reorder_point,
                suggested_qty
            );
            if let Err(e) = svc.handle_item_below_reorder(&item_id, &item_name, suggested_qty).await {
                tracing::error!("Failed to auto-create PO for item {}: {}", item_id, e);
            }
        }
    }).await.ok();

    // Stock received from inventory -> track PO fulfillment
    let svc = service.clone();
    bus.subscribe::<StockReceived, _, _>("scm.inventory.stock.received", move |envelope| {
        let svc = svc.clone();
        let item_id = envelope.payload.item_id.clone();
        let warehouse_id = envelope.payload.warehouse_id.clone();
        let quantity = envelope.payload.quantity;
        let reference_type = envelope.payload.reference_type.clone();
        let reference_id = envelope.payload.reference_id.clone();
        async move {
            tracing::info!(
                "Stock received event: item={}, warehouse={}, qty={}, ref_type={}, ref_id={}",
                item_id, warehouse_id, quantity, reference_type, reference_id
            );
            if let Err(e) = svc.handle_stock_received(&item_id, &warehouse_id, quantity, &reference_type, &reference_id).await {
                tracing::error!("Failed to process stock received for PO fulfillment: {}", e);
            }
        }
    }).await.ok();

    Ok(())
}
