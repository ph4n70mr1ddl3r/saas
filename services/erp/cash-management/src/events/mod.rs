use crate::routes::AppState;
use saas_common::error::AppResult;
use saas_nats_bus::NatsBus;
use saas_proto::events::{ApPaymentCreated, ArReceiptCreated, ExpenseReportPaid, PayRunCompleted};

pub async fn register(bus: &NatsBus, _state: &AppState) -> AppResult<()> {
    // AP Payment Created -> auto-create bank withdrawal
    let bus_clone = bus.clone();
    bus.subscribe::<ApPaymentCreated, _, _>("erp.ap.payment.created", move |envelope| {
        let bus = bus_clone.clone();
        let amount = envelope.payload.amount_cents;
        let vendor_id = envelope.payload.vendor_id.clone();
        async move {
            tracing::info!(
                "AP payment created: vendor={}, amount={} - triggering bank withdrawal",
                vendor_id, amount
            );
            if let Err(e) = bus.publish(
                "erp.cash.withdrawal.requested",
                saas_proto::events::TransferCompleted {
                    from_account_id: String::new(),
                    to_account_id: String::new(),
                    amount_cents: amount,
                    currency: "USD".to_string(),
                },
            ).await {
                tracing::error!("Failed to publish cash withdrawal request: {}", e);
            }
        }
    }).await.ok();

    // AR Receipt Created -> auto-create bank deposit
    let bus_clone = bus.clone();
    bus.subscribe::<ArReceiptCreated, _, _>("erp.ar.receipt.created", move |envelope| {
        let bus = bus_clone.clone();
        let amount = envelope.payload.amount_cents;
        let customer_id = envelope.payload.customer_id.clone();
        async move {
            tracing::info!(
                "AR receipt created: customer={}, amount={} - triggering bank deposit",
                customer_id, amount
            );
            if let Err(e) = bus.publish(
                "erp.cash.deposit.requested",
                saas_proto::events::TransferCompleted {
                    from_account_id: String::new(),
                    to_account_id: String::new(),
                    amount_cents: amount,
                    currency: "USD".to_string(),
                },
            ).await {
                tracing::error!("Failed to publish cash deposit request: {}", e);
            }
        }
    }).await.ok();

    // Expense Report Paid -> auto-create bank withdrawal for reimbursement
    let bus_clone = bus.clone();
    bus.subscribe::<ExpenseReportPaid, _, _>("erp.expense.report.paid", move |envelope| {
        let bus = bus_clone.clone();
        let amount = envelope.payload.total_cents;
        let employee_id = envelope.payload.employee_id.clone();
        async move {
            tracing::info!(
                "Expense report paid: employee={}, amount={} - triggering reimbursement withdrawal",
                employee_id, amount
            );
            if let Err(e) = bus.publish(
                "erp.cash.reimbursement.requested",
                saas_proto::events::TransferCompleted {
                    from_account_id: String::new(),
                    to_account_id: String::new(),
                    amount_cents: amount,
                    currency: "USD".to_string(),
                },
            ).await {
                tracing::error!("Failed to publish cash reimbursement request: {}", e);
            }
        }
    }).await.ok();

    // Payroll Run Completed -> auto-create bank withdrawal for payroll disbursement
    let bus_clone = bus.clone();
    bus.subscribe::<PayRunCompleted, _, _>("hcm.payroll.run.completed", move |envelope| {
        let bus = bus_clone.clone();
        let total = envelope.payload.total_net_pay_cents;
        let pay_run_id = envelope.payload.pay_run_id.clone();
        async move {
            tracing::info!(
                "Payroll run completed: run={}, total={} - triggering payroll disbursement",
                pay_run_id, total
            );
            if let Err(e) = bus.publish(
                "erp.cash.payroll.disbursement.requested",
                saas_proto::events::TransferCompleted {
                    from_account_id: String::new(),
                    to_account_id: String::new(),
                    amount_cents: total,
                    currency: "USD".to_string(),
                },
            ).await {
                tracing::error!("Failed to publish payroll disbursement request: {}", e);
            }
        }
    }).await.ok();

    tracing::info!("Cash Management event subscribers registered");
    Ok(())
}
