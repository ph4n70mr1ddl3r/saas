use crate::service::ManufacturingService;
use saas_nats_bus::NatsBus;
use saas_proto::events::SalesOrderConfirmed;
use sqlx::SqlitePool;

pub async fn register(bus: &NatsBus, service: ManufacturingService) -> anyhow::Result<()> {
    // Sales order confirmed -> auto-create work order if BOM exists for the item
    let svc = service.clone();
    bus.subscribe::<SalesOrderConfirmed, _, _>("scm.orders.order.confirmed", move |envelope| {
        let svc = svc.clone();
        let order_id = envelope.payload.order_id.clone();
        async move {
            for line in &envelope.payload.lines {
                tracing::info!(
                    "Checking if item {} from order {} needs manufacturing",
                    line.item_id, order_id
                );
                match svc.handle_order_confirmed(&order_id, &line.item_id, line.quantity).await {
                    Ok(Some(wo)) => {
                        tracing::info!(
                            "Auto-created work order {} for item {} (qty {})",
                            wo.wo_number, wo.item_id, wo.quantity
                        );
                    }
                    Ok(None) => {
                        tracing::debug!("No BOM found for item {}, skipping work order creation", line.item_id);
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to auto-create work order for item {} from order {}: {}",
                            line.item_id, order_id, e
                        );
                    }
                }
            }
        }
    }).await.ok();

    Ok(())
}
