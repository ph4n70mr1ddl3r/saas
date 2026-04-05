use async_nats::Client;
use serde::{de::DeserializeOwned, Serialize};

use anyhow::Result;
use futures::StreamExt;
use crate::envelope::EventEnvelope;

#[derive(Clone)]
pub struct NatsBus {
    client: Client,
    source: String,
}

const MAX_MESSAGE_SIZE: usize = 512 * 1024; // 512KB, well under NATS 1MB default

impl NatsBus {
    pub async fn connect(url: &str, source: &str) -> Result<Self> {
        let client = async_nats::connect(url).await?;
        Ok(Self { client, source: source.to_string() })
    }

    pub async fn publish<T: Serialize>(&self, subject: &str, payload: T) -> Result<()> {
        let envelope = EventEnvelope::new(&self.source, subject, payload);
        let bytes = serde_json::to_vec(&envelope)?;
        if bytes.len() > MAX_MESSAGE_SIZE {
            anyhow::bail!("Message size {} exceeds maximum {}", bytes.len(), MAX_MESSAGE_SIZE);
        }
        self.client.publish(subject.to_string(), bytes.into()).await?;
        Ok(())
    }

    pub async fn subscribe<T, F, Fut>(&self, subject: &str, handler: F) -> Result<tokio::task::JoinHandle<()>>
    where
        T: DeserializeOwned + Serialize + Send + 'static,
        F: Fn(EventEnvelope<T>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut subscriber = self.client.subscribe(subject.to_string()).await?;
        let subject_name = subject.to_string();
        let handle = tokio::spawn(async move {
            while let Some(msg) = subscriber.next().await {
                match serde_json::from_slice::<EventEnvelope<T>>(&msg.payload) {
                    Ok(envelope) => handler(envelope).await,
                    Err(e) => {
                        tracing::error!("Failed to deserialize event on {}: {}", subject_name, e);
                    }
                }
            }
            tracing::warn!("Subscription stream ended for {}", subject_name);
        });
        Ok(handle)
    }
}
