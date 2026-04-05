use crate::service::BenefitsService;
use saas_nats_bus::NatsBus;
use saas_proto::events::EmployeeCreated;

pub async fn subscribe(bus: &NatsBus, service: BenefitsService) -> anyhow::Result<()> {
    let service = service.clone();
    bus.subscribe::<EmployeeCreated, _, _>("hcm.employee.created", move |envelope| {
        let service = service.clone();
        let employee_id = envelope.payload.employee_id.clone();
        async move {
            tracing::info!(
                "Received employee.created event for {} — evaluating default plan eligibility",
                employee_id
            );
            if let Err(e) = service.handle_employee_created(&employee_id).await {
                tracing::error!("Failed to evaluate benefits eligibility for {}: {}", employee_id, e);
            }
        }
    })
    .await?;
    Ok(())
}
