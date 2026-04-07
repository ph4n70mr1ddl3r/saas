use crate::service::RecruitingService;
use saas_nats_bus::NatsBus;
use saas_proto::events::EmployeeTerminated;

pub async fn subscribe(bus: &NatsBus, service: RecruitingService) -> anyhow::Result<()> {
    // When an employee is terminated, log for recruiting awareness
    let svc = service.clone();
    bus.subscribe::<EmployeeTerminated, _, _>("hcm.employee.terminated", move |envelope| {
        let svc = svc.clone();
        async move {
            tracing::info!(
                "Employee terminated: id={}, date={}. Reviewing open job postings for backfill.",
                envelope.payload.employee_id,
                envelope.payload.termination_date
            );
            // Could auto-create job postings for the same department/role
            // For now, log for awareness
            if let Err(e) = svc.handle_employee_terminated(&envelope.payload.employee_id).await {
                tracing::error!("Failed to handle employee terminated event: {}", e);
            }
        }
    }).await?;

    tracing::info!("Recruiting event subscribers registered");
    Ok(())
}
