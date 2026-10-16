use async_trait::async_trait;
use chrono::Utc;
use platform::{Database, EventHandler, EventEnvelope, MessagingError};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCreatedPayload {
    pub order_id: Uuid,
    pub customer_name: String,
    pub amount_cents: i64,
    pub currency: String,
}

pub struct OrderProcessor {
    db: Database,
}

impl OrderProcessor {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl EventHandler for OrderProcessor {
    async fn handle(&self, topic: &str, key: Option<&str>, payload: &[u8]) -> Result<(), MessagingError> {
        let envelope: EventEnvelope<OrderCreatedPayload> = serde_json::from_slice(payload)
            .map_err(|e| MessagingError::DeserializationError(e.to_string()))?;

        info!(
            event_id = %envelope.id,
            topic = %topic,
            key = ?key,
            order_id = %envelope.payload.order_id,
            customer = %envelope.payload.customer_name,
            amount = envelope.payload.amount_cents,
            "Worker received OrderCreated event from message bus"
        );

        // Step 1: Update status to 'PROCESSING' in Postgres
        let processing_status = "PROCESSING";
        let now = Utc::now();
        if let Err(e) = sqlx::query(
            "UPDATE orders SET status = $1, updated_at = $2 WHERE id = $3"
        )
        .bind(processing_status)
        .bind(now)
        .bind(envelope.payload.order_id)
        .execute(self.db.pool())
        .await
        {
            error!(error = %e, order_id = %envelope.payload.order_id, "Failed to update order status to PROCESSING");
            return Err(MessagingError::PublishFailed(topic.to_string(), e.to_string()));
        }

        // Step 2: Simulate business processing (e.g. payment gateway validation, inventory allocation)
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Step 3: Complete the order
        let completed_status = "COMPLETED";
        let completed_at = Utc::now();
        match sqlx::query(
            "UPDATE orders SET status = $1, updated_at = $2 WHERE id = $3"
        )
        .bind(completed_status)
        .bind(completed_at)
        .bind(envelope.payload.order_id)
        .execute(self.db.pool())
        .await
        {
            Ok(result) => {
                if result.rows_affected() > 0 {
                    info!(
                        order_id = %envelope.payload.order_id,
                        status = "COMPLETED",
                        "Order successfully verified, processed, and marked COMPLETED in database"
                    );
                } else {
                    warn!(
                        order_id = %envelope.payload.order_id,
                        "Order was not found in database during completion step"
                    );
                }
            }
            Err(e) => {
                error!(error = %e, order_id = %envelope.payload.order_id, "Failed to mark order as COMPLETED");
                return Err(MessagingError::PublishFailed(topic.to_string(), e.to_string()));
            }
        }

        Ok(())
    }
}
