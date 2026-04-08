use crate::service::InventoryService;
use saas_nats_bus::NatsBus;
use saas_proto::events::{
    OrderFulfilled, OrderFulfilledLine, PurchaseOrderCancelled, PurchaseOrderReceived, ReturnCreated,
    SalesOrderCancelled, SalesOrderConfirmed, WorkOrderCompleted, WorkOrderCancelled, WorkOrderStarted,
};

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

    // Order fulfilled -> deduct stock from inventory
    let svc = service.clone();
    bus.subscribe::<OrderFulfilled, _, _>("scm.orders.order.fulfilled", move |envelope| {
        let svc = svc.clone();
        async move {
            for line in &envelope.payload.lines {
                tracing::info!(
                    "Processing order fulfilled: order_id={}, item={}, warehouse={}, qty={}",
                    envelope.payload.order_id, line.item_id, line.warehouse_id, line.quantity
                );
                if let Err(e) = svc.handle_order_fulfilled(
                    &envelope.payload.order_id,
                    &line.item_id,
                    &line.warehouse_id,
                    line.quantity,
                ).await {
                    tracing::error!("Failed to handle order fulfilled for item {}: {}", line.item_id, e);
                }
            }
        }
    }).await.ok();

    // Work order completed -> add finished goods to inventory
    let svc = service.clone();
    bus.subscribe::<WorkOrderCompleted, _, _>("scm.manufacturing.work_order.completed", move |envelope| {
        let svc = svc.clone();
        async move {
            tracing::info!(
                "Processing work order completed: wo_id={}, item={}, qty={}",
                envelope.payload.work_order_id, envelope.payload.item_id, envelope.payload.quantity
            );
            if let Err(e) = svc.handle_work_order_completed(
                &envelope.payload.work_order_id,
                &envelope.payload.item_id,
                envelope.payload.quantity,
            ).await {
                tracing::error!("Failed to handle work order completed: {}", e);
            }
        }
    }).await.ok();

    // Return created -> restock items in inventory
    let svc = service.clone();
    bus.subscribe::<ReturnCreated, _, _>("scm.orders.return.created", move |envelope| {
        let svc = svc.clone();
        async move {
            tracing::info!(
                "Processing return: return_id={}, order_id={}, item={}, qty={}",
                envelope.payload.return_id, envelope.payload.order_id, envelope.payload.item_id, envelope.payload.quantity
            );
            if let Err(e) = svc.handle_return_created(
                &envelope.payload.return_id,
                &envelope.payload.item_id,
                envelope.payload.quantity,
            ).await {
                tracing::error!("Failed to handle return created: {}", e);
            }
        }
    }).await.ok();

    // Work order cancelled -> release reserved materials (log only for now)
    let svc = service.clone();
    bus.subscribe::<WorkOrderCancelled, _, _>("scm.manufacturing.work_order.cancelled", move |envelope| {
        let svc = svc.clone();
        async move {
            tracing::info!(
                "Processing work order cancelled: wo_id={}, item={}, qty={}",
                envelope.payload.work_order_id, envelope.payload.item_id, envelope.payload.quantity
            );
            // Release any reserved stock for this work order's components
            if let Err(e) = svc.handle_work_order_cancelled(
                &envelope.payload.work_order_id,
            ).await {
                tracing::error!("Failed to handle work order cancelled: {}", e);
            }
        }
    }).await.ok();

    // Work order started -> log material requirements and check stock availability
    let svc = service.clone();
    bus.subscribe::<WorkOrderStarted, _, _>("scm.manufacturing.work_order.started", move |envelope| {
        let svc = svc.clone();
        async move {
            tracing::info!(
                "Processing work order started: wo_id={}, item={}, qty={}",
                envelope.payload.work_order_id, envelope.payload.item_id, envelope.payload.quantity
            );
            if let Err(e) = svc.handle_work_order_started(
                &envelope.payload.work_order_id,
                &envelope.payload.item_id,
                envelope.payload.quantity,
            ).await {
                tracing::error!("Failed to handle work order started: {}", e);
            }
        }
    }).await.ok();

    // Sales order cancelled -> release reserved stock
    let svc = service.clone();
    bus.subscribe::<SalesOrderCancelled, _, _>("scm.orders.order.cancelled", move |envelope| {
        let svc = svc.clone();
        async move {
            tracing::info!(
                "Processing sales order cancelled event: order_id={}, order_number={}, customer_id={}",
                envelope.payload.order_id, envelope.payload.order_number, envelope.payload.customer_id
            );
            if let Err(e) = svc.handle_order_cancelled(
                &envelope.payload.order_id,
                &envelope.payload.order_number,
                &envelope.payload.reason,
            ).await {
                tracing::error!("Failed to handle order cancelled for order {}: {}", envelope.payload.order_id, e);
            }
        }
    }).await.ok();

    // PO cancelled -> cancel reservations tied to this PO
    let svc = service.clone();
    bus.subscribe::<PurchaseOrderCancelled, _, _>("scm.procurement.po.cancelled", move |envelope| {
        let svc = svc.clone();
        async move {
            tracing::info!(
                "Processing PO cancelled: po_id={}, supplier_id={}",
                envelope.payload.po_id, envelope.payload.supplier_id
            );
            if let Err(e) = svc.handle_po_cancelled(
                &envelope.payload.po_id,
                &envelope.payload.supplier_id,
                &envelope.payload.reason,
            ).await {
                tracing::error!("Failed to handle PO cancelled for PO {}: {}", envelope.payload.po_id, e);
            }
        }
    }).await.ok();

    Ok(())
}
