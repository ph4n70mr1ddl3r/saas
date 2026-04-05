use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthState {
    pub access_token: String,
    pub user_id: String,
    pub username: String,
    pub display_name: String,
}

pub fn use_auth() -> RwSignal<Option<AuthState>> {
    use_context::<RwSignal<Option<AuthState>>>()
        .unwrap_or_else(|| {
            let signal = RwSignal::new(None);
            provide_context(signal);
            signal
        })
}
