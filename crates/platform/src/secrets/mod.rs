pub mod env;
pub mod file;

#[cfg(feature = "aws")]
pub mod aws;

#[cfg(feature = "gcp")]
pub mod gcp;

use async_trait::async_trait;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("Secret not found for key: {0}")]
    NotFound(String),

    #[error("Permission denied reading secret for key: {0}: {1}")]
    AccessDenied(String, String),

    #[error("I/O error reading secret file: {0}")]
    Io(#[from] std::io::Error),

    #[error("Provider '{0}' error: {1}")]
    ProviderError(&'static str, String),

    #[error("Unsupported or uncompiled secrets provider: '{0}'. Rebuild with relevant cargo feature flags.")]
    UnsupportedProvider(String),
}

/// Abstract contract for secrets retrieval across environments:
/// - Bare metal / Local dev: Environment variables
/// - Kubernetes: Projected volume / Mounted secret files
/// - AWS: AWS Secrets Manager (EC2 IAM role, EKS IRSA, Nitro Enclave)
/// - GCP: GCP Secret Manager (GCE Service Account, Workload Identity, Confidential VM)
#[async_trait]
pub trait SecretProvider: Send + Sync {
    /// Human-readable identifier for the active provider
    fn provider_name(&self) -> &'static str;

    /// Fetches the raw secret string for a given key or secret name
    async fn get_secret(&self, key: &str) -> Result<String, SecretError>;

    /// Resolves a value if it uses the `secret://<key>` URI reference syntax.
    /// If not a reference, returns the input string unchanged.
    async fn resolve_reference(&self, value: &str) -> Result<String, SecretError> {
        if let Some(key) = value.strip_prefix("secret://") {
            self.get_secret(key).await
        } else {
            Ok(value.to_string())
        }
    }
}

/// Factory to initialize the configured SecretProvider based on environment variables or configuration.
pub async fn create_secret_provider(provider_type: &str) -> Result<Arc<dyn SecretProvider>, SecretError> {
    match provider_type.to_lowercase().as_str() {
        "env" | "environment" => {
            Ok(Arc::new(env::EnvSecretProvider::new()))
        }
        "file" | "k8s" | "kubernetes" | "volume" => {
            let base_path = std::env::var("SECRETS_DIR").unwrap_or_else(|_| "/etc/secrets".to_string());
            Ok(Arc::new(file::FileSecretProvider::new(base_path)))
        }
        #[cfg(feature = "aws")]
        "aws" | "aws-secrets-manager" => {
            let provider = aws::AwsSecretsManagerProvider::new().await?;
            Ok(Arc::new(provider))
        }
        #[cfg(feature = "gcp")]
        "gcp" | "gcp-secret-manager" => {
            let project_id = std::env::var("GCP_PROJECT_ID")
                .map_err(|_| SecretError::ProviderError("gcp", "GCP_PROJECT_ID env var required".into()))?;
            let provider = gcp::GcpSecretManagerProvider::new(project_id);
            Ok(Arc::new(provider))
        }
        other => Err(SecretError::UnsupportedProvider(other.to_string())),
    }
}
