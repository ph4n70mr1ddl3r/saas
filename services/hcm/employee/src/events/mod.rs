// Event subscriber registration - subscribes to cross-service events
use crate::service::EmployeeService;
use saas_nats_bus::NatsBus;
use saas_proto::events::{ApplicationStatusChanged, UserCreated, UserUpdated, UserDeactivated};

pub async fn register(bus: &NatsBus, service: EmployeeService) -> anyhow::Result<()> {
    let svc1 = service.clone();
    bus.subscribe::<ApplicationStatusChanged, _, _>(
        "hcm.recruiting.application.status_changed",
        move |envelope| {
            let svc1 = svc1.clone();
            let event = envelope.payload.clone();
            async move {
                if event.new_status != "hired" {
                    return;
                }
                tracing::info!(
                    "Received recruiting.application.hired for {} — auto-creating employee",
                    event.candidate_email
                );
                if let Err(e) = svc1.handle_application_hired(&event).await {
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

    // IAM user created -> auto-create employee
    let svc2 = service.clone();
    bus.subscribe::<UserCreated, _, _>(
        "iam.user.created",
        move |envelope| {
            let svc2 = svc2.clone();
            let user_id = envelope.payload.user_id.clone();
            let username = envelope.payload.username.clone();
            let email = envelope.payload.email.clone();
            async move {
                tracing::info!(
                    "Received iam.user.created for user_id={} — auto-creating employee",
                    user_id
                );
                svc2.handle_user_created(&user_id, &username, &email).await;
            }
        },
    )
    .await?;

    // IAM user updated -> sync employee email
    let svc3 = service.clone();
    bus.subscribe::<UserUpdated, _, _>(
        "iam.user.updated",
        move |envelope| {
            let svc3 = svc3.clone();
            let user_id = envelope.payload.user_id.clone();
            let username = envelope.payload.username.clone();
            let email = envelope.payload.email.clone();
            async move {
                tracing::info!(
                    "Received iam.user.updated for user_id={} — syncing employee",
                    user_id
                );
                svc3.handle_user_updated(&user_id, &username, &email).await;
            }
        },
    )
    .await?;

    // IAM user deactivated -> flag employee for HR review
    let svc4 = service.clone();
    bus.subscribe::<UserDeactivated, _, _>(
        "iam.user.deactivated",
        move |envelope| {
            let svc4 = svc4.clone();
            let user_id = envelope.payload.user_id.clone();
            let username = envelope.payload.username.clone();
            async move {
                tracing::info!(
                    "Received iam.user.deactivated for user_id={} — flagging employee",
                    user_id
                );
                svc4.handle_user_deactivated(&user_id, &username).await;
            }
        },
    )
    .await?;

    Ok(())
}
