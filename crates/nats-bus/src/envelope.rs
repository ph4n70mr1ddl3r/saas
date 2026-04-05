use serde::Serialize;
use std::future::Future;

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct EventEnvelope<T: Serialize> {
    pub event_id: String,
    pub event_type: String,
    pub timestamp: String,
    pub source: String,
    pub correlation_id: String,
    pub payload: T,
}

impl<T: Serialize> EventEnvelope<T> {
    pub fn new(source: &str, event_type: &str, payload: T) -> Self {
        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            event_type: event_type.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            source: source.to_string(),
            correlation_id: uuid::Uuid::new_v4().to_string(),
            payload,
        }
    }

    pub fn with_correlation_id(mut self, correlation_id: &str) -> Self {
        self.correlation_id = correlation_id.to_string();
        self
    }
}
