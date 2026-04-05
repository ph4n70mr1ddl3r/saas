use saas_nats_bus::NatsBus;
use saas_common::error::AppResult;
use crate::routes::AppState;

pub async fn register(_bus: &NatsBus, _state: &AppState) -> AppResult<()> {
    tracing::info!("Accounts Payable event subscribers registered");
    Ok(())
}
