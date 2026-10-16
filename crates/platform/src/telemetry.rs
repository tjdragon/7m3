use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initializes structured logging according to 12-Factor principles (Factor XI: Logs as event streams).
///
/// If `LOG_FORMAT=json` is set, emits newline-delimited JSON logs for cloud aggregation
/// (e.g. AWS CloudWatch, GCP Cloud Logging, Datadog).
/// Otherwise, outputs human-readable colored logs for local development.
pub fn init_telemetry(default_level: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_level));

    let log_format = std::env::var("LOG_FORMAT").unwrap_or_else(|_| "pretty".to_string());

    if log_format.eq_ignore_ascii_case("json") {
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer().pretty())
            .init();
    }
}
