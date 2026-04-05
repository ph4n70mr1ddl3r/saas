use crate::service::PayrollService;
use saas_nats_bus::NatsBus;
use saas_proto::events::EmployeeCreated;

pub async fn subscribe(bus: &NatsBus, service: PayrollService) -> anyhow::Result<()> {
    let service = service.clone();
    bus.subscribe::<EmployeeCreated, _, _>("hcm.employee.created", move |envelope| {
        let service = service.clone();
        let employee_id = envelope.payload.employee_id.clone();
        async move {
            tracing::info!(
                "Received employee.created event for {} — creating default compensation record",
                employee_id
            );
            if let Err(e) = service.handle_employee_created(&employee_id).await {
                tracing::error!("Failed to create default compensation for {}: {}", employee_id, e);
            }
        }
    })
    .await?;
    Ok(())
}
