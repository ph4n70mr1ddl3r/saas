use crate::service::PayrollService;
use saas_nats_bus::NatsBus;
use saas_proto::events::{EmployeeCreated, EmployeeTerminated, EmployeeUpdated, EmployeeEnrolled, EnrollmentCancelled, TimesheetApproved, TimesheetSubmitted, LeaveRequestSubmitted};

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

    let svc3 = service.clone();
    bus.subscribe::<TimesheetApproved, _, _>("hcm.timelabor.timesheet.approved", move |envelope| {
        let svc3 = svc3.clone();
        let employee_id = envelope.payload.employee_id.clone();
        let week_start = envelope.payload.week_start.clone();
        async move {
            tracing::info!(
                "Received timesheet.approved for employee {} — week starting {} available for payroll",
                employee_id, week_start
            );
            if let Err(e) = svc3.handle_timesheet_approved(&employee_id, &week_start).await {
                tracing::error!("Failed to handle timesheet approved for {}: {}", employee_id, e);
            }
        }
    })
    .await?;

    let svc4 = service.clone();
    bus.subscribe::<EmployeeUpdated, _, _>("hcm.employee.updated", move |envelope| {
        let svc4 = svc4.clone();
        let employee_id = envelope.payload.employee_id.clone();
        let changes = envelope.payload.changes.clone();
        async move {
            tracing::info!(
                "Received employee.updated event for {} — changes: {:?}",
                employee_id, changes
            );
            if let Err(e) = svc4.handle_employee_updated(&employee_id, &changes).await {
                tracing::error!("Failed to handle employee updated for {}: {}", employee_id, e);
            }
        }
    })
    .await?;

    let svc5 = service.clone();
    bus.subscribe::<EmployeeEnrolled, _, _>("hcm.benefits.enrollment.created", move |envelope| {
        let svc5 = svc5.clone();
        let enrollment_id = envelope.payload.enrollment_id.clone();
        let employee_id = envelope.payload.employee_id.clone();
        let plan_id = envelope.payload.plan_id.clone();
        async move {
            tracing::info!(
                "Received benefits.enrollment.created for employee {} plan {} — creating benefit deduction",
                employee_id, plan_id
            );
            if let Err(e) = svc5.handle_benefit_enrollment_created(&enrollment_id, &employee_id, &plan_id).await {
                tracing::error!("Failed to create benefit deduction for employee {} plan {}: {}", employee_id, plan_id, e);
            }
        }
    })
    .await?;

    let svc6 = service.clone();
    bus.subscribe::<EnrollmentCancelled, _, _>("hcm.benefits.enrollment.cancelled", move |envelope| {
        let svc6 = svc6.clone();
        let enrollment_id = envelope.payload.enrollment_id.clone();
        let employee_id = envelope.payload.employee_id.clone();
        let plan_id = envelope.payload.plan_id.clone();
        async move {
            tracing::info!(
                "Received benefits.enrollment.cancelled for employee {} plan {} — deactivating benefit deduction",
                employee_id, plan_id
            );
            if let Err(e) = svc6.handle_benefit_enrollment_cancelled(&enrollment_id, &employee_id, &plan_id).await {
                tracing::error!("Failed to cancel benefit deduction for employee {} plan {}: {}", employee_id, plan_id, e);
            }
        }
    })
    .await?;

    let svc7 = service.clone();
    bus.subscribe::<TimesheetSubmitted, _, _>("hcm.timelabor.timesheet.submitted", move |envelope| {
        let svc7 = svc7.clone();
        let timesheet_id = envelope.payload.timesheet_id.clone();
        let employee_id = envelope.payload.employee_id.clone();
        let week_start = envelope.payload.week_start.clone();
        async move {
            tracing::info!(
                "Received timesheet.submitted for employee {} — week starting {}",
                employee_id, week_start
            );
            if let Err(e) = svc7.handle_timesheet_submitted(&timesheet_id, &employee_id, &week_start).await {
                tracing::error!("Failed to handle timesheet submitted for {}: {}", employee_id, e);
            }
        }
    })
    .await?;

    let svc8 = service.clone();
    bus.subscribe::<LeaveRequestSubmitted, _, _>("hcm.timelabor.leave.submitted", move |envelope| {
        let svc8 = svc8.clone();
        let request_id = envelope.payload.request_id.clone();
        let employee_id = envelope.payload.employee_id.clone();
        let leave_type = envelope.payload.leave_type.clone();
        let start_date = envelope.payload.start_date.clone();
        let end_date = envelope.payload.end_date.clone();
        async move {
            tracing::info!(
                "Received leave.submitted for employee {} — type: {}, dates: {} to {}",
                employee_id, leave_type, start_date, end_date
            );
            if let Err(e) = svc8.handle_leave_submitted(&request_id, &employee_id, &leave_type, &start_date, &end_date).await {
                tracing::error!("Failed to handle leave submitted for {}: {}", employee_id, e);
            }
        }
    })
    .await?;

    Ok(())
}
