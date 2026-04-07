use crate::service::BenefitsService;
use saas_nats_bus::NatsBus;
use saas_proto::events::{EmployeeCreated, EmployeeTerminated};

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

    Ok(())
}
