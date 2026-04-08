use crate::routes::AppState;
use saas_common::error::AppResult;
use saas_nats_bus::NatsBus;
use saas_proto::events::{
    EmployeeCreated, BenefitPlanCreated, BudgetActivated, YearEndClosed,
    ExpenseReportSubmitted, ExpenseReportRejected,
};

pub async fn register(bus: &NatsBus, state: &AppState) -> AppResult<()> {
    // Employee Created -> auto-create onboarding expense report
    let svc = state.service.clone();
    bus.subscribe::<EmployeeCreated, _, _>("hcm.employee.created", move |envelope| {
        let svc = svc.clone();
        let employee_id = envelope.payload.employee_id.clone();
        let first_name = envelope.payload.first_name.clone();
        let last_name = envelope.payload.last_name.clone();
        async move {
            tracing::info!(
                "Employee created: {} {} ({}) - creating onboarding expense report",
                first_name, last_name, employee_id
            );
            if let Err(e) = svc.handle_employee_created(&employee_id, &first_name, &last_name).await {
                tracing::error!(
                    "Failed to create onboarding expense report for employee {}: {}",
                    employee_id, e
                );
            }
        }
    }).await.ok();

    // Benefit Plan Created -> auto-create expense category
    let svc = state.service.clone();
    bus.subscribe::<BenefitPlanCreated, _, _>("hcm.benefits.plan.created", move |envelope| {
        let svc = svc.clone();
        let plan_id = envelope.payload.plan_id.clone();
        let name = envelope.payload.name.clone();
        let plan_type = envelope.payload.plan_type.clone();
        async move {
            tracing::info!(
                "Benefit plan created: {} ({}) - creating expense category",
                name, plan_type
            );
            if let Err(e) = svc.handle_benefit_plan_created(&plan_id, &name, &plan_type).await {
                tracing::error!(
                    "Failed to create expense category for benefit plan {}: {}",
                    plan_id, e
                );
            }
        }
    }).await.ok();

    // Budget Activated -> track budget for expense validation awareness
    let svc = state.service.clone();
    bus.subscribe::<BudgetActivated, _, _>("erp.gl.budget.activated", move |envelope| {
        let svc = svc.clone();
        let budget_id = envelope.payload.budget_id.clone();
        let name = envelope.payload.name.clone();
        let total_budget_cents = envelope.payload.total_budget_cents;
        async move {
            tracing::info!(
                "Budget activated event received: '{}' (id={})",
                name, budget_id
            );
            if let Err(e) = svc.handle_budget_activated(&budget_id, &name, total_budget_cents).await {
                tracing::error!(
                    "Failed to process budget activation for budget {}: {}",
                    budget_id, e
                );
            }
        }
    }).await.ok();

    // GL Year-End Closed -> block expense transactions for closed fiscal year
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

    // Expense Report Submitted -> notify managers for approval
    let svc = state.service.clone();
    bus.subscribe::<ExpenseReportSubmitted, _, _>("erp.expense.report.submitted", move |envelope| {
        let svc = svc.clone();
        let report_id = envelope.payload.report_id.clone();
        let employee_id = envelope.payload.employee_id.clone();
        let title = envelope.payload.title.clone();
        async move {
            tracing::info!(
                "Expense report submitted event received: report_id={}, employee_id={}, title='{}'",
                report_id, employee_id, title
            );
            if let Err(e) = svc.handle_expense_report_submitted_notification(&report_id, &employee_id, &title).await {
                tracing::error!(
                    "Failed to handle expense report submitted notification for report {}: {}",
                    report_id, e
                );
            }
        }
    }).await.ok();

    // Expense Report Rejected -> notify employee of rejection
    let svc = state.service.clone();
    bus.subscribe::<ExpenseReportRejected, _, _>("erp.expense.report.rejected", move |envelope| {
        let svc = svc.clone();
        let report_id = envelope.payload.report_id.clone();
        let employee_id = envelope.payload.employee_id.clone();
        let reason = envelope.payload.reason.clone();
        async move {
            tracing::info!(
                "Expense report rejected event received: report_id={}, employee_id={}",
                report_id, employee_id
            );
            if let Err(e) = svc.handle_expense_report_rejected_notification(&report_id, &employee_id, &reason).await {
                tracing::error!(
                    "Failed to handle expense report rejected notification for report {}: {}",
                    report_id, e
                );
            }
        }
    }).await.ok();

    tracing::info!("Expense Management event subscribers registered");
    Ok(())
}
