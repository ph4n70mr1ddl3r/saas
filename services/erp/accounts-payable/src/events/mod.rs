use crate::routes::AppState;
use saas_common::error::AppResult;
use saas_nats_bus::NatsBus;
use saas_proto::events::{PurchaseOrderReceived, StockReceived};

pub async fn register(bus: &NatsBus, state: &AppState) -> AppResult<()> {
    // PO Received -> auto-create AP invoice (three-way match)
    let svc = state.service.clone();
    bus.subscribe::<PurchaseOrderReceived, _, _>("scm.procurement.po.received", move |envelope| {
        let svc = svc.clone();
        let po_id = envelope.payload.po_id.clone();
        let supplier_id = envelope.payload.supplier_id.clone();
        let lines: Vec<(String, i64, i64)> = envelope.payload.lines.iter()
            .map(|l| (l.item_id.clone(), l.quantity_received, l.unit_price_cents))
            .collect();
        let line_count = lines.len();
        async move {
            tracing::info!(
                "PO received: po={}, supplier={}, {} lines - creating auto-invoice",
                po_id, supplier_id, line_count
            );
            if let Err(e) = svc.handle_po_received(&po_id, &supplier_id, &lines).await {
                tracing::error!("Failed to create auto-invoice for PO {}: {}", po_id, e);
            }
        }
    }).await.ok();

    // Stock Received -> match against pending PO invoices (three-way match tracking)
    let svc = state.service.clone();
    bus.subscribe::<StockReceived, _, _>("scm.inventory.stock.received", move |envelope| {
        let svc = svc.clone();
        let item_id = envelope.payload.item_id.clone();
        let quantity = envelope.payload.quantity;
        let reference_type = envelope.payload.reference_type.clone();
        let reference_id = envelope.payload.reference_id.clone();
        async move {
            if let Err(e) = svc.handle_stock_received(&item_id, quantity, &reference_type, &reference_id).await {
                tracing::error!(
                    "Failed to handle stock receipt for item {}: {}", item_id, e
                );
            }
        }
    }).await.ok();

    tracing::info!("Accounts Payable event subscribers registered");
    Ok(())
}
