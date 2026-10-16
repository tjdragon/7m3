use super::{EventHandler, EventPublisher, EventSubscriber, MessagingError};
use async_trait::async_trait;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::Message;
use rdkafka::producer::{FutureProducer, FutureRecord};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};

pub struct KafkaPublisher {
    producer: FutureProducer,
}

impl KafkaPublisher {
    pub fn new(brokers: &str) -> Result<Self, MessagingError> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("message.timeout.ms", "5000")
            .create()
            .map_err(|e| MessagingError::PublishFailed("config".into(), e.to_string()))?;

        Ok(Self { producer })
    }
}

#[async_trait]
impl EventPublisher for KafkaPublisher {
    fn publisher_name(&self) -> &'static str {
        "kafka"
    }

    async fn publish(&self, topic: &str, key: Option<&str>, payload: &[u8]) -> Result<(), MessagingError> {
        let mut record = FutureRecord::to(topic).payload(payload);
        if let Some(k) = key {
            record = record.key(k);
        }

        self.producer
            .send(record, Duration::from_secs(5))
            .await
            .map_err(|(e, _)| MessagingError::PublishFailed(topic.to_string(), e.to_string()))?;

        Ok(())
    }
}

pub struct KafkaSubscriber {
    brokers: String,
}

impl KafkaSubscriber {
    pub fn new(brokers: impl Into<String>) -> Result<Self, MessagingError> {
        Ok(Self {
            brokers: brokers.into(),
        })
    }
}

#[async_trait]
impl EventSubscriber for KafkaSubscriber {
    fn subscriber_name(&self) -> &'static str {
        "kafka"
    }

    async fn subscribe(
        &self,
        topic: &str,
        group_id: &str,
        handler: Arc<dyn EventHandler>,
    ) -> Result<(), MessagingError> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("group.id", group_id)
            .set("bootstrap.servers", &self.brokers)
            .set("enable.auto.commit", "true")
            .set("auto.offset.reset", "earliest")
            .create()
            .map_err(|e| MessagingError::SubscribeFailed(topic.to_string(), e.to_string()))?;

        consumer
            .subscribe(&[topic])
            .map_err(|e| MessagingError::SubscribeFailed(topic.to_string(), e.to_string()))?;

        info!(topic = %topic, group_id = %group_id, "Connected Kafka stream consumer");

        let topic_str = topic.to_string();
        tokio::spawn(async move {
            loop {
                match consumer.recv().await {
                    Err(e) => {
                        error!(error = %e, "Kafka consumer receive error");
                    }
                    Ok(m) => {
                        let payload = m.payload().unwrap_or_default();
                        let key = m.key().and_then(|k| std::str::from_utf8(k).ok());

                        if let Err(e) = handler.handle(&topic_str, key, payload).await {
                            error!(error = %e, "Kafka message handler error");
                        }
                    }
                }
            }
        });

        Ok(())
    }
}
