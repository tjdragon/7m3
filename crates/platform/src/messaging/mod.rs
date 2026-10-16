pub mod in_memory;

#[cfg(feature = "kafka")]
pub mod kafka;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum MessagingError {
    #[error("Publish error on topic '{0}': {1}")]
    PublishFailed(String, String),

    #[error("Subscription error on topic '{0}': {1}")]
    SubscribeFailed(String, String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("Unsupported messaging backend: '{0}'")]
    UnsupportedBackend(String),
}

/// Standardized envelope adhering to CloudEvents / 12-factor event streams.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope<T> {
    pub id: Uuid,
    pub event_type: String,
    pub source: String,
    pub timestamp: DateTime<Utc>,
    pub payload: T,
}

impl<T> EventEnvelope<T> {
    pub fn new(event_type: impl Into<String>, source: impl Into<String>, payload: T) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type: event_type.into(),
            source: source.into(),
            timestamp: Utc::now(),
            payload,
        }
    }
}

/// Handler for processing incoming events from subscribers.
#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, topic: &str, key: Option<&str>, payload: &[u8]) -> Result<(), MessagingError>;
}

/// Abstract contract for producing events (Kafka, AWS MSK, GCP Managed Service for Apache Kafka, In-Memory).
#[async_trait]
pub trait EventPublisher: Send + Sync {
    fn publisher_name(&self) -> &'static str;

    async fn publish(&self, topic: &str, key: Option<&str>, payload: &[u8]) -> Result<(), MessagingError>;
}

/// Generic extension methods on EventPublisher
#[async_trait]
pub trait EventPublisherExt {
    async fn publish_envelope<T: Serialize + Sync + Send>(
        &self,
        topic: &str,
        key: Option<&str>,
        envelope: &EventEnvelope<T>,
    ) -> Result<(), MessagingError>;
}

#[async_trait]
impl<P: EventPublisher + ?Sized> EventPublisherExt for P {
    async fn publish_envelope<T: Serialize + Sync + Send>(
        &self,
        topic: &str,
        key: Option<&str>,
        envelope: &EventEnvelope<T>,
    ) -> Result<(), MessagingError> {
        let bytes = serde_json::to_vec(envelope)
            .map_err(|e| MessagingError::SerializationError(e.to_string()))?;
        self.publish(topic, key, &bytes).await
    }
}

/// Abstract contract for consuming events across consumer groups.
#[async_trait]
pub trait EventSubscriber: Send + Sync {
    fn subscriber_name(&self) -> &'static str;

    async fn subscribe(
        &self,
        topic: &str,
        group_id: &str,
        handler: Arc<dyn EventHandler>,
    ) -> Result<(), MessagingError>;
}

/// Factory to initialize publisher based on `MESSAGE_BUS_BACKEND` env var (`inmemory`, `kafka`).
pub async fn create_messaging_bus(
    backend: &str,
    _brokers: Option<&str>,
) -> Result<(Arc<dyn EventPublisher>, Arc<dyn EventSubscriber>), MessagingError> {
    match backend.to_lowercase().as_str() {
        "inmemory" | "in_memory" | "memory" | "local" => {
            let bus = Arc::new(in_memory::InMemoryEventBus::new());
            Ok((bus.clone(), bus))
        }
        #[cfg(feature = "kafka")]
        "kafka" => {
            let brokers = _brokers.unwrap_or("localhost:9092");
            let publisher = Arc::new(kafka::KafkaPublisher::new(brokers)?);
            let subscriber = Arc::new(kafka::KafkaSubscriber::new(brokers)?);
            Ok((publisher, subscriber))
        }
        other => Err(MessagingError::UnsupportedBackend(other.to_string())),
    }
}
