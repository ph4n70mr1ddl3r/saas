use crate::service::PayrollService;
use saas_nats_bus::NatsBus;
use saas_proto::events::{EmployeeCreated, EmployeeTerminated};

pub async fn subscribe(bus: &NatsBus, service: PayrollService) -> anyhow::Result<()> {
    let svc1 = service.clone();
    bus.subscribe::<EmployeeCreated, _, _>("hcm.employee.created", move |envelope| {
        let svc1 = svc1.clone();
        let employee_id = envelope.payload.employee_id.clone();
        async move {
            tracing::info!(
                "Received employee.created event for {} — creating default compensation record",
                employee_id
            );
            if let Err(e) = svc1.handle_employee_created(&employee_id).await {
                tracing::error!("Failed to create default compensation for {}: {}", employee_id, e);
            }
        }
    })
    .await?;

    let svc2 = service.clone();
    bus.subscribe::<EmployeeTerminated, _, _>("hcm.employee.terminated", move |envelope| {
        let svc2 = svc2.clone();
        let employee_id = envelope.payload.employee_id.clone();
        let termination_date = envelope.payload.termination_date.clone();
        async move {
            tracing::info!(
                "Received employee.terminated event for {} — ending compensation",
                employee_id
            );
            if let Err(e) = svc2.handle_employee_terminated(&employee_id, &termination_date).await {
                tracing::error!("Failed to end compensation for {}: {}", employee_id, e);
            }
        }
    })
    .await?;

    Ok(())
}
