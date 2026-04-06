use crate::service::PerformanceService;
use saas_nats_bus::NatsBus;
use saas_proto::events::EmployeeCreated;

pub async fn subscribe(bus: &NatsBus, service: PerformanceService) -> anyhow::Result<()> {
    let service = service.clone();
    bus.subscribe::<EmployeeCreated, _, _>("hcm.employee.created", move |envelope| {
        let service = service.clone();
        let employee_id = envelope.payload.employee_id.clone();
        let first_name = envelope.payload.first_name.clone();
        let last_name = envelope.payload.last_name.clone();
        async move {
            tracing::info!(
                "Received employee.created event for {} {} ({}) — creating default onboarding goal",
                first_name,
                last_name,
                employee_id
            );
            if let Err(e) = service.handle_employee_created(&employee_id, &first_name, &last_name).await {
                tracing::error!(
                    "Failed to create default goal for employee {}: {}",
                    employee_id,
                    e
                );
            }
        }
    })
    .await?;
    Ok(())
}
