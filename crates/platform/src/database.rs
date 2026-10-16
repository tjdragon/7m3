use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("Database connection failed: {0}")]
    ConnectionError(#[from] sqlx::Error),

    #[error("Database migration failed: {0}")]
    MigrationError(String),
}

/// Abstracted Postgres Database pool adhering to 12-Factor backing service guidelines (Factor IV).
/// Treats database connection as an attached resource with health probes and auto-recovery.
#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self, DbError> {
        let max_connections = std::env::var("DB_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20);

        let min_connections = std::env::var("DB_MIN_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);

        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .min_connections(min_connections)
            .acquire_timeout(Duration::from_secs(5))
            .idle_timeout(Duration::from_secs(600))
            .connect(database_url)
            .await?;

        info!(
            max_connections = max_connections,
            min_connections = min_connections,
            "Connected to PostgreSQL database pool"
        );

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Health check probe: executes `SELECT 1` against the database
    pub async fn ping(&self) -> Result<(), DbError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(DbError::ConnectionError)
    }

    /// Applies schema migrations (Factor XII: Admin processes)
    pub async fn run_migrations(&self) -> Result<(), DbError> {
        info!("Applying database migrations...");
        
        // Ensure the orders table exists idempotently
        sqlx::raw_sql(
            r#"
            CREATE TABLE IF NOT EXISTS orders (
                id UUID PRIMARY KEY,
                customer_name VARCHAR(255) NOT NULL,
                amount_cents BIGINT NOT NULL,
                currency VARCHAR(3) NOT NULL,
                status VARCHAR(50) NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_orders_status ON orders(status);
            "#
        )
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::MigrationError(e.to_string()))?;

        info!("Database migrations applied successfully");
        Ok(())
    }
}
