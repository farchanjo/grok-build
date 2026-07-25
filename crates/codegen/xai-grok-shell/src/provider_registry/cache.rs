//! Per-provider catalog and capability caches with origin validation.

use super::id::ProviderId;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub const CATALOG_CACHE_VERSION: u32 = 1;
pub const CAPABILITY_CACHE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheOrigin {
    Live,
    Probe,
    Manual,
    LegacyMigration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheValidationError {
    VersionMismatch { found: u32, expected: u32 },
    ProviderMismatch { found: String, expected: String },
    OriginMismatch,
    Corrupt(String),
    BaselineMismatch { found: String, expected: String },
}

impl std::fmt::Display for CacheValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::VersionMismatch { found, expected } => {
                write!(f, "cache version {found} != expected {expected}")
            }
            Self::ProviderMismatch { found, expected } => {
                write!(f, "cache provider `{found}` != `{expected}`")
            }
            Self::OriginMismatch => write!(f, "cache origin validation failed"),
            Self::Corrupt(m) => write!(f, "corrupt cache: {m}"),
            Self::BaselineMismatch { found, expected } => {
                write!(f, "baseline `{found}` != expected `{expected}`")
            }
        }
    }
}

impl std::error::Error for CacheValidationError {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CatalogCacheEntry {
    pub version: u32,
    pub provider_id: String,
    pub origin: CacheOrigin,
    pub base_url_origin: String,
    pub fetched_at_unix: u64,
    pub models: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilityCacheEntry {
    pub version: u32,
    pub provider_id: String,
    pub origin: CacheOrigin,
    pub base_url_origin: String,
    pub baseline_version: String,
    pub fetched_at_unix: u64,
    pub capabilities: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<serde_json::Value>,
}

fn provider_cache_dir(grok_home: &Path, provider_id: &ProviderId) -> PathBuf {
    grok_home.join("provider_caches").join(provider_id.as_str())
}

fn catalog_path(grok_home: &Path, provider_id: &ProviderId) -> PathBuf {
    provider_cache_dir(grok_home, provider_id).join("catalog.json")
}

fn capability_path(grok_home: &Path, provider_id: &ProviderId) -> PathBuf {
    provider_cache_dir(grok_home, provider_id).join("capabilities.json")
}

/// Atomic owner-only write (mode 0600 on Unix).
fn atomic_write_owner_only(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
        }
    }
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

pub struct CatalogCacheStore;

impl CatalogCacheStore {
    pub fn load(
        grok_home: &Path,
        provider_id: &ProviderId,
        expected_origin_host: &str,
    ) -> Result<Option<CatalogCacheEntry>, CacheValidationError> {
        let path = catalog_path(grok_home, provider_id);
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(CacheValidationError::Corrupt(e.to_string())),
        };
        let entry: CatalogCacheEntry = serde_json::from_slice(&bytes)
            .map_err(|e| CacheValidationError::Corrupt(e.to_string()))?;
        if entry.version != CATALOG_CACHE_VERSION {
            return Err(CacheValidationError::VersionMismatch {
                found: entry.version,
                expected: CATALOG_CACHE_VERSION,
            });
        }
        if entry.provider_id != provider_id.as_str() {
            return Err(CacheValidationError::ProviderMismatch {
                found: entry.provider_id,
                expected: provider_id.as_str().to_owned(),
            });
        }
        if entry.base_url_origin != expected_origin_host {
            return Err(CacheValidationError::OriginMismatch);
        }
        Ok(Some(entry))
    }

    pub fn store(grok_home: &Path, entry: &CatalogCacheEntry) -> io::Result<()> {
        let provider_id = ProviderId::new(&entry.provider_id)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
        let path = catalog_path(grok_home, &provider_id);
        let bytes = serde_json::to_vec_pretty(entry)?;
        atomic_write_owner_only(&path, &bytes)
    }

