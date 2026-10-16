use tokio::signal;
use tracing::info;

/// Graceful shutdown listener for 12-Factor disposability (Factor IX).
/// Handles both SIGINT (Ctrl+C) and SIGTERM (Kubernetes/Docker/systemd stop signals).
pub struct ShutdownSignal;

impl ShutdownSignal {
    pub async fn listen() {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C signal handler");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("Failed to install SIGTERM signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {
                info!("Received SIGINT (Ctrl+C), initiating graceful shutdown");
            },
            _ = terminate => {
                info!("Received SIGTERM, initiating graceful shutdown");
            },
        }
    }
}
