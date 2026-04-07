use crate::routes::AppState;
use saas_common::error::AppResult;
use saas_nats_bus::NatsBus;
use saas_proto::events::DepreciationRunCompleted;

pub async fn register(bus: &NatsBus, _state: &AppState) -> AppResult<()> {
    // Log depreciation completion for audit trail.
    // GL integration is handled by the GL service's own subscriber to
    // erp.assets.depreciation.completed which creates proper journal entries.
    bus.subscribe::<DepreciationRunCompleted, _, _>("erp.assets.depreciation.completed", move |envelope| {
        let period = envelope.payload.period.clone();
        let total = envelope.payload.total_depreciation_cents;
        let count = envelope.payload.asset_count;
        async move {
            tracing::info!(
                "Depreciation completed: period={}, {} assets, total={} cents. GL auto-JE handled by GL service.",
                period, count, total
            );
        }
    }).await.ok();

    tracing::info!("Fixed Assets event subscribers registered");
    Ok(())
}