    pub fn remove(grok_home: &Path, provider_id: &ProviderId) -> io::Result<()> {
        let path = catalog_path(grok_home, provider_id);
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}

pub struct CapabilityCacheStore;

impl CapabilityCacheStore {
    pub fn load(
        grok_home: &Path,
        provider_id: &ProviderId,
        expected_origin_host: &str,
        expected_baseline: &str,
    ) -> Result<Option<CapabilityCacheEntry>, CacheValidationError> {
        let path = capability_path(grok_home, provider_id);
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(CacheValidationError::Corrupt(e.to_string())),
        };
        let entry: CapabilityCacheEntry = serde_json::from_slice(&bytes)
            .map_err(|e| CacheValidationError::Corrupt(e.to_string()))?;
        if entry.version != CAPABILITY_CACHE_VERSION {
            return Err(CacheValidationError::VersionMismatch {
                found: entry.version,
                expected: CAPABILITY_CACHE_VERSION,
            });
        }
        if entry.provider_id != provider_id.as_str() {
            return Err(CacheValidationError::ProviderMismatch {
                found: entry.provider_id,
                expected: provider_id.as_str().to_owned(),
            });
        }
        if entry.base_url_origin != expected_origin_host {
            return Err(CacheValidationError::OriginMismatch);
        }
        if entry.baseline_version != expected_baseline {
            return Err(CacheValidationError::BaselineMismatch {
                found: entry.baseline_version,
                expected: expected_baseline.to_owned(),
            });
        }
        Ok(Some(entry))
    }

    pub fn store(grok_home: &Path, entry: &CapabilityCacheEntry) -> io::Result<()> {
        let provider_id = ProviderId::new(&entry.provider_id)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
        let path = capability_path(grok_home, &provider_id);
        let bytes = serde_json::to_vec_pretty(entry)?;
        atomic_write_owner_only(&path, &bytes)
    }

    pub fn remove(grok_home: &Path, provider_id: &ProviderId) -> io::Result<()> {
        let path = capability_path(grok_home, provider_id);
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}

/// Remove all caches for a provider (sibling isolation: only this id).
pub fn remove_all_provider_caches(grok_home: &Path, provider_id: &ProviderId) -> io::Result<()> {
    CatalogCacheStore::remove(grok_home, provider_id)?;
    CapabilityCacheStore::remove(grok_home, provider_id)?;
    let dir = provider_cache_dir(grok_home, provider_id);
    match fs::remove_dir_all(&dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn pid(s: &str) -> ProviderId {
        ProviderId::new(s).unwrap()
    }

    #[test]
    fn catalog_round_trip_and_sibling_isolation() {
        let dir = TempDir::new().unwrap();
        let a = pid("local_a");
        let b = pid("local_b");
        let entry = CatalogCacheEntry {
            version: CATALOG_CACHE_VERSION,
            provider_id: a.as_str().into(),
            origin: CacheOrigin::Live,
            base_url_origin: "http://127.0.0.1:8000".into(),
            fetched_at_unix: 1,
            models: vec![serde_json::json!({"id": "m"})],
            baseline_version: None,
        };
        CatalogCacheStore::store(dir.path(), &entry).unwrap();
        assert!(
            CatalogCacheStore::load(dir.path(), &a, "http://127.0.0.1:8000")
                .unwrap()
                .is_some()
        );
        assert!(
            CatalogCacheStore::load(dir.path(), &b, "http://127.0.0.1:8000")
                .unwrap()
                .is_none()
        );
        // Wrong origin fails closed.
        assert!(matches!(
            CatalogCacheStore::load(dir.path(), &a, "http://evil"),
            Err(CacheValidationError::OriginMismatch)
        ));
    }

    #[test]
    fn corrupt_cache_fails_closed() {
        let dir = TempDir::new().unwrap();
        let a = pid("local_a");
        let path = catalog_path(dir.path(), &a);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"{not json").unwrap();
        assert!(matches!(
            CatalogCacheStore::load(dir.path(), &a, "http://x"),
            Err(CacheValidationError::Corrupt(_))
        ));
    }
}
