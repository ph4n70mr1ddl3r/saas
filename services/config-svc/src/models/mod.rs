use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConfigEntry {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetConfigRequest {
    pub value: String,
}
