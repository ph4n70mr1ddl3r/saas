use crate::routes::AppState;
use saas_common::error::AppResult;
use saas_nats_bus::NatsBus;
use saas_proto::events::{OrderFulfilled, PeriodClosed, ReturnApproved, YearEndClosed};

pub async fn register(bus: &NatsBus, state: &AppState) -> AppResult<()> {
    // Order Fulfilled -> auto-create AR invoice
    let svc = state.service.clone();
    bus.subscribe::<OrderFulfilled, _, _>("scm.orders.order.fulfilled", move |envelope| {
        let svc = svc.clone();
        let order_id = envelope.payload.order_id.clone();
        let order_number = envelope.payload.order_number.clone();
        let customer_id = envelope.payload.customer_id.clone();
        let lines: Vec<(String, i64, i64)> = envelope.payload.lines.iter()
            .map(|l| (l.item_id.clone(), l.quantity, l.unit_price_cents))
            .collect();
        let line_count = lines.len();
        async move {
            tracing::info!(
                "Order fulfilled: order={}, customer={}, {} lines - creating auto-invoice",
                order_id, customer_id, line_count
            );
            if let Err(e) = svc.handle_order_fulfilled(&order_id, &order_number, &customer_id, &lines).await {
                tracing::error!("Failed to create auto-invoice for order {}: {}", order_number, e);
            }
        }
    }).await.ok();

    // Return Approved -> auto-create credit memo
    let svc2 = state.service.clone();
    bus.subscribe::<ReturnApproved, _, _>("scm.orders.return.approved", move |envelope| {
        let svc = svc2.clone();
        let return_id = envelope.payload.return_id.clone();
        let order_id = envelope.payload.order_id.clone();
        let item_id = envelope.payload.item_id.clone();
        let quantity = envelope.payload.quantity;
        async move {
            tracing::info!(
                "Return approved event received: return_id={}, order_id={}",
                return_id, order_id
            );
            if let Err(e) = svc.handle_return_approved(&return_id, &order_id, &item_id, quantity).await {
                tracing::error!("Failed to create credit memo for return {}: {}", return_id, e);
            }
        }
    }).await.ok();

    // GL Period Closed -> block AR transactions for closed period
    let svc = state.service.clone();
    bus.subscribe::<PeriodClosed, _, _>("erp.gl.period.closed", move |envelope| {
        let svc = svc.clone();
        let period_id = envelope.payload.period_id.clone();
        let name = envelope.payload.name.clone();
        let fiscal_year = envelope.payload.fiscal_year;
        async move {
            if let Err(e) = svc.handle_period_closed(&period_id, &name, fiscal_year).await {
                tracing::error!(
                    "Failed to handle GL period closed for period {}: {}", period_id, e
                );
            }
        }
    }).await.ok();

    // GL Year-End Closed -> block AR transactions for closed fiscal year
    let svc = state.service.clone();
    bus.subscribe::<YearEndClosed, _, _>("erp.gl.year_end.closed", move |envelope| {
        let svc = svc.clone();
        let fiscal_year = envelope.payload.fiscal_year;
        let entry_id = envelope.payload.entry_id.clone();
        async move {
            if let Err(e) = svc.handle_year_end_closed(fiscal_year, &entry_id).await {
                tracing::error!(
                    "Failed to handle GL year-end close for fiscal year {}: {}", fiscal_year, e
                );
            }
        }
    }).await.ok();

    tracing::info!("Accounts Receivable event subscribers registered");
    Ok(())
}
