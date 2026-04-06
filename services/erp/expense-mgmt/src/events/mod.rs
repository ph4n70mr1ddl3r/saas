use crate::routes::AppState;
use saas_common::error::AppResult;
use saas_nats_bus::NatsBus;
use saas_proto::events::{EmployeeCreated, BenefitPlanCreated};

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

    tracing::info!("Expense Management event subscribers registered");
    Ok(())
}
