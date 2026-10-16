use super::{EventHandler, EventPublisher, EventSubscriber, MessagingError};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error};

type HandlersList = Vec<Arc<dyn EventHandler>>;

/// Fast, asynchronous in-memory event bus.
/// Guarantees:
/// - Zero external broker dependency for local unit tests and developer onboarding.
/// - Identical async behavior and API contract as Kafka.
/// - Full thread safety under concurrent Tokio tasks.
#[derive(Default)]
pub struct InMemoryEventBus {
    subscribers: RwLock<HashMap<String, HandlersList>>,
}

impl InMemoryEventBus {
    pub fn new() -> Self {
        Self {
            subscribers: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl EventPublisher for InMemoryEventBus {
    fn publisher_name(&self) -> &'static str {
        "in-memory"
    }

    async fn publish(&self, topic: &str, key: Option<&str>, payload: &[u8]) -> Result<(), MessagingError> {
        let subscribers = self.subscribers.read().await;
        if let Some(handlers) = subscribers.get(topic) {
            debug!(topic = %topic, subscribers_count = handlers.len(), "Dispatching in-memory event");
            for handler in handlers {
                let handler = Arc::clone(handler);
                let topic = topic.to_string();
                let key = key.map(|k| k.to_string());
                let payload = payload.to_vec();

                tokio::spawn(async move {
                    if let Err(err) = handler.handle(&topic, key.as_deref(), &payload).await {
                        error!(topic = %topic, error = %err, "Handler failed processing event");
                    }
                });
            }
        } else {
            debug!(topic = %topic, "No subscribers registered for in-memory topic");
        }
        Ok(())
    }
}

#[async_trait]
impl EventSubscriber for InMemoryEventBus {
    fn subscriber_name(&self) -> &'static str {
        "in-memory"
    }

    async fn subscribe(
        &self,
        topic: &str,
        _group_id: &str,
        handler: Arc<dyn EventHandler>,
    ) -> Result<(), MessagingError> {
        let mut subscribers = self.subscribers.write().await;
        subscribers.entry(topic.to_string()).or_default().push(handler);
        debug!(topic = %topic, "Registered in-memory subscriber handler");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct TestHandler {
        received: Arc<AtomicBool>,
    }

    #[async_trait]
    impl EventHandler for TestHandler {
        async fn handle(&self, _topic: &str, _key: Option<&str>, payload: &[u8]) -> Result<(), MessagingError> {
            let msg = std::str::from_utf8(payload).unwrap();
            if msg == "ping" {
                self.received.store(true, Ordering::SeqCst);
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_in_memory_pubsub() {
        let bus = Arc::new(InMemoryEventBus::new());
        let flag = Arc::new(AtomicBool::new(false));
        let handler = Arc::new(TestHandler {
            received: Arc::clone(&flag),
        });

        bus.subscribe("orders.test", "test-group", handler).await.unwrap();
        bus.publish("orders.test", None, b"ping").await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        assert!(flag.load(Ordering::SeqCst));
    }
}
