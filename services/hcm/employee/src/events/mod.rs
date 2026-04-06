// Event subscriber registration - subscribes to cross-service events
use crate::service::EmployeeService;
use saas_nats_bus::NatsBus;
use saas_proto::events::ApplicationStatusChanged;

pub async fn register(bus: &NatsBus, service: EmployeeService) -> anyhow::Result<()> {
    let service = service.clone();
    bus.subscribe::<ApplicationStatusChanged, _, _>(
        "hcm.recruiting.application.status_changed",
        move |envelope| {
            let service = service.clone();
            let event = envelope.payload.clone();
            async move {
                if event.new_status != "hired" {
                    return;
                }
                tracing::info!(
                    "Received recruiting.application.hired for {} — auto-creating employee",
                    event.candidate_email
                );
                if let Err(e) = service.handle_application_hired(&event).await {
                    tracing::error!(
                        "Failed to auto-create employee for hired application {}: {}",
                        event.application_id,
                        e
                    );
                }
            }
        },
    )
    .await?;
    Ok(())
}
