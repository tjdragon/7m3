use super::{SecretError, SecretProvider};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Reads secrets from files in a mounted directory.
/// Best suited for:
/// - Kubernetes Secret Volume Mounts (`/etc/secrets/<key>`)
/// - Docker Swarm / Container secrets (`/run/secrets/<key>`)
/// - Confidential VMs / Secure Enclaves (e.g. AWS Nitro Enclaves or AMD SEV-SNP tmpfs ramdisks
///   where attested secrets are projected into encrypted memory)
#[derive(Debug, Clone)]
pub struct FileSecretProvider {
    base_dir: PathBuf,
}

impl FileSecretProvider {
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl SecretProvider for FileSecretProvider {
    fn provider_name(&self) -> &'static str {
        "file/k8s-volume"
    }

    async fn get_secret(&self, key: &str) -> Result<String, SecretError> {
        let file_path = self.base_dir.join(key);

        if !file_path.exists() {
            // Also try normalized key (e.g., db_password vs db-password)
            let alt_path = self.base_dir.join(key.replace(['-', '.'], "_"));
            if alt_path.exists() {
                let content = fs::read_to_string(&alt_path).await?;
                return Ok(content.trim().to_string());
            }

            return Err(SecretError::NotFound(format!(
                "{} (searched path: {})",
                key,
                file_path.display()
            )));
        }

        let content = fs::read_to_string(&file_path).await?;
        Ok(content.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_file_secret_lookup() {
        let tmp_dir = std::env::temp_dir().join(format!("secrets_test_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&tmp_dir).await.unwrap();
        tokio::fs::write(tmp_dir.join("api-key"), "file-secret-value\n").await.unwrap();

        let provider = FileSecretProvider::new(&tmp_dir);
        let val = provider.get_secret("api-key").await.unwrap();
        assert_eq!(val, "file-secret-value");

        tokio::fs::remove_dir_all(&tmp_dir).await.unwrap();
    }
}
