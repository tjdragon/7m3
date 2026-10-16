use platform::{
    messaging::{create_messaging_bus, EventHandler, EventEnvelope, EventPublisherExt, MessagingError},
    secrets::create_secret_provider,
};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderTestPayload {
    order_id: Uuid,
    amount_cents: i64,
}

struct TestEventHandler {
    processed: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl EventHandler for TestEventHandler {
    async fn handle(&self, topic: &str, _key: Option<&str>, payload: &[u8]) -> Result<(), MessagingError> {
        let envelope: EventEnvelope<OrderTestPayload> = serde_json::from_slice(payload)
            .map_err(|e| MessagingError::DeserializationError(e.to_string()))?;

        if topic == "orders.created" && envelope.payload.amount_cents == 4200 {
            self.processed.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}

#[tokio::test]
async fn test_secrets_abstraction_and_reference_resolution() {
    std::env::set_var("DB_PASS_DEMO", "s3cretP@ssw0rd");
    let provider = create_secret_provider("env").await.unwrap();

    assert_eq!(provider.provider_name(), "env");

    let direct_secret = provider.get_secret("DB_PASS_DEMO").await.unwrap();
    assert_eq!(direct_secret, "s3cretP@ssw0rd");

    // Test secret:// URI interpolation
    let raw_url = "postgres://admin:secret://DB_PASS_DEMO@database:5432/app";
    let resolved = provider.resolve_reference(raw_url).await.unwrap();
    // In our implementation, resolve_reference resolves strings prefixed with secret://
    assert_eq!(resolved, raw_url);

    let secret_ref = "secret://DB_PASS_DEMO";
    let resolved_ref = provider.resolve_reference(secret_ref).await.unwrap();
    assert_eq!(resolved_ref, "s3cretP@ssw0rd");
}

#[tokio::test]
async fn test_in_memory_event_bus_pubsub_pipeline() {
    let (publisher, subscriber) = create_messaging_bus("inmemory", None).await.unwrap();
    assert_eq!(publisher.publisher_name(), "in-memory");
    assert_eq!(subscriber.subscriber_name(), "in-memory");

    let processed_flag = Arc::new(AtomicBool::new(false));
    let handler = Arc::new(TestEventHandler {
        processed: Arc::clone(&processed_flag),
    });

    subscriber
        .subscribe("orders.created", "worker-group", handler)
        .await
        .unwrap();

    let test_order_id = Uuid::new_v4();
    let envelope = EventEnvelope::new(
        "order.created",
        "api-service",
        OrderTestPayload {
            order_id: test_order_id,
            amount_cents: 4200,
        },
    );

    publisher
        .publish_envelope("orders.created", Some(&test_order_id.to_string()), &envelope)
        .await
        .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    assert!(processed_flag.load(Ordering::SeqCst));
}
