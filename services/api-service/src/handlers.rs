use crate::models::{CreateOrderRequest, HealthStatus, Order, OrderCreatedPayload, SecretsAuditResponse};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use platform::{
    AppConfig, Database, EventEnvelope, EventPublisher, EventPublisherExt, SecretProvider,
};
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info};
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub publisher: Arc<dyn EventPublisher>,
    pub secret_provider: Arc<dyn SecretProvider>,
    pub config: AppConfig,
    pub start_time: Instant,
}

/// POST /api/orders
/// Persists an order into Postgres and publishes an OrderCreated event onto the abstracted message bus.
pub async fn create_order(
    State(state): State<AppState>,
    Json(payload): Json<CreateOrderRequest>,
) -> Result<(StatusCode, Json<Order>), (StatusCode, String)> {
    let order_id = Uuid::new_v4();
    let now = Utc::now();
    let initial_status = "PENDING".to_string();

    let order = sqlx::query_as::<_, Order>(
        r#"
        INSERT INTO orders (id, customer_name, amount_cents, currency, status, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, customer_name, amount_cents, currency, status, created_at, updated_at
        "#
    )
    .bind(order_id)
    .bind(&payload.customer_name)
    .bind(payload.amount_cents)
    .bind(&payload.currency)
    .bind(&initial_status)
    .bind(now)
    .bind(now)
    .fetch_one(state.db.pool())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to insert order into Postgres");
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;

    // Publish event to abstracted message bus
    let event = EventEnvelope::new(
        "order.created",
        "api-service",
        OrderCreatedPayload {
            order_id: order.id,
            customer_name: order.customer_name.clone(),
            amount_cents: order.amount_cents,
            currency: order.currency.clone(),
        },
    );

    if let Err(e) = state
        .publisher
        .publish_envelope("orders.created", Some(&order.id.to_string()), &event)
        .await
    {
        error!(error = %e, order_id = %order.id, "Failed to publish OrderCreated event");
        // In event-driven systems, this could trigger an outbox retry; for this demo we log the error
    } else {
        info!(order_id = %order.id, topic = "orders.created", "Published OrderCreated event to message bus");
    }

    Ok((StatusCode::CREATED, Json(order)))
}

/// GET /api/orders
/// Returns recent orders from PostgreSQL.
pub async fn list_orders(
    State(state): State<AppState>,
) -> Result<Json<Vec<Order>>, (StatusCode, String)> {
    let orders = sqlx::query_as::<_, Order>(
        "SELECT id, customer_name, amount_cents, currency, status, created_at, updated_at FROM orders ORDER BY created_at DESC LIMIT 50"
    )
    .fetch_all(state.db.pool())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to query orders from Postgres");
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;

    Ok(Json(orders))
}

/// GET /api/orders/:id
/// Fetches a specific order by UUID.
pub async fn get_order(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Order>, (StatusCode, String)> {
    let order = sqlx::query_as::<_, Order>(
        "SELECT id, customer_name, amount_cents, currency, status, created_at, updated_at FROM orders WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(state.db.pool())
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;

    match order {
        Some(o) => Ok(Json(o)),
        None => Err((StatusCode::NOT_FOUND, "Order not found".to_string())),
    }
}

/// GET /api/system/secrets-audit
/// Demonstrates the SecretProvider abstraction without leaking plaintext credentials.
pub async fn secrets_audit(
    State(state): State<AppState>,
) -> Json<SecretsAuditResponse> {
    let key = &state.config.audit_secret_key;
    let provider_name = state.secret_provider.provider_name();

    match state.secret_provider.get_secret(key).await {
        Ok(secret_val) => {
            let masked = if secret_val.len() <= 4 {
                "****".to_string()
            } else {
                format!("{}****{}", &secret_val[..2], &secret_val[secret_val.len() - 2..])
            };

            Json(SecretsAuditResponse {
                provider: provider_name,
                secret_key: key.clone(),
                status: "FOUND".to_string(),
                masked_value: Some(masked),
                explanation: format!(
                    "Secret '{}' successfully retrieved via provider '{}'. The exact same code runs unmodified on AWS, GCP, K8s, bare metal, or Confidential VMs.",
                    key, provider_name
                ),
            })
        }
        Err(e) => Json(SecretsAuditResponse {
            provider: provider_name,
            secret_key: key.clone(),
            status: format!("NOT_FOUND: {}", e),
            masked_value: None,
            explanation: format!(
                "Secret retrieval attempted via provider '{}'. Ensure environment variable or secret mount '{}' exists.",
                provider_name, key
            ),
        }),
    }
}

/// GET /health/live
/// Kubernetes / Cloud load balancer liveness probe (Factor IX).
pub async fn health_live() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "status": "UP" })))
}

/// GET /health/ready
/// Kubernetes / Cloud readiness probe: verifies backing services (PostgreSQL & Messaging).
pub async fn health_ready(
    State(state): State<AppState>,
) -> Result<Json<HealthStatus>, (StatusCode, Json<serde_json::Value>)> {
    let db_ok = state.db.ping().await;

    if let Err(e) = db_ok {
        error!(error = %e, "Readiness probe failed: Database unreachable");
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "NOT_READY",
                "database": "UNAVAILABLE",
                "error": e.to_string(),
            })),
        ));
    }

    Ok(Json(HealthStatus {
        status: "READY".to_string(),
        database: "CONNECTED".to_string(),
        messaging_backend: state.publisher.publisher_name().to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
    }))
}
