use crate::service::LedgerService;
use saas_common::error::AppResult;
use saas_nats_bus::NatsBus;
use saas_proto::events::{
    ApInvoiceCancelled, ApPaymentCreated, ArInvoiceApproved, ArInvoiceCancelled, ArReceiptCreated, AssetCreated, AssetDisposed,
    CycleCountPosted, DepreciationRunCompleted, ExpenseReportApproved, PayRunCompleted,
    ReconciliationCompleted, ReturnProcessed, TransferCompleted, VendorInvoiceApproved,
    CustomerInvoiceCreated,
};

pub async fn register(bus: &NatsBus, service: &LedgerService) -> AppResult<()> {
    // AP Invoice Approved -> auto-create journal entry
    let svc = service.clone();
    bus.subscribe::<VendorInvoiceApproved, _, _>("erp.ap.invoice.approved", move |envelope| {
        let svc = svc.clone();
        let invoice_id = envelope.payload.invoice_id.clone();
        let total_cents = envelope.payload.total_cents;
        let gl_account_code = envelope.payload.gl_account_code.clone();
        async move {
            tracing::info!("AP invoice approved: {} ({} cents)", invoice_id, total_cents);
            if let Err(e) = svc.handle_ap_invoice_approved(&invoice_id, total_cents, &gl_account_code).await {
                tracing::error!("Failed to create auto-JE for AP invoice {}: {}", invoice_id, e);
            }
        }
    }).await.ok();

    // AR Invoice Created -> auto-create journal entry
    let svc = service.clone();
    bus.subscribe::<CustomerInvoiceCreated, _, _>("erp.ar.invoice.created", move |envelope| {
        let svc = svc.clone();
        let invoice_id = envelope.payload.invoice_id.clone();
        let total_cents = envelope.payload.total_cents;
        async move {
            tracing::info!("AR invoice created: {} ({} cents)", invoice_id, total_cents);
            if let Err(e) = svc.handle_ar_invoice_created(&invoice_id, total_cents).await {
                tracing::error!("Failed to create auto-JE for AR invoice {}: {}", invoice_id, e);
            }
        }
    }).await.ok();

    // AR Invoice Approved -> auto-create journal entry
    let svc = service.clone();
    bus.subscribe::<ArInvoiceApproved, _, _>("erp.ar.invoice.approved", move |envelope| {
        let svc = svc.clone();
        let invoice_id = envelope.payload.invoice_id.clone();
        let customer_id = envelope.payload.customer_id.clone();
        let total_cents = envelope.payload.total_cents;
        async move {
            tracing::info!("AR invoice approved: {} (customer: {}, {} cents)", invoice_id, customer_id, total_cents);
            if let Err(e) = svc.handle_ar_invoice_approved(&invoice_id, &customer_id, total_cents).await {
                tracing::error!("Failed to create auto-JE for approved AR invoice {}: {}", invoice_id, e);
            }
        }
    }).await.ok();

    // Payroll Run Completed -> auto-create journal entry
    let svc = service.clone();
    bus.subscribe::<PayRunCompleted, _, _>("hcm.payroll.run.completed", move |envelope| {
        let svc = svc.clone();
        let pay_run_id = envelope.payload.pay_run_id.clone();
        let total_net_pay_cents = envelope.payload.total_net_pay_cents;
        async move {
            tracing::info!("Payroll run completed: {} ({} cents)", pay_run_id, total_net_pay_cents);
            if let Err(e) = svc.handle_payroll_run_completed(&pay_run_id, total_net_pay_cents).await {
                tracing::error!("Failed to create auto-JE for payroll run {}: {}", pay_run_id, e);
            }
        }
    }).await.ok();

    // Expense Report Approved -> auto-create journal entry
    let svc = service.clone();
    bus.subscribe::<ExpenseReportApproved, _, _>("erp.expense.report.approved", move |envelope| {
        let svc = svc.clone();
        let report_id = envelope.payload.report_id.clone();
        let total_cents = envelope.payload.total_cents;
        let gl_account_code = envelope.payload.gl_account_code.clone();
        async move {
            tracing::info!("Expense report approved: {} ({} cents)", report_id, total_cents);
            if let Err(e) = svc.handle_expense_report_approved(&report_id, total_cents, &gl_account_code).await {
                tracing::error!("Failed to create auto-JE for expense report {}: {}", report_id, e);
            }
        }
    }).await.ok();

    // Depreciation Run Completed -> auto-create journal entry
    let svc = service.clone();
    bus.subscribe::<DepreciationRunCompleted, _, _>("erp.assets.depreciation.completed", move |envelope| {
        let svc = svc.clone();
        let period = envelope.payload.period.clone();
        let total_depreciation_cents = envelope.payload.total_depreciation_cents;
        let asset_count = envelope.payload.asset_count;
        async move {
            tracing::info!(
                "Depreciation completed: period={}, {} assets, total={} cents",
                period, asset_count, total_depreciation_cents
            );
            if let Err(e) = svc.handle_depreciation_completed(&period, total_depreciation_cents, asset_count).await {
                tracing::error!("Failed to create auto-JE for depreciation {}: {}", period, e);
            }
        }
    }).await.ok();

    // AP Payment Created -> auto-create journal entry (clear AP liability)
    let svc = service.clone();
    bus.subscribe::<ApPaymentCreated, _, _>("erp.ap.payment.created", move |envelope| {
        let svc = svc.clone();
        let payment_id = envelope.payload.payment_id.clone();
        let amount_cents = envelope.payload.amount_cents;
        async move {
            tracing::info!("AP payment created: {} ({} cents)", payment_id, amount_cents);
            if let Err(e) = svc.handle_ap_payment_created(&payment_id, amount_cents).await {
                tracing::error!("Failed to create auto-JE for AP payment {}: {}", payment_id, e);
            }
        }
    }).await.ok();

    // AR Receipt Created -> auto-create journal entry (clear AR receivable)
    let svc = service.clone();
    bus.subscribe::<ArReceiptCreated, _, _>("erp.ar.receipt.created", move |envelope| {
        let svc = svc.clone();
        let receipt_id = envelope.payload.receipt_id.clone();
        let amount_cents = envelope.payload.amount_cents;
        async move {
            tracing::info!("AR receipt created: {} ({} cents)", receipt_id, amount_cents);
            if let Err(e) = svc.handle_ar_receipt_created(&receipt_id, amount_cents).await {
                tracing::error!("Failed to create auto-JE for AR receipt {}: {}", receipt_id, e);
            }
        }
    }).await.ok();

    // Asset Created -> auto-create journal entry (capitalize fixed asset)
    let svc = service.clone();
    bus.subscribe::<AssetCreated, _, _>("erp.assets.asset.created", move |envelope| {
        let svc = svc.clone();
        let asset_id = envelope.payload.asset_id.clone();
        let name = envelope.payload.name.clone();
        let cost_cents = envelope.payload.purchase_cost_cents;
        async move {
            tracing::info!("Asset created: {} ({} cents)", name, cost_cents);
            if let Err(e) = svc.handle_asset_created(&asset_id, &name, cost_cents).await {
                tracing::error!("Failed to create auto-JE for asset {}: {}", asset_id, e);
            }
        }
    }).await.ok();

    // Asset Disposed -> auto-create journal entry
    let svc = service.clone();
    bus.subscribe::<AssetDisposed, _, _>("erp.assets.asset.disposed", move |envelope| {
        let svc = svc.clone();
        let asset_id = envelope.payload.asset_id.clone();
        let name = envelope.payload.name.clone();
        let cost_cents = envelope.payload.cost_cents;
        let accumulated_depreciation_cents = envelope.payload.accumulated_depreciation_cents;
        async move {
            tracing::info!("Asset disposed: {} ({})", name, asset_id);
            if let Err(e) = svc.handle_asset_disposed(&asset_id, &name, cost_cents, accumulated_depreciation_cents).await {
                tracing::error!("Failed to create auto-JE for asset disposal {}: {}", asset_id, e);
            }
        }
    }).await.ok();

    // Cash Transfer Completed -> auto-create journal entry
    let svc = service.clone();
    bus.subscribe::<TransferCompleted, _, _>("erp.cash.transfer.completed", move |envelope| {
        let svc = svc.clone();
        let from_account = envelope.payload.from_account_id.clone();
        let to_account = envelope.payload.to_account_id.clone();
        let amount_cents = envelope.payload.amount_cents;
        async move {
            tracing::info!("Cash transfer: {} -> {} ({} cents)", from_account, to_account, amount_cents);
            if let Err(e) = svc.handle_transfer_completed(&from_account, &to_account, amount_cents).await {
                tracing::error!("Failed to create auto-JE for cash transfer: {}", e);
            }
        }
    }).await.ok();

    // Reconciliation Completed -> auto-create adjustment journal entry
    let svc = service.clone();
    bus.subscribe::<ReconciliationCompleted, _, _>("erp.cash.reconciliation.completed", move |envelope| {
        let svc = svc.clone();
        let recon_id = envelope.payload.reconciliation_id.clone();
        let difference_cents = envelope.payload.difference_cents;
        async move {
            tracing::info!("Reconciliation completed: {} (difference: {} cents)", recon_id, difference_cents);
            if let Err(e) = svc.handle_reconciliation_completed(&recon_id, difference_cents).await {
                tracing::error!("Failed to create auto-JE for reconciliation {}: {}", recon_id, e);
            }
        }
    }).await.ok();

    // AP Invoice Cancelled -> reverse the original auto-created JE
    let svc = service.clone();
    bus.subscribe::<ApInvoiceCancelled, _, _>("erp.ap.invoice.cancelled", move |envelope| {
        let svc = svc.clone();
        let invoice_id = envelope.payload.invoice_id.clone();
        let vendor_id = envelope.payload.vendor_id.clone();
        async move {
            tracing::info!(
                "AP invoice cancelled: {} (vendor: {}) - reversing original JE",
                invoice_id, vendor_id
            );
            if let Err(e) = svc.handle_ap_invoice_cancelled(&invoice_id, &vendor_id).await {
                tracing::error!("Failed to reverse JE for cancelled AP invoice {}: {}", invoice_id, e);
            }
        }
    }).await.ok();

    // AR Invoice Cancelled -> reverse the original auto-created JE
    let svc = service.clone();
    bus.subscribe::<ArInvoiceCancelled, _, _>("erp.ar.invoice.cancelled", move |envelope| {
        let svc = svc.clone();
        let invoice_id = envelope.payload.invoice_id.clone();
        let customer_id = envelope.payload.customer_id.clone();
        async move {
            tracing::info!(
                "AR invoice cancelled: {} (customer: {}) - reversing original JE",
                invoice_id, customer_id
            );
            if let Err(e) = svc.handle_ar_invoice_cancelled(&invoice_id, &customer_id).await {
                tracing::error!("Failed to reverse JE for cancelled AR invoice {}: {}", invoice_id, e);
            }
        }
    }).await.ok();

    // Cycle Count Posted -> auto-create inventory adjustment journal entry
    let svc = service.clone();
    bus.subscribe::<CycleCountPosted, _, _>("scm.inventory.cycle_count.posted", move |envelope| {
        let svc = svc.clone();
        let session_id = envelope.payload.session_id.clone();
        let warehouse_id = envelope.payload.warehouse_id.clone();
        let adjustment_count = envelope.payload.adjustment_count;
        async move {
            tracing::info!(
                "Cycle count posted: session={}, warehouse={}, {} adjustments",
                session_id, warehouse_id, adjustment_count
            );
            if let Err(e) = svc.handle_cycle_count_posted(&session_id, &warehouse_id, adjustment_count).await {
                tracing::error!("Failed to create auto-JE for cycle count session {}: {}", session_id, e);
            }
        }
    }).await.ok();

    // Return Processed -> auto-create refund journal entry
    let svc = service.clone();
    bus.subscribe::<ReturnProcessed, _, _>("scm.orders.return.processed", move |envelope| {
        let svc = svc.clone();
        let return_id = envelope.payload.return_id.clone();
        let order_id = envelope.payload.order_id.clone();
        let refund_amount_cents = envelope.payload.refund_amount_cents;
        async move {
            tracing::info!("Return processed: {} (order: {}, {} cents)", return_id, order_id, refund_amount_cents);
            if let Err(e) = svc.handle_return_processed(&return_id, &order_id, refund_amount_cents).await {
                tracing::error!("Failed to create auto-JE for return {}: {}", return_id, e);
            }
        }
    }).await.ok();

    tracing::info!("General Ledger event subscribers registered");
    Ok(())
}
