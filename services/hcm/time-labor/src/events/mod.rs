use crate::service::TimeLaborService;
use saas_nats_bus::NatsBus;
use saas_proto::events::{
    EmployeeCreated, EmployeeTerminated, LeaveRequestApproved, LeaveRequestRejected,
    TimesheetRejected,
};

pub async fn subscribe(bus: &NatsBus, service: TimeLaborService) -> anyhow::Result<()> {
    let svc1 = service.clone();
    bus.subscribe::<EmployeeCreated, _, _>("hcm.employee.created", move |envelope| {
        let svc1 = svc1.clone();
        let employee_id = envelope.payload.employee_id.clone();
        async move {
            tracing::info!(
                "Received employee.created event for {} — creating default leave balances",
                employee_id
            );
            if let Err(e) = svc1.handle_employee_created(&employee_id).await {
                tracing::error!("Failed to create default leave balances for {}: {}", employee_id, e);
            }
        }
    })
    .await?;

    let svc2 = service.clone();
    bus.subscribe::<EmployeeTerminated, _, _>("hcm.employee.terminated", move |envelope| {
        let svc2 = svc2.clone();
        let employee_id = envelope.payload.employee_id.clone();
        async move {
            tracing::info!(
                "Received employee.terminated event for {} — rejecting pending leave requests",
                employee_id
            );
            if let Err(e) = svc2.handle_employee_terminated(&employee_id).await {
                tracing::error!("Failed to reject leave requests for {}: {}", employee_id, e);
            }
        }
    })
    .await?;

    let svc3 = service.clone();
    bus.subscribe::<TimesheetRejected, _, _>("hcm.timelabor.timesheet.rejected", move |envelope| {
        let svc3 = svc3.clone();
        let timesheet_id = envelope.payload.timesheet_id.clone();
        let employee_id = envelope.payload.employee_id.clone();
        let week_start = envelope.payload.week_start.clone();
        async move {
            tracing::info!(
                "Received timesheet.rejected event — timesheet_id={}, employee_id={}, week_start={}",
                timesheet_id,
                employee_id,
                week_start
            );
            if let Err(e) = svc3
                .handle_timesheet_rejected_notification(&timesheet_id, &employee_id, &week_start)
                .await
            {
                tracing::error!(
                    "Failed to handle timesheet.rejected notification for {}: {}",
                    timesheet_id,
                    e
                );
            }
        }
    })
    .await?;

    let svc4 = service.clone();
    bus.subscribe::<LeaveRequestApproved, _, _>("hcm.timelabor.leave.approved", move |envelope| {
        let svc4 = svc4.clone();
        let request_id = envelope.payload.request_id.clone();
        let employee_id = envelope.payload.employee_id.clone();
        let leave_type = envelope.payload.leave_type.clone();
        let days = envelope.payload.days;
        async move {
            tracing::info!(
                "Received leave.approved event — request_id={}, employee_id={}, leave_type={}, days={}",
                request_id,
                employee_id,
                leave_type,
                days
            );
            if let Err(e) = svc4
                .handle_leave_approved_notification(&request_id, &employee_id, &leave_type, days)
                .await
            {
                tracing::error!(
                    "Failed to handle leave.approved notification for {}: {}",
                    request_id,
                    e
                );
            }
        }
    })
    .await?;

    let svc5 = service.clone();
    bus.subscribe::<LeaveRequestRejected, _, _>("hcm.timelabor.leave.rejected", move |envelope| {
        let svc5 = svc5.clone();
        let request_id = envelope.payload.request_id.clone();
        let employee_id = envelope.payload.employee_id.clone();
        let leave_type = envelope.payload.leave_type.clone();
        async move {
            tracing::info!(
                "Received leave.rejected event — request_id={}, employee_id={}, leave_type={}",
                request_id,
                employee_id,
                leave_type
            );
            if let Err(e) = svc5
                .handle_leave_rejected_notification(&request_id, &employee_id, &leave_type)
                .await
            {
                tracing::error!(
                    "Failed to handle leave.rejected notification for {}: {}",
                    request_id,
                    e
                );
            }
        }
    })
    .await?;

    Ok(())
}
