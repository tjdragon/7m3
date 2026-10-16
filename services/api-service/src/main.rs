mod handlers;
mod models;

use axum::{
    routing::{get, post},
    Router,
};
use handlers::AppState;
use platform::{
    lifecycle::ShutdownSignal, messaging::create_messaging_bus, secrets::create_secret_provider,
    telemetry::init_telemetry, AppConfig, Database,
};
use std::net::SocketAddr;
use std::time::Instant;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Factor XI: Logs as event streams
    init_telemetry("info,platform=debug,api_service=debug");

    info!("Starting 7ME3 API Service (Cloud-Agnostic 12-Factor Architecture)...");

    // Factor III: Config in Environment & Secrets Abstraction
    let secrets_provider_type = std::env::var("SECRETS_PROVIDER").unwrap_or_else(|_| "env".to_string());
    let secret_provider = create_secret_provider(&secrets_provider_type).await?;
    let config = AppConfig::load(&secret_provider).await?;

    // Factor IV: Backing Services - Database (Postgres)
    let db = Database::connect(&config.database_url).await?;

    // Factor XII: Admin Processes - Database Migrations
    if config.auto_migrate {
        db.run_migrations().await?;
    }

    // Factor IV: Backing Services - Abstracted Messaging (Kafka / In-Memory)
    let (publisher, _subscriber) = create_messaging_bus(
        &config.message_bus_backend,
        config.kafka_brokers.as_deref(),
    )
    .await?;

    let state = AppState {
        db,
        publisher,
        secret_provider,
        config: config.clone(),
        start_time: Instant::now(),
    };

    // Factor VII: Port Binding & Routing
    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);

    let app = Router::new()
        .route("/api/orders", post(handlers::create_order).get(handlers::list_orders))
        .route("/api/orders/{id}", get(handlers::get_order))
        .route("/api/system/secrets-audit", get(handlers::secrets_audit))
        .route("/health/live", get(handlers::health_live))
        .route("/health/ready", get(handlers::health_ready))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!(addr = %addr, "Binding HTTP server to port");

    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Factor IX: Disposability (Fast startup & Graceful shutdown)
    axum::serve(listener, app)
        .with_graceful_shutdown(ShutdownSignal::listen())
        .await?;

    info!("API Service shut down gracefully");
    Ok(())
}
