use crate::routes::AppState;
use saas_common::error::AppResult;
use saas_nats_bus::NatsBus;
use saas_proto::events::DepreciationRunCompleted;

pub async fn register(bus: &NatsBus, _state: &AppState) -> AppResult<()> {
    // Depreciation Run Completed -> log for GL integration
    let bus_clone = bus.clone();
    bus.subscribe::<DepreciationRunCompleted, _, _>("erp.assets.depreciation.completed", move |envelope| {
        let bus = bus_clone.clone();
        let period = envelope.payload.period.clone();
        let total = envelope.payload.total_depreciation_cents;
        let count = envelope.payload.asset_count;
        async move {
            tracing::info!(
                "Depreciation completed: period={}, {} assets, total={} cents - GL entry should be created",
                period, count, total
            );
            if let Err(e) = bus.publish(
                "erp.gl.auto_je.depreciation",
                saas_proto::events::JournalEntryPosted {
                    entry_id: String::new(),
                    entry_number: format!("DEP-{}", period),
                    lines: vec![saas_proto::events::JournalLinePosted {
                        account_code: "1800".to_string(), // Accumulated Depreciation
                        debit_cents: 0,
                        credit_cents: total,
                    }],
                    posted_by: "system".to_string(),
                },
            ).await {
                tracing::error!("Failed to publish depreciation GL event: {}", e);
            }
        }
    }).await.ok();

    tracing::info!("Fixed Assets event subscribers registered");
    Ok(())
}
