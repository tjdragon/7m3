use super::{SecretError, SecretProvider};
use async_trait::async_trait;

/// Reads secrets directly from process environment variables.
/// Best suited for:
/// - Local developer workflows (.env files)
/// - Bare metal deployments with systemd EnvironmentFile
/// - Lightweight containers
#[derive(Debug, Default, Clone)]
pub struct EnvSecretProvider;

impl EnvSecretProvider {
    pub fn new() -> Self {
        Self
    }

    fn normalize_key(key: &str) -> String {
        key.to_uppercase().replace(['.', '-', '/'], "_")
    }
}

#[async_trait]
impl SecretProvider for EnvSecretProvider {
    fn provider_name(&self) -> &'static str {
        "env"
    }

    async fn get_secret(&self, key: &str) -> Result<String, SecretError> {
        // Try exact key first
        if let Ok(val) = std::env::var(key) {
            return Ok(val);
        }

        // Try normalized key (e.g. "db-password" -> "DB_PASSWORD")
        let normalized = Self::normalize_key(key);
        if let Ok(val) = std::env::var(&normalized) {
            return Ok(val);
        }

        Err(SecretError::NotFound(key.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_env_secret_lookup() {
        std::env::set_var("TEST_DB_PASSWORD", "supersecret123");
        let provider = EnvSecretProvider::new();
        let val = provider.get_secret("test-db-password").await.unwrap();
        assert_eq!(val, "supersecret123");
    }
}
