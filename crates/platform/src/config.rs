use crate::secrets::{SecretError, SecretProvider};
use std::sync::Arc;
use tracing::info;

/// 12-Factor Application Configuration (Factor III: Config stored strictly in environment).
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub app_env: String,
    pub port: u16,
    pub database_url: String,
    pub secrets_provider: String,
    pub message_bus_backend: String,
    pub kafka_brokers: Option<String>,
    pub auto_migrate: bool,
    pub audit_secret_key: String,
}

impl AppConfig {
    /// Loads configuration from environment variables and resolves references
    pub async fn load(secret_provider: &Arc<dyn SecretProvider>) -> Result<Self, SecretError> {
        let _ = dotenvy::dotenv();

        let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());
        let port = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);

        let raw_db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://postgres:postgres@localhost:5432/orders_db".to_string()
        });

        // Resolve secret:// references in database_url if present
        let database_url = secret_provider.resolve_reference(&raw_db_url).await?;

        let secrets_provider = std::env::var("SECRETS_PROVIDER").unwrap_or_else(|_| "env".to_string());
        let message_bus_backend = std::env::var("MESSAGE_BUS_BACKEND").unwrap_or_else(|_| "inmemory".to_string());
        let kafka_brokers = std::env::var("KAFKA_BROKERS").ok();
        let auto_migrate = std::env::var("AUTO_MIGRATE")
            .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
            .unwrap_or(true);

        let audit_secret_key = std::env::var("AUDIT_SECRET_KEY")
            .unwrap_or_else(|_| "APP_ENCRYPTION_KEY".to_string());

        info!(
            env = %app_env,
            port = port,
            secrets_provider = %secrets_provider,
            message_bus = %message_bus_backend,
            "Loaded 12-Factor application configuration"
        );

        Ok(Self {
            app_env,
            port,
            database_url,
            secrets_provider,
            message_bus_backend,
            kafka_brokers,
            auto_migrate,
            audit_secret_key,
        })
    }
}
