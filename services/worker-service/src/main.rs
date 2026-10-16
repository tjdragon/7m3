mod processor;

use axum::{http::StatusCode, routing::get, Json, Router};
use platform::{
    lifecycle::ShutdownSignal, messaging::create_messaging_bus, secrets::create_secret_provider,
    telemetry::init_telemetry, AppConfig, Database,
};
use processor::OrderProcessor;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Factor XI: Logs as event streams
    init_telemetry("info,platform=debug,worker_service=debug");

    info!("Starting 7ME3 Worker Service (Downstream Processing / 12-Factor Concurrency)...");

    // Factor III: Config in Environment & Secrets Abstraction
    let secrets_provider_type = std::env::var("SECRETS_PROVIDER").unwrap_or_else(|_| "env".to_string());
    let secret_provider = create_secret_provider(&secrets_provider_type).await?;
    let config = AppConfig::load(&secret_provider).await?;

    // Factor IV: Backing Services - Database (Postgres)
    let db = Database::connect(&config.database_url).await?;

    if config.auto_migrate {
        db.run_migrations().await?;
    }

    // Factor IV: Backing Services - Messaging Subscriber (Kafka / In-Memory)
    let (_publisher, subscriber) = create_messaging_bus(
        &config.message_bus_backend,
        config.kafka_brokers.as_deref(),
    )
    .await?;

    let processor = Arc::new(OrderProcessor::new(db.clone()));

    // Subscribe to orders topic
    subscriber
        .subscribe("orders.created", "worker-service-group", processor)
        .await?;

    info!(
        topic = "orders.created",
        backend = subscriber.subscriber_name(),
        "Worker successfully subscribed and listening for events"
    );

    // Factor VII: Port binding for Worker Health Probes (Factor IX & K8s)
    let health_port = std::env::var("WORKER_HEALTH_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8081);

    let health_db = db.clone();
    let health_backend = subscriber.subscriber_name().to_string();

    let health_app = Router::new()
        .route(
            "/health/live",
            get(|| async { (StatusCode::OK, Json(serde_json::json!({ "status": "UP" }))) }),
        )
        .route(
            "/health/ready",
            get(move || {
                let db = health_db.clone();
                let backend = health_backend.clone();
                async move {
                    if let Err(e) = db.ping().await {
                        return (
                            StatusCode::SERVICE_UNAVAILABLE,
                            Json(serde_json::json!({
                                "status": "NOT_READY",
                                "database": "UNAVAILABLE",
                                "error": e.to_string(),
                            })),
                        );
                    }
                    (
                        StatusCode::OK,
                        Json(serde_json::json!({
                            "status": "READY",
                            "database": "CONNECTED",
                            "messaging_backend": backend,
                        })),
                    )
                }
            }),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], health_port));
    info!(addr = %addr, "Worker health HTTP server listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Factor IX: Disposability (Graceful shutdown)
    axum::serve(listener, health_app)
        .with_graceful_shutdown(ShutdownSignal::listen())
        .await?;

    info!("Worker service shut down gracefully");
    Ok(())
}
