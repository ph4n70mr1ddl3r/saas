use crate::routes::AppState;
use saas_common::error::AppResult;
use saas_nats_bus::NatsBus;

pub async fn register(_bus: &NatsBus, _state: &AppState) -> AppResult<()> {
    tracing::info!("Accounts Payable event subscribers registered");
    Ok(())
}
