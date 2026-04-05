use saas_nats_bus::NatsBus;
use saas_proto::events::EmployeeCreated;

pub async fn subscribe(bus: &NatsBus) -> anyhow::Result<()> {
    bus.subscribe::<EmployeeCreated, _, _>("hcm.employee.created", |envelope| {
        let employee_id = envelope.payload.employee_id.clone();
        async move {
            tracing::info!("Received employee.created event for {}", employee_id);
        }
    })
    .await?;
    Ok(())
}
