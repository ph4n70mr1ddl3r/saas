use saas_nats_bus::NatsBus;
use saas_proto::events::EmployeeCreated;

 use tracing;

pub async fn subscribe(bus: &NatsBus) -> anyhow::Result<()> {
    bus.subscribe::<EmployeeCreated, _>(
        "hcm.employee.created",
        |envelope| {
            let employee_id = envelope.payload.employee_id.clone();
            tracing::info!("Received employee.created event for {}", employee_id);
            tokio::spawn(async move {
                tracing::info!(
                    "Auto-creating compensation record for employee {}",
                    employee_id
                );
            });
        },
    )
    .await?;
    Ok(())
}
