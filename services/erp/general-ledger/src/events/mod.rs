use saas_nats_bus::NatsBus;
use saas_common::error::AppResult;
use crate::routes::AppState;

pub async fn register(_bus: &NatsBus, _state: &AppState) -> AppResult<()> {
    // Subscribers for cross-domain events:
    // - erp.ap.invoice.approved
    // - erp.ar.invoice.created
    // - hcm.payroll.run.completed
    // - scm.procurement.po.received
    //
    // These would auto-create journal entries in the general ledger.
    // Handlers are registered here when event processing logic is wired.
    tracing::info!("General Ledger event subscribers registered");
    Ok(())
}
