use crate::service::PerformanceService;
use saas_nats_bus::NatsBus;
use saas_proto::events::{EmployeeCreated, ReviewCycleActivated, ReviewSubmitted};

pub async fn subscribe(bus: &NatsBus, service: PerformanceService) -> anyhow::Result<()> {
    let svc1 = service.clone();
    bus.subscribe::<EmployeeCreated, _, _>("hcm.employee.created", move |envelope| {
        let svc1 = svc1.clone();
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
            if let Err(e) = svc1.handle_employee_created(&employee_id, &first_name, &last_name).await {
                tracing::error!(
                    "Failed to create default goal for employee {}: {}",
                    employee_id,
                    e
                );
            }
        }
    })
    .await?;

    let svc2 = service.clone();
    bus.subscribe::<ReviewSubmitted, _, _>("hcm.performance.review.submitted", move |envelope| {
        let svc2 = svc2.clone();
        let assignment_id = envelope.payload.assignment_id.clone();
        let cycle_id = envelope.payload.cycle_id.clone();
        let employee_id = envelope.payload.employee_id.clone();
        let reviewer_id = envelope.payload.reviewer_id.clone();
        let rating = envelope.payload.rating;
        async move {
            tracing::info!(
                "Received review.submitted event — assignment_id={}, cycle_id={}, employee_id={}, reviewer_id={}, rating={}",
                assignment_id,
                cycle_id,
                employee_id,
                reviewer_id,
                rating
            );
            if let Err(e) = svc2
                .handle_review_submitted_notification(
                    &assignment_id,
                    &cycle_id,
                    &employee_id,
                    &reviewer_id,
                    rating,
                )
                .await
            {
                tracing::error!(
                    "Failed to handle review.submitted notification for assignment {}: {}",
                    assignment_id,
                    e
                );
            }
        }
    })
    .await?;

    let svc3 = service.clone();
    bus.subscribe::<ReviewCycleActivated, _, _>("hcm.performance.cycle.activated", move |envelope| {
        let svc3 = svc3.clone();
        let cycle_id = envelope.payload.cycle_id.clone();
        let name = envelope.payload.name.clone();
        async move {
            tracing::info!(
                "Received performance.cycle.activated event — cycle_id={}, name='{}'",
                cycle_id,
                name
            );
            if let Err(e) = svc3.handle_cycle_activated_notification(&cycle_id, &name).await {
                tracing::error!(
                    "Failed to handle cycle.activated notification for cycle {}: {}",
                    cycle_id,
                    e
                );
            }
        }
    })
    .await?;

    Ok(())
}
