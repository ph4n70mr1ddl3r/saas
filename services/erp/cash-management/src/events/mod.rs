use crate::routes::AppState;
use saas_common::error::AppResult;
use saas_nats_bus::NatsBus;
use saas_proto::events::{ApPaymentCreated, ArReceiptCreated, ExpenseReportPaid, PayRunCompleted};

pub async fn register(bus: &NatsBus, state: &AppState) -> AppResult<()> {
    // AP Payment Created -> auto-create bank withdrawal
    let svc = state.service.clone();
    bus.subscribe::<ApPaymentCreated, _, _>("erp.ap.payment.created", move |envelope| {
        let svc = svc.clone();
        let amount = envelope.payload.amount_cents;
        let vendor_id = envelope.payload.vendor_id.clone();
        async move {
            tracing::info!(
                "AP payment created: vendor={}, amount={} - creating bank withdrawal",
                vendor_id, amount
            );
            if let Err(e) = svc.handle_ap_payment_created(amount, &vendor_id).await {
                tracing::error!("Failed to create bank withdrawal for AP payment to vendor {}: {}", vendor_id, e);
            }
        }
    }).await.ok();

    // AR Receipt Created -> auto-create bank deposit
    let svc = state.service.clone();
    bus.subscribe::<ArReceiptCreated, _, _>("erp.ar.receipt.created", move |envelope| {
        let svc = svc.clone();
        let amount = envelope.payload.amount_cents;
        let customer_id = envelope.payload.customer_id.clone();
        async move {
            tracing::info!(
                "AR receipt created: customer={}, amount={} - creating bank deposit",
                customer_id, amount
            );
            if let Err(e) = svc.handle_ar_receipt_created(amount, &customer_id).await {
                tracing::error!("Failed to create bank deposit for AR receipt from customer {}: {}", customer_id, e);
            }
        }
    }).await.ok();

    // Expense Report Paid -> auto-create bank withdrawal for reimbursement
    let svc = state.service.clone();
    bus.subscribe::<ExpenseReportPaid, _, _>("erp.expense.report.paid", move |envelope| {
        let svc = svc.clone();
        let amount = envelope.payload.total_cents;
        let employee_id = envelope.payload.employee_id.clone();
        async move {
            tracing::info!(
                "Expense report paid: employee={}, amount={} - creating reimbursement withdrawal",
                employee_id, amount
            );
            if let Err(e) = svc.handle_expense_report_paid(amount, &employee_id).await {
                tracing::error!("Failed to create reimbursement withdrawal for employee {}: {}", employee_id, e);
            }
        }
    }).await.ok();

    // Payroll Run Completed -> auto-create bank withdrawal for payroll disbursement
    let svc = state.service.clone();
    bus.subscribe::<PayRunCompleted, _, _>("hcm.payroll.run.completed", move |envelope| {
        let svc = svc.clone();
        let total = envelope.payload.total_net_pay_cents;
        let pay_run_id = envelope.payload.pay_run_id.clone();
        async move {
            tracing::info!(
                "Payroll run completed: run={}, total={} - creating payroll disbursement",
                pay_run_id, total
            );
            if let Err(e) = svc.handle_payroll_completed(total, &pay_run_id).await {
                tracing::error!("Failed to create payroll disbursement for run {}: {}", pay_run_id, e);
            }
        }
    }).await.ok();

    tracing::info!("Cash Management event subscribers registered");
    Ok(())
}
