use super::{SecretError, SecretProvider};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::Deserialize;

/// GCP Secret Manager implementation.
/// Authenticates seamlessly on:
/// - Google Compute Engine (GCE) via Instance Metadata Service
/// - Google Kubernetes Engine (GKE) via Workload Identity
/// - GCP Confidential VMs (SEV / SEV-SNP protected instances)
/// - Local environments with `GOOGLE_OAUTH_TOKEN`
#[derive(Debug, Clone)]
pub struct GcpSecretManagerProvider {
    project_id: String,
    http_client: reqwest::Client,
}

#[derive(Deserialize)]
struct GcpSecretPayload {
    data: String,
}

#[derive(Deserialize)]
struct GcpSecretAccessResponse {
    payload: GcpSecretPayload,
}

#[derive(Deserialize)]
struct MetadataTokenResponse {
    access_token: String,
}

impl GcpSecretManagerProvider {
    pub fn new(project_id: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            http_client: reqwest::Client::new(),
        }
    }

    async fn get_access_token(&self) -> Result<String, SecretError> {
        // If an explicit token is provided in env (e.g. for CI or local service account)
        if let Ok(token) = std::env::var("GOOGLE_OAUTH_TOKEN") {
            return Ok(token);
        }

        // Fetch from GCE / GKE metadata server
        let metadata_url = "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";
        let mut headers = HeaderMap::new();
        headers.insert("Metadata-Flavor", HeaderValue::from_static("Google"));

        let resp = self
            .http_client
            .get(metadata_url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| SecretError::ProviderError("gcp-secret-manager", format!("Failed to reach GCP metadata server: {}", e)))?;

        if !resp.status().is_success() {
            return Err(SecretError::ProviderError(
                "gcp-secret-manager",
                format!("Metadata server returned error status: {}", resp.status()),
            ));
        }

        let token_resp: MetadataTokenResponse = resp.json().await.map_err(|e| {
            SecretError::ProviderError("gcp-secret-manager", format!("Invalid token response: {}", e))
        })?;

        Ok(token_resp.access_token)
    }
}

#[async_trait]
impl SecretProvider for GcpSecretManagerProvider {
    fn provider_name(&self) -> &'static str {
        "gcp-secret-manager"
    }

    async fn get_secret(&self, key: &str) -> Result<String, SecretError> {
        let token = self.get_access_token().await?;
        let url = format!(
            "https://secretmanager.googleapis.com/v1/projects/{}/secrets/{}/versions/latest:access",
            self.project_id, key
        );

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token))
                .map_err(|e| SecretError::ProviderError("gcp-secret-manager", e.to_string()))?,
        );

        let resp = self
            .http_client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| SecretError::ProviderError("gcp-secret-manager", e.to_string()))?;

        if resp.status().as_u16() == 404 {
            return Err(SecretError::NotFound(key.to_string()));
        }

        if !resp.status().is_success() {
            return Err(SecretError::ProviderError(
                "gcp-secret-manager",
                format!("GCP Secret Manager API returned error: {}", resp.status()),
            ));
        }

        let body: GcpSecretAccessResponse = resp.json().await.map_err(|e| {
            SecretError::ProviderError("gcp-secret-manager", format!("Failed to parse response: {}", e))
        })?;

        use base64::Engine;
        let decoded_bytes = base64::engine::general_purpose::STANDARD
            .decode(&body.payload.data)
            .map_err(|e| SecretError::ProviderError("gcp-secret-manager", format!("Base64 decode failed: {}", e)))?;

        String::from_utf8(decoded_bytes).map_err(|e| {
            SecretError::ProviderError("gcp-secret-manager", format!("Secret payload is not valid UTF-8: {}", e))
        })
    }
}
