use crate::service::LedgerService;
use saas_common::error::AppResult;
use saas_nats_bus::NatsBus;
use saas_proto::events::{ExpenseReportApproved, PayRunCompleted, VendorInvoiceApproved, CustomerInvoiceCreated};

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

    tracing::info!("General Ledger event subscribers registered");
    Ok(())
}
