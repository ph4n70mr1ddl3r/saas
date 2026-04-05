use crate::service::InventoryService;
use saas_nats_bus::NatsBus;
use saas_proto::events::{PurchaseOrderReceived, SalesOrderConfirmed};

pub async fn register(bus: &NatsBus, service: InventoryService) -> anyhow::Result<()> {
    let svc = service.clone();
    bus.subscribe::<PurchaseOrderReceived, _, _>("scm.procurement.po.received", move |envelope| {
        let svc = svc.clone();
        async move {
            for line in &envelope.payload.lines {
                tracing::info!(
                    "Processing PO received: po_id={}, item={}, warehouse={}, qty={}",
                    envelope.payload.po_id, line.item_id, line.warehouse_id, line.quantity_received
                );
                if let Err(e) = svc.handle_po_received(
                    &envelope.payload.po_id,
                    &line.item_id,
                    &line.warehouse_id,
                    line.quantity_received,
                ).await {
                    tracing::error!("Failed to handle PO received for item {}: {}", line.item_id, e);
                }
            }
        }
    }).await?;

    let svc = service.clone();
    bus.subscribe::<SalesOrderConfirmed, _, _>("scm.orders.order.confirmed", move |envelope| {
        let svc = svc.clone();
        async move {
            for line in &envelope.payload.lines {
                if let Some(ref warehouse_id) = line.warehouse_id {
                    tracing::info!(
                        "Processing order confirmed: order_id={}, item={}, warehouse={}, qty={}",
                        envelope.payload.order_id, line.item_id, warehouse_id, line.quantity
                    );
                    if let Err(e) = svc.handle_order_confirmed(
                        &envelope.payload.order_id,
                        &line.item_id,
                        warehouse_id,
                        line.quantity,
                    ).await {
                        tracing::error!("Failed to handle order confirmed for item {}: {}", line.item_id, e);
                    }
                }
            }
        }
    }).await?;

    Ok(())
}
