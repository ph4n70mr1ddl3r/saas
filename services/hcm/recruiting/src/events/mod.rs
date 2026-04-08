use crate::service::RecruitingService;
use saas_nats_bus::NatsBus;
use saas_proto::events::{ApplicationStatusChanged, EmployeeTerminated};

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
            if let Err(e) = svc.handle_employee_terminated(&envelope.payload.employee_id).await {
                tracing::error!("Failed to handle employee terminated event: {}", e);
            }
        }
    }).await?;

    // Self-subscriber: log non-hired application status changes for notification
    let svc = service.clone();
    bus.subscribe::<ApplicationStatusChanged, _, _>("hcm.recruiting.application.status_changed", move |envelope| {
        let svc = svc.clone();
        let application_id = envelope.payload.application_id.clone();
        let old_status = envelope.payload.old_status.clone();
        let new_status = envelope.payload.new_status.clone();
        let candidate_first_name = envelope.payload.candidate_first_name.clone();
        let candidate_last_name = envelope.payload.candidate_last_name.clone();
        let job_title = envelope.payload.job_title.clone();
        async move {
            if new_status != "hired" {
                tracing::info!(
                    "ApplicationStatusChanged: app={}, {} -> {}, candidate={} {}",
                    application_id, old_status, new_status,
                    candidate_first_name, candidate_last_name
                );
                if let Err(e) = svc.handle_application_status_changed(
                    &application_id,
                    &old_status,
                    &new_status,
                    &candidate_first_name,
                    &candidate_last_name,
                    &job_title,
                ).await {
                    tracing::error!("Failed to handle application status changed event: {}", e);
                }
            }
        }
    }).await?;

    tracing::info!("Recruiting event subscribers registered");
    Ok(())
}
