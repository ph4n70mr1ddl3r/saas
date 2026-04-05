use saas_nats_bus::NatsBus;

pub async fn subscribe(_bus: &NatsBus) -> anyhow::Result<()> {
    Ok(())
}

#[allow(dead_code)]
pub fn register() {
    // Event registration placeholder
}
