//! Registry client.

use crate::metadata::*;
use ferrite_core::FerriteError;
use ferrite_core::Result;
use std::path::PathBuf;

/// npm registry HTTP client with on-disk metadata caching.
#[derive(Debug, Clone)]
pub struct RegistryClient {
    /// HTTP client.
    client: reqwest::Client,
    /// Registry base URL.
    pub registry: String,
    /// Metadata cache directory.
    pub cache_dir: PathBuf,
}

impl RegistryClient {
    /// Create a client.
    pub fn new(registry: impl Into<String>, cache_dir: PathBuf) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(format!("ferrite/{}", ferrite_core::VERSION))
            .build()
            .map_err(|error| FerriteError::Npm(error.to_string()))?;
        Ok(Self {
            client,
            registry: registry.into().trim_end_matches('/').to_string(),
            cache_dir,
        })
    }

    /// Fetch (and cache) package metadata.
    pub async fn metadata(&self, name: &str) -> Result<RegistryMetadata> {
        crate::validate_package_name(name)?;
        let cache_path = self
            .cache_dir
            .join(format!("{}.json", name.replace('/', "__")));
        if let Ok(text) = std::fs::read_to_string(&cache_path) {
            if let Ok(cached) = serde_json::from_str(&text) {
                return Ok(cached);
            }
        }
        let url = format!("{}/{name}", self.registry);
        let response = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.npm.install-v1+json")
            .send()
            .await
            .map_err(|error| FerriteError::Npm(format!("registry request failed: {error}")))?;
        if !response.status().is_success() {
            return Err(FerriteError::Npm(format!(
                "registry returned {} for `{name}`",
                response.status()
            )));
        }
        let metadata: RegistryMetadata = response
            .json()
            .await
            .map_err(|error| FerriteError::Npm(format!("bad registry JSON: {error}")))?;
        if let Some(parent) = cache_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(
            &cache_path,
            serde_json::to_string(&metadata).unwrap_or_default(),
        );
        Ok(metadata)
    }

    /// Explicit update requests refresh metadata instead of indefinitely reusing cache.
    pub fn invalidate_metadata(&self, name: &str) -> Result<()> {
        crate::validate_package_name(name)?;
        let path = self
            .cache_dir
            .join(format!("{}.json", name.replace('/', "__")));
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Download a tarball.
    pub async fn tarball(&self, url: &str) -> Result<Vec<u8>> {
        let bytes = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| FerriteError::Npm(format!("tarball download failed: {error}")))?
            .error_for_status()
            .map_err(|error| FerriteError::Npm(format!("tarball HTTP failure: {error}")))?
            .bytes()
            .await
            .map_err(|error| FerriteError::Npm(format!("tarball read failed: {error}")))?;
        Ok(bytes.to_vec())
    }
}
