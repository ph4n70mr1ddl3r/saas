use crate::routes::AppState;
use saas_common::error::AppResult;
use saas_nats_bus::NatsBus;
use saas_proto::events::PurchaseOrderReceived;

pub async fn register(bus: &NatsBus, _state: &AppState) -> AppResult<()> {
    // PO Received -> log for auto-invoice creation (three-way match)
    bus.subscribe::<PurchaseOrderReceived, _, _>("scm.procurement.po.received", move |envelope| {
        let po_id = envelope.payload.po_id.clone();
        let supplier_id = envelope.payload.supplier_id.clone();
        let line_count = envelope.payload.lines.len();
        async move {
            tracing::info!(
                "PO received: po={}, supplier={}, {} lines - auto-invoice should be created",
                po_id, supplier_id, line_count
            );
        }
    }).await.ok();

    tracing::info!("Accounts Payable event subscribers registered");
    Ok(())
}
