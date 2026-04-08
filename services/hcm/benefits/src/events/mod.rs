use crate::service::BenefitsService;
use saas_nats_bus::NatsBus;
use saas_proto::events::{
    CompensationCreated, CompensationUpdated, EmployeeCreated, EmployeeTerminated, EmployeeUpdated,
};

pub async fn subscribe(bus: &NatsBus, service: BenefitsService) -> anyhow::Result<()> {
    let svc1 = service.clone();
    bus.subscribe::<EmployeeCreated, _, _>("hcm.employee.created", move |envelope| {
        let svc1 = svc1.clone();
        let employee_id = envelope.payload.employee_id.clone();
        async move {
            tracing::info!(
                "Received employee.created event for {} — evaluating default plan eligibility",
                employee_id
            );
            if let Err(e) = svc1.handle_employee_created(&employee_id).await {
                tracing::error!("Failed to evaluate benefits eligibility for {}: {}", employee_id, e);
            }
        }
    })
    .await?;

    let svc2 = service.clone();
    bus.subscribe::<EmployeeTerminated, _, _>("hcm.employee.terminated", move |envelope| {
        let svc2 = svc2.clone();
        let employee_id = envelope.payload.employee_id.clone();
        async move {
            tracing::info!(
                "Received employee.terminated event for {} — cancelling benefit enrollments",
                employee_id
            );
            if let Err(e) = svc2.handle_employee_terminated(&employee_id).await {
                tracing::error!("Failed to cancel benefits for {}: {}", employee_id, e);
            }
        }
    })
    .await?;

    let svc3 = service.clone();
    bus.subscribe::<EmployeeUpdated, _, _>("hcm.employee.updated", move |envelope| {
        let _svc3 = svc3.clone();
        let employee_id = envelope.payload.employee_id.clone();
        let changes = envelope.payload.changes.clone();
        async move {
            tracing::info!(
                "Received employee.updated event for {} — changed fields: {:?}",
                employee_id, changes
            );
            if changes.iter().any(|c| c == "department_id") {
                tracing::warn!(
                    "Employee {} department changed — benefits eligibility may need review",
                    employee_id
                );
            }
        }
    })
    .await?;

    let svc4 = service.clone();
    bus.subscribe::<CompensationCreated, _, _>("hcm.payroll.compensation.created", move |envelope| {
        let svc4 = svc4.clone();
        let compensation_id = envelope.payload.compensation_id.clone();
        let employee_id = envelope.payload.employee_id.clone();
        let amount_cents = envelope.payload.amount_cents;
        async move {
            tracing::info!(
                "Received compensation.created event for compensation {} (employee {})",
                compensation_id,
                employee_id
            );
            if let Err(e) = svc4.handle_compensation_changed(&compensation_id, &employee_id, amount_cents, "created").await {
                tracing::error!(
                    "Failed to re-evaluate benefits eligibility for compensation {}: {}",
                    compensation_id,
                    e
                );
            }
        }
    })
    .await?;

    let svc5 = service.clone();
    bus.subscribe::<CompensationUpdated, _, _>("hcm.payroll.compensation.updated", move |envelope| {
        let svc5 = svc5.clone();
        let compensation_id = envelope.payload.compensation_id.clone();
        let employee_id = envelope.payload.employee_id.clone();
        let amount_cents = envelope.payload.amount_cents;
        async move {
            tracing::info!(
                "Received compensation.updated event for compensation {} (employee {})",
                compensation_id,
                employee_id
            );
            if let Err(e) = svc5.handle_compensation_changed(&compensation_id, &employee_id, amount_cents, "updated").await {
                tracing::error!(
                    "Failed to re-evaluate benefits eligibility for compensation {}: {}",
                    compensation_id,
                    e
                );
            }
        }
    })
    .await?;

    Ok(())
}
