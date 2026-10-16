use super::{SecretError, SecretProvider};
use async_trait::async_trait;
use aws_sdk_secretsmanager::Client;

/// AWS Secrets Manager integration.
/// Automatically resolves IAM credentials via:
/// - EC2 Instance Metadata Service (IMDSv2)
/// - EKS IAM Roles for Service Accounts (IRSA)
/// - AWS Nitro Enclaves / ECS Task Roles
pub struct AwsSecretsManagerProvider {
    client: Client,
}

impl AwsSecretsManagerProvider {
    pub async fn new() -> Result<Self, SecretError> {
        let config = aws_config::load_from_env().await;
        let client = Client::new(&config);
        Ok(Self { client })
    }
}

#[async_trait]
impl SecretProvider for AwsSecretsManagerProvider {
    fn provider_name(&self) -> &'static str {
        "aws-secrets-manager"
    }

    async fn get_secret(&self, key: &str) -> Result<String, SecretError> {
        let resp = self
            .client
            .get_secret_value()
            .secret_id(key)
            .send()
            .await
            .map_err(|e| {
                SecretError::ProviderError("aws-secrets-manager", e.to_string())
            })?;

        if let Some(secret_string) = resp.secret_string() {
            Ok(secret_string.to_string())
        } else if let Some(binary) = resp.secret_binary() {
            String::from_utf8(binary.as_ref().to_vec())
                .map_err(|e| SecretError::ProviderError("aws-secrets-manager", format!("Binary secret is not valid UTF-8: {}", e)))
        } else {
            Err(SecretError::NotFound(key.to_string()))
        }
    }
}
