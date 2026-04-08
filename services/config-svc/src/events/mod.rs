use crate::service::ConfigService;
use saas_nats_bus::NatsBus;
use saas_proto::events::ConfigUpdated;

pub async fn subscribe(bus: &NatsBus, service: ConfigService) -> anyhow::Result<()> {
    // Self-subscriber: log config changes for audit awareness
    let svc = service.clone();
    bus.subscribe::<ConfigUpdated, _, _>("config.updated", move |envelope| {
        let svc = svc.clone();
        let key = envelope.payload.key.clone();
        let value = envelope.payload.value.clone();
        async move {
            tracing::info!(
                "ConfigUpdated event received: key='{}', value='{}'",
                key, value
            );
            if let Err(e) = svc.handle_config_updated(&key, &value).await {
                tracing::error!("Failed to handle config updated event: {}", e);
            }
        }
    }).await?;

    tracing::info!("Config service event subscribers registered");
    Ok(())
}
