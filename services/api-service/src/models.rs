use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Order {
    pub id: Uuid,
    pub customer_name: String,
    pub amount_cents: i64,
    pub currency: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateOrderRequest {
    pub customer_name: String,
    pub amount_cents: i64,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCreatedPayload {
    pub order_id: Uuid,
    pub customer_name: String,
    pub amount_cents: i64,
    pub currency: String,
}

#[derive(Debug, Serialize)]
pub struct SecretsAuditResponse {
    pub provider: &'static str,
    pub secret_key: String,
    pub status: String,
    pub masked_value: Option<String>,
    pub explanation: String,
}

#[derive(Debug, Serialize)]
pub struct HealthStatus {
    pub status: String,
    pub database: String,
    pub messaging_backend: String,
    pub uptime_seconds: u64,
}
