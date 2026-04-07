use crate::service::ProcurementService;
use saas_nats_bus::NatsBus;
use saas_proto::events::ItemBelowReorderPoint;
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

    Ok(())
}
