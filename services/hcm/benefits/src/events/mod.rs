use saas_nats_bus::NatsBus;
use saas_proto::events::EmployeeCreated;

pub async fn subscribe(bus: &NatsBus) -> anyhow::Result<()> {
    let bus = bus.clone();
    bus.subscribe::<EmployeeCreated, _, _>("hcm.employee.created", move |envelope| {
        let employee_id = envelope.payload.employee_id.clone();
        async move {
            tracing::info!(
                "Received employee.created event for {} — evaluating default plan eligibility",
                employee_id
            );
        }
    })
    .await?;
    Ok(())
}
