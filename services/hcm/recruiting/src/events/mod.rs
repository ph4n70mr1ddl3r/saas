// Recruiting service does not subscribe to any external events.
// It publishes `hcm.recruiting.application.status_changed` when an application
// status changes to "hired" — handled directly in RecruitingService.

pub async fn subscribe(_bus: &saas_nats_bus::NatsBus) -> anyhow::Result<()> {
    Ok(())
}
