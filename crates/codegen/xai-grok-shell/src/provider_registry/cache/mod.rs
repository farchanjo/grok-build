//! Incarnation-safe per-instance provider catalog/capability/lifecycle cache.
//!
//! Authoritative layout:
//! ```text
//! $GROK_HOME/provider_caches/<validated-instance-id>/
//!   catalog.json
//!   capabilities.json
//!   state.json
//!   provider_cache.lock
//!   provider_cache.txn
//! ```
//!
//! Legacy singleton files under `$GROK_HOME` remain non-destructive import
//! sources / old-reader projections. Automatic migration is copy-only: never
//! rename, delete, truncate, or rewrite a legacy file merely on read/import.
//! Built-in caches are never imported into a configured sibling.

mod fs;
mod identity;

#[cfg(test)]
mod tests;

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::id::{BuiltInProviderId, ProviderId};
use super::instance::{ApiSurface, CredentialRoute, ProviderIncarnation, ProviderKind};
use fs::{
    CAPABILITIES_FILE, CATALOG_FILE, InstanceLock, MAX_CAPABILITIES_BYTES, MAX_CATALOG_BYTES,
    MAX_STATE_BYTES, MAX_TXN_BYTES, STATE_FILE, TXN_FILE, TrustedInstanceDir, content_hash,
    file_hash_relative, invalid_data, is_valid_staged_temp_name, read_home_regular_nofollow,
    read_optional_regular_relative, remove_instance_dir_locked, rename_relative,
    stage_bytes_relative, unlink_relative,
};
pub use identity::{
    CredentialBindingId, FingerprintError, OriginNormalizeError, ProviderCacheIdentity,
    normalize_endpoint_origin, org_project_fingerprint, validate_org_project_fingerprint,
};

pub const CATALOG_CACHE_VERSION: u32 = 1;
pub const CAPABILITY_CACHE_VERSION: u32 = 1;
pub const STATE_CACHE_VERSION: u32 = 1;
pub const TXN_VERSION: u8 = 1;

/// How a catalog/capability payload was produced (secret-free).
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
    KindMismatch,
    SurfaceMismatch,
    RouteMismatch,
    IncarnationMismatch,
    BindingMismatch,
    OrgProjectMismatch,
    Tombstoned,
    Corrupt(String),
    BaselineMismatch { found: String, expected: String },
    Io(String),
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
            Self::KindMismatch => write!(f, "cache provider kind mismatch"),
            Self::SurfaceMismatch => write!(f, "cache api surface mismatch"),
            Self::RouteMismatch => write!(f, "cache credential route mismatch"),
            Self::IncarnationMismatch => write!(f, "cache incarnation mismatch"),
            Self::BindingMismatch => write!(f, "cache credential binding mismatch"),
            Self::OrgProjectMismatch => write!(f, "cache org/project fingerprint mismatch"),
            Self::Tombstoned => write!(f, "provider cache is tombstoned for this instance"),
            Self::Corrupt(m) => write!(f, "corrupt cache: {m}"),
            Self::BaselineMismatch { found, expected } => {
                write!(f, "baseline `{found}` != expected `{expected}`")
            }
            Self::Io(m) => write!(f, "cache io: {m}"),
        }
    }
}

impl std::error::Error for CacheValidationError {}

impl From<io::Error> for CacheValidationError {
    fn from(value: io::Error) -> Self {
        match value.kind() {
            io::ErrorKind::InvalidData | io::ErrorKind::InvalidInput => {
                Self::Corrupt(value.to_string())
            }
            _ => Self::Io(value.to_string()),
        }
    }
}

/// Authoritative catalog cache envelope (additive over the original public shape).
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incarnation: Option<ProviderIncarnation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_kind: Option<ProviderKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_surface: Option<ApiSurface>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_route: Option<CredentialRoute>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_binding_id: Option<CredentialBindingId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org_project_fingerprint: Option<String>,
    #[serde(default)]
    pub catalog_generation: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle_generation: Option<u64>,
}

/// Authoritative capability cache envelope.
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incarnation: Option<ProviderIncarnation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_kind: Option<ProviderKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_surface: Option<ApiSurface>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_route: Option<CredentialRoute>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_binding_id: Option<CredentialBindingId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org_project_fingerprint: Option<String>,
    #[serde(default)]
    pub capability_generation: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle_generation: Option<u64>,
}

/// Grok-owned per-instance lifecycle/cache state (never secrets).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderCacheState {
    pub schema_version: u32,
    pub provider_instance_id: String,
    pub incarnation: ProviderIncarnation,
    pub provider_kind: ProviderKind,
    pub api_surface: ApiSurface,
    pub credential_route: CredentialRoute,
    pub endpoint_origin: String,
    #[serde(default)]
    pub org_project_fingerprint: String,
    pub credential_binding_id: CredentialBindingId,
    #[serde(default)]
    pub catalog_generation: u64,
    #[serde(default)]
    pub capability_generation: u64,
    #[serde(default)]
    pub lifecycle_generation: u64,
    #[serde(default)]
    pub tombstoned: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_import: Option<LegacyImportMarker>,
    pub updated_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LegacyImportMarker {
    pub source: String,
    pub imported_at_unix: u64,
    pub source_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TxnMarker {
    version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    catalog_tmp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    capabilities_tmp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    state_tmp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    catalog_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    capabilities_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    state_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous_catalog_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous_capabilities_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous_state_sha256: Option<String>,
    #[serde(default)]
    clear_catalog: bool,
    #[serde(default)]
    clear_capabilities: bool,
    #[serde(default)]
    clear_all: bool,
    #[serde(default)]
    partial_compat: bool,
}

// ---------------------------------------------------------------------------
// Failpoints
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderCacheTxnFault {
    AfterJournalFsync,
    AfterCatalogRename,
    AfterCapabilitiesRename,
    AfterStateRename,
    BeforeMarkerRemoval,
}

#[cfg(test)]
mod fault {
    use super::ProviderCacheTxnFault;
    use std::cell::Cell;

    thread_local! {
        static FAULT: Cell<Option<ProviderCacheTxnFault>> = const { Cell::new(None) };
    }

    pub struct FaultGuard;
    impl FaultGuard {
        pub fn arm(fault: ProviderCacheTxnFault) -> Self {
            FAULT.with(|c| c.set(Some(fault)));
            Self
        }
    }
    impl Drop for FaultGuard {
        fn drop(&mut self) {
            FAULT.with(|c| c.set(None));
        }
    }
    pub fn take_if(expected: ProviderCacheTxnFault) -> bool {
        FAULT.with(|c| {
            if c.get() == Some(expected) {
                c.set(None);
                true
            } else {
                false
            }
        })
    }
}

#[cfg(test)]
fn maybe_fault(point: ProviderCacheTxnFault) -> io::Result<()> {
    if fault::take_if(point) {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("provider cache failpoint: {point:?}"),
        ));
    }
    Ok(())
}

#[cfg(not(test))]
fn maybe_fault(_point: ProviderCacheTxnFault) -> io::Result<()> {
    Ok(())
}

// ---------------------------------------------------------------------------
// Authoritative store
// ---------------------------------------------------------------------------

pub struct ProviderCacheStore;

impl ProviderCacheStore {
    pub fn load_catalog(
        grok_home: &Path,
        identity: &ProviderCacheIdentity,
    ) -> Result<Option<CatalogCacheEntry>, CacheValidationError> {
        let inst = match TrustedInstanceDir::open(grok_home, identity.instance_id.as_str(), false) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
            Ok(i) => i,
        };
        let _lock = InstanceLock::acquire(&inst)?;
        recover_transaction(&inst)?;
        let state = match load_state_unlocked(&inst)? {
            Some(s) => s,
            None => return Ok(None),
        };
        validate_state_against_identity(&state, identity)?;
        if state.tombstoned {
            return Err(CacheValidationError::Tombstoned);
        }
        let Some(bytes) = read_optional_regular_relative(&inst, CATALOG_FILE, MAX_CATALOG_BYTES)?
        else {
            return Ok(None);
        };
        let entry: CatalogCacheEntry = serde_json::from_slice(&bytes)
            .map_err(|e| CacheValidationError::Corrupt(e.to_string()))?;
        validate_catalog_envelope(&entry, identity)?;
        Ok(Some(entry))
    }

    pub fn store_catalog(
        grok_home: &Path,
        identity: &ProviderCacheIdentity,
        entry: &CatalogCacheEntry,
    ) -> Result<(), CacheValidationError> {
        let mut entry = entry.clone();
        seal_catalog_entry(&mut entry, identity);
        validate_catalog_envelope(&entry, identity)?;
        let catalog_bytes = serde_json::to_vec_pretty(&entry)
            .map_err(|e| CacheValidationError::Corrupt(e.to_string()))?;
        if catalog_bytes.len() as u64 > MAX_CATALOG_BYTES {
            return Err(CacheValidationError::Corrupt(
                "catalog exceeds size bound".into(),
            ));
        }

        with_instance(grok_home, &identity.instance_id, true, |inst, lock| {
            ensure_lock_live(lock, inst)?;
            recover_transaction(inst)?;
            ensure_lock_live(lock, inst)?;
            let previous_state = load_state_unlocked(inst)?;
            refuse_same_identity_untombstone(previous_state.as_ref(), identity)?;
            let supersede = previous_state
                .as_ref()
                .is_some_and(|s| validate_state_against_identity(s, identity).is_err());
            let (catalog_generation, capability_generation, lifecycle_generation) =
                next_generations(previous_state.as_ref(), identity, true, false);
            let legacy_import = if supersede {
                None
            } else {
                previous_state
                    .as_ref()
                    .and_then(|s| s.legacy_import.clone())
            };
            let state = build_state(
                identity,
                catalog_generation,
                if supersede { 0 } else { capability_generation },
                lifecycle_generation,
                legacy_import,
                false,
            );
            let state_bytes = encode_state(&state)?;
            let catalog_hash = content_hash(&catalog_bytes);
            commit_files(
                inst,
                lock,
                Some((catalog_bytes, catalog_hash)),
                None,
                state_bytes,
                false,
                supersede,
            )?;
            Ok(())
        })
    }

    pub fn load_capabilities(
        grok_home: &Path,
        identity: &ProviderCacheIdentity,
        expected_baseline: &str,
    ) -> Result<Option<CapabilityCacheEntry>, CacheValidationError> {
        let inst = match TrustedInstanceDir::open(grok_home, identity.instance_id.as_str(), false) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
            Ok(i) => i,
        };
        let _lock = InstanceLock::acquire(&inst)?;
        recover_transaction(&inst)?;
        let state = match load_state_unlocked(&inst)? {
            Some(s) => s,
            None => return Ok(None),
        };
        validate_state_against_identity(&state, identity)?;
        if state.tombstoned {
            return Err(CacheValidationError::Tombstoned);
        }
        let Some(bytes) =
            read_optional_regular_relative(&inst, CAPABILITIES_FILE, MAX_CAPABILITIES_BYTES)?
        else {
            return Ok(None);
        };
        let entry: CapabilityCacheEntry = serde_json::from_slice(&bytes)
            .map_err(|e| CacheValidationError::Corrupt(e.to_string()))?;
        validate_capability_envelope(&entry, identity, expected_baseline)?;
        Ok(Some(entry))
    }

    pub fn store_capabilities(
        grok_home: &Path,
        identity: &ProviderCacheIdentity,
        entry: &CapabilityCacheEntry,
    ) -> Result<(), CacheValidationError> {
        let mut entry = entry.clone();
        seal_capability_entry(&mut entry, identity);
        validate_capability_envelope(&entry, identity, &entry.baseline_version)?;
        let cap_bytes = serde_json::to_vec_pretty(&entry)
            .map_err(|e| CacheValidationError::Corrupt(e.to_string()))?;
        if cap_bytes.len() as u64 > MAX_CAPABILITIES_BYTES {
            return Err(CacheValidationError::Corrupt(
                "capabilities exceed size bound".into(),
            ));
        }

        with_instance(grok_home, &identity.instance_id, true, |inst, lock| {
            ensure_lock_live(lock, inst)?;
            recover_transaction(inst)?;
            ensure_lock_live(lock, inst)?;
            let previous_state = load_state_unlocked(inst)?;
            refuse_same_identity_untombstone(previous_state.as_ref(), identity)?;
            let supersede = previous_state
                .as_ref()
                .is_some_and(|s| validate_state_against_identity(s, identity).is_err());
            let (catalog_generation, capability_generation, lifecycle_generation) =
                next_generations(previous_state.as_ref(), identity, false, true);
            let legacy_import = if supersede {
                None
            } else {
                previous_state
                    .as_ref()
                    .and_then(|s| s.legacy_import.clone())
            };
            let state = build_state(
                identity,
                if supersede { 0 } else { catalog_generation },
                capability_generation,
                lifecycle_generation,
                legacy_import,
                false,
            );
            let state_bytes = encode_state(&state)?;
            let cap_hash = content_hash(&cap_bytes);
            commit_files(
                inst,
                lock,
                None,
                Some((cap_bytes, cap_hash)),
                state_bytes,
                supersede,
                false,
            )?;
            Ok(())
        })
    }

    pub fn load_state(
        grok_home: &Path,
        instance_id: &ProviderId,
    ) -> Result<Option<ProviderCacheState>, CacheValidationError> {
        let inst = match TrustedInstanceDir::open(grok_home, instance_id.as_str(), false) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
            Ok(i) => i,
        };
        let _lock = InstanceLock::acquire(&inst)?;
        recover_transaction(&inst)?;
        Ok(load_state_unlocked(&inst)?)
    }

    pub fn tombstone(
        grok_home: &Path,
        identity: &ProviderCacheIdentity,
    ) -> Result<(), CacheValidationError> {
        with_instance(grok_home, &identity.instance_id, true, |inst, lock| {
            ensure_lock_live(lock, inst)?;
            recover_transaction(inst)?;
            ensure_lock_live(lock, inst)?;
            let previous = load_state_unlocked(inst)?;
            if let Some(prev) = &previous {
                validate_state_against_identity(prev, identity)?;
            }
            let state = build_state(
                identity,
                previous.as_ref().map(|s| s.catalog_generation).unwrap_or(0),
                previous
                    .as_ref()
                    .map(|s| s.capability_generation)
                    .unwrap_or(0),
                previous
                    .as_ref()
                    .map(|s| s.lifecycle_generation.saturating_add(1))
                    .unwrap_or(1),
                previous.as_ref().and_then(|s| s.legacy_import.clone()),
                true,
            );
            let state_bytes = encode_state(&state)?;
            commit_clear_payloads(inst, lock, true, true, state_bytes)?;
            Ok(())
        })
    }

    pub fn remove_instance(
        grok_home: &Path,
        instance_id: &ProviderId,
    ) -> Result<(), CacheValidationError> {
        match TrustedInstanceDir::open(grok_home, instance_id.as_str(), false) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
            Ok(inst) => {
                let lock = InstanceLock::acquire(&inst)?;
                ensure_lock_live(&lock, &inst)?;
                recover_transaction(&inst)?;
                ensure_lock_live(&lock, &inst)?;
                remove_instance_dir_locked(&inst, instance_id.as_str())?;
                drop(lock);
                Ok(())
            }
        }
    }

    pub fn try_import_legacy_builtin(
        grok_home: &Path,
        identity: &ProviderCacheIdentity,
    ) -> Result<Option<CatalogCacheEntry>, CacheValidationError> {
        let built_in = match BuiltInProviderId::parse(identity.instance_id.as_str()) {
            Some(b) if b.as_str() == identity.instance_id.as_str() => b,
            _ => return Ok(None),
        };
        match Self::load_catalog(grok_home, identity) {
            Ok(Some(entry)) => return Ok(Some(entry)),
            Ok(None) => {}
            Err(CacheValidationError::IncarnationMismatch)
            | Err(CacheValidationError::BindingMismatch)
            | Err(CacheValidationError::Tombstoned) => {
                if Self::load_state(grok_home, &identity.instance_id)?.is_some() {
                    return Ok(None);
                }
            }
            Err(e) => return Err(e),
        }

        let Some((source_name, legacy_models, fetched_at)) =
            read_legacy_builtin_catalog(grok_home, built_in, identity)?
        else {
            return Ok(None);
        };

        let source_bytes =
            read_home_regular_nofollow(grok_home, source_name, MAX_CATALOG_BYTES)?
                .ok_or_else(|| CacheValidationError::Corrupt("legacy disappeared".into()))?;
        let source_sha = content_hash(&source_bytes);

        if let Some(state) = Self::load_state(grok_home, &identity.instance_id)?
            && let Some(marker) = &state.legacy_import
            && marker.source == source_name
            && marker.source_sha256 == source_sha
        {
            if let Ok(Some(entry)) = Self::load_catalog(grok_home, identity) {
                return Ok(Some(entry));
            }
        }

        let entry = CatalogCacheEntry {
            version: CATALOG_CACHE_VERSION,
            provider_id: identity.instance_id.as_str().to_owned(),
            origin: CacheOrigin::LegacyMigration,
            base_url_origin: identity.endpoint_origin.clone(),
            fetched_at_unix: fetched_at.unwrap_or(0),
            models: legacy_models,
            baseline_version: None,
            incarnation: Some(identity.incarnation.clone()),
            provider_kind: Some(identity.kind),
            api_surface: Some(identity.api_surface),
            credential_route: Some(identity.credential_route),
            credential_binding_id: Some(identity.credential_binding_id.clone()),
            org_project_fingerprint: Some(identity.org_project_fingerprint.clone()),
            catalog_generation: 1,
            lifecycle_generation: Some(1),
        };

        with_instance(grok_home, &identity.instance_id, true, |inst, lock| {
            ensure_lock_live(lock, inst)?;
            recover_transaction(inst)?;
            ensure_lock_live(lock, inst)?;

            if load_state_unlocked(inst)?.is_none() {
                if let Some(bytes) =
                    read_optional_regular_relative(inst, CATALOG_FILE, MAX_CATALOG_BYTES)?
                {
                    if let Ok(existing) = serde_json::from_slice::<CatalogCacheEntry>(&bytes)
                        && existing.version == CATALOG_CACHE_VERSION
                        && existing.provider_id == identity.instance_id.as_str()
                        && existing.base_url_origin == identity.endpoint_origin
                        && !existing.models.is_empty()
                    {
                        return Ok(Some(existing));
                    }
                }
            }

            if let Some(state) = load_state_unlocked(inst)? {
                if validate_state_against_identity(&state, identity).is_ok() && !state.tombstoned {
                    if let Some(bytes) =
                        read_optional_regular_relative(inst, CATALOG_FILE, MAX_CATALOG_BYTES)?
                    {
                        let existing: CatalogCacheEntry = serde_json::from_slice(&bytes)
                            .map_err(|e| CacheValidationError::Corrupt(e.to_string()))?;
                        if validate_catalog_envelope(&existing, identity).is_ok() {
                            return Ok(Some(existing));
                        }
                    }
                } else {
                    return Ok(None);
                }
            }

            let mut sealed = entry.clone();
            seal_catalog_entry(&mut sealed, identity);
            let catalog_bytes = serde_json::to_vec_pretty(&sealed)
                .map_err(|e| CacheValidationError::Corrupt(e.to_string()))?;
            let state = build_state(
                identity,
                1,
                0,
                1,
                Some(LegacyImportMarker {
                    source: source_name.to_owned(),
                    imported_at_unix: now_unix(),
                    source_sha256: source_sha,
                }),
                false,
            );
            let state_bytes = encode_state(&state)?;
            let catalog_hash = content_hash(&catalog_bytes);
            commit_files(
                inst,
                lock,
                Some((catalog_bytes, catalog_hash)),
                None,
                state_bytes,
                false,
                false,
            )?;
            Ok(Some(sealed))
        })
    }
}

// ---------------------------------------------------------------------------
// Compatibility facades
// ---------------------------------------------------------------------------

pub struct CatalogCacheStore;

impl CatalogCacheStore {
    pub fn load(
        grok_home: &Path,
        provider_id: &ProviderId,
        expected_origin_host: &str,
    ) -> Result<Option<CatalogCacheEntry>, CacheValidationError> {
        match TrustedInstanceDir::open(grok_home, provider_id.as_str(), false) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
            Ok(inst) => {
                let _lock = InstanceLock::acquire(&inst)?;
                recover_transaction(&inst)?;
                let state = load_state_unlocked(&inst)?;
                if let Some(s) = &state {
                    if s.tombstoned {
                        return Err(CacheValidationError::Tombstoned);
                    }
                    if s.provider_instance_id != provider_id.as_str() {
                        return Err(CacheValidationError::ProviderMismatch {
                            found: s.provider_instance_id.clone(),
                            expected: provider_id.as_str().to_owned(),
                        });
                    }
                    if s.endpoint_origin != expected_origin_host {
                        return Err(CacheValidationError::OriginMismatch);
                    }
                }
                let Some(bytes) =
                    read_optional_regular_relative(&inst, CATALOG_FILE, MAX_CATALOG_BYTES)?
                else {
                    return Ok(None);
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
                if let Some(s) = &state {
                    enforce_envelope_matches_state_catalog(&entry, s)?;
                }
                Ok(Some(entry))
            }
        }
    }

    pub fn store(grok_home: &Path, entry: &CatalogCacheEntry) -> io::Result<()> {
        let provider_id = ProviderId::new(&entry.provider_id)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
        if let (Some(incarnation), Some(kind), Some(surface), Some(route), Some(binding)) = (
            entry.incarnation.clone(),
            entry.provider_kind,
            entry.api_surface,
            entry.credential_route,
            entry.credential_binding_id.clone(),
        ) {
            let identity = ProviderCacheIdentity::new(
                provider_id,
                incarnation,
                kind,
                surface,
                route,
                entry.base_url_origin.clone(),
                entry.org_project_fingerprint.clone().unwrap_or_default(),
                binding,
            )
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
            return ProviderCacheStore::store_catalog(grok_home, &identity, entry)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()));
        }

        let inst = TrustedInstanceDir::open(grok_home, provider_id.as_str(), true)?;
        let lock = InstanceLock::acquire(&inst)?;
        ensure_lock_live(&lock, &inst).map_err(|e| io::Error::new(e.kind(), e.to_string()))?;
        recover_transaction(&inst)?;
        ensure_lock_live(&lock, &inst).map_err(|e| io::Error::new(e.kind(), e.to_string()))?;
        if load_state_unlocked(&inst)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?
            .is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "partial catalog store refused over authoritative provider cache state",
            ));
        }
        let catalog_bytes = serde_json::to_vec_pretty(entry)?;
        if catalog_bytes.len() as u64 > MAX_CATALOG_BYTES {
            return Err(invalid_data("catalog exceeds size bound"));
        }
        let previous_catalog = file_hash_relative(&inst, CATALOG_FILE, MAX_CATALOG_BYTES)?;
        let previous_capabilities =
            file_hash_relative(&inst, CAPABILITIES_FILE, MAX_CAPABILITIES_BYTES)?;
        let previous_state = file_hash_relative(&inst, STATE_FILE, MAX_STATE_BYTES)?;
        let tmp = stage_bytes_relative(&inst, CATALOG_FILE, &catalog_bytes)?;
        let marker = TxnMarker {
            version: TXN_VERSION,
            catalog_tmp: Some(tmp.clone()),
            capabilities_tmp: None,
            state_tmp: None,
            catalog_sha256: Some(content_hash(&catalog_bytes)),
            capabilities_sha256: None,
            state_sha256: None,
            previous_catalog_sha256: previous_catalog,
            previous_capabilities_sha256: previous_capabilities,
            previous_state_sha256: previous_state,
            clear_catalog: false,
            clear_capabilities: false,
            clear_all: false,
            partial_compat: true,
        };
        ensure_lock_live(&lock, &inst).map_err(|e| io::Error::new(e.kind(), e.to_string()))?;
        write_marker(&inst, &marker)?;
        maybe_fault(ProviderCacheTxnFault::AfterJournalFsync)
            .map_err(|e| io::Error::new(e.kind(), e.to_string()))?;
        ensure_lock_live(&lock, &inst).map_err(|e| io::Error::new(e.kind(), e.to_string()))?;
        rename_relative(&inst, &tmp, CATALOG_FILE)?;
        maybe_fault(ProviderCacheTxnFault::AfterCatalogRename)
            .map_err(|e| io::Error::new(e.kind(), e.to_string()))?;
        unlink_relative(&inst, TXN_FILE)?;
        Ok(())
    }

    pub fn remove(grok_home: &Path, provider_id: &ProviderId) -> io::Result<()> {
        match TrustedInstanceDir::open(grok_home, provider_id.as_str(), false) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
            Ok(inst) => {
                let _lock = InstanceLock::acquire(&inst)?;
                recover_transaction(&inst)?;
                unlink_relative(&inst, CATALOG_FILE)
            }
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
        match TrustedInstanceDir::open(grok_home, provider_id.as_str(), false) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
            Ok(inst) => {
                let _lock = InstanceLock::acquire(&inst)?;
                recover_transaction(&inst)?;
                let state = load_state_unlocked(&inst)?;
                if let Some(s) = &state {
                    if s.tombstoned {
                        return Err(CacheValidationError::Tombstoned);
                    }
                    if s.provider_instance_id != provider_id.as_str() {
                        return Err(CacheValidationError::ProviderMismatch {
                            found: s.provider_instance_id.clone(),
                            expected: provider_id.as_str().to_owned(),
                        });
                    }
                    if s.endpoint_origin != expected_origin_host {
                        return Err(CacheValidationError::OriginMismatch);
                    }
                }
                let Some(bytes) = read_optional_regular_relative(
                    &inst,
                    CAPABILITIES_FILE,
                    MAX_CAPABILITIES_BYTES,
                )?
                else {
                    return Ok(None);
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
                if let Some(s) = &state {
                    enforce_envelope_matches_state_capability(&entry, s)?;
                }
                Ok(Some(entry))
            }
        }
    }

    pub fn store(grok_home: &Path, entry: &CapabilityCacheEntry) -> io::Result<()> {
        let provider_id = ProviderId::new(&entry.provider_id)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
        if let (Some(incarnation), Some(kind), Some(surface), Some(route), Some(binding)) = (
            entry.incarnation.clone(),
            entry.provider_kind,
            entry.api_surface,
            entry.credential_route,
            entry.credential_binding_id.clone(),
        ) {
            let identity = ProviderCacheIdentity::new(
                provider_id,
                incarnation,
                kind,
                surface,
                route,
                entry.base_url_origin.clone(),
                entry.org_project_fingerprint.clone().unwrap_or_default(),
                binding,
            )
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
            return ProviderCacheStore::store_capabilities(grok_home, &identity, entry)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()));
        }
        let inst = TrustedInstanceDir::open(grok_home, provider_id.as_str(), true)?;
        let lock = InstanceLock::acquire(&inst)?;
        ensure_lock_live(&lock, &inst).map_err(|e| io::Error::new(e.kind(), e.to_string()))?;
        recover_transaction(&inst)?;
        ensure_lock_live(&lock, &inst).map_err(|e| io::Error::new(e.kind(), e.to_string()))?;
        if load_state_unlocked(&inst)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?
            .is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "partial capability store refused over authoritative provider cache state",
            ));
        }
        let bytes = serde_json::to_vec_pretty(entry)?;
        if bytes.len() as u64 > MAX_CAPABILITIES_BYTES {
            return Err(invalid_data("capabilities exceed size bound"));
        }
        let previous_catalog = file_hash_relative(&inst, CATALOG_FILE, MAX_CATALOG_BYTES)?;
        let previous_capabilities =
            file_hash_relative(&inst, CAPABILITIES_FILE, MAX_CAPABILITIES_BYTES)?;
        let previous_state = file_hash_relative(&inst, STATE_FILE, MAX_STATE_BYTES)?;
        let tmp = stage_bytes_relative(&inst, CAPABILITIES_FILE, &bytes)?;
        let marker = TxnMarker {
            version: TXN_VERSION,
            catalog_tmp: None,
            capabilities_tmp: Some(tmp.clone()),
            state_tmp: None,
            catalog_sha256: None,
            capabilities_sha256: Some(content_hash(&bytes)),
            state_sha256: None,
            previous_catalog_sha256: previous_catalog,
            previous_capabilities_sha256: previous_capabilities,
            previous_state_sha256: previous_state,
            clear_catalog: false,
            clear_capabilities: false,
            clear_all: false,
            partial_compat: true,
        };
        ensure_lock_live(&lock, &inst).map_err(|e| io::Error::new(e.kind(), e.to_string()))?;
        write_marker(&inst, &marker)?;
        maybe_fault(ProviderCacheTxnFault::AfterJournalFsync)
            .map_err(|e| io::Error::new(e.kind(), e.to_string()))?;
        ensure_lock_live(&lock, &inst).map_err(|e| io::Error::new(e.kind(), e.to_string()))?;
        rename_relative(&inst, &tmp, CAPABILITIES_FILE)?;
        maybe_fault(ProviderCacheTxnFault::AfterCapabilitiesRename)
            .map_err(|e| io::Error::new(e.kind(), e.to_string()))?;
        unlink_relative(&inst, TXN_FILE)?;
        Ok(())
    }

    pub fn remove(grok_home: &Path, provider_id: &ProviderId) -> io::Result<()> {
        match TrustedInstanceDir::open(grok_home, provider_id.as_str(), false) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
            Ok(inst) => {
                let _lock = InstanceLock::acquire(&inst)?;
                recover_transaction(&inst)?;
                unlink_relative(&inst, CAPABILITIES_FILE)
            }
        }
    }
}

pub fn remove_all_provider_caches(grok_home: &Path, provider_id: &ProviderId) -> io::Result<()> {
    ProviderCacheStore::remove_instance(grok_home, provider_id)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn with_instance<T>(
    grok_home: &Path,
    instance_id: &ProviderId,
    create: bool,
    f: impl FnOnce(&TrustedInstanceDir, &InstanceLock) -> Result<T, CacheValidationError>,
) -> Result<T, CacheValidationError> {
    let inst = TrustedInstanceDir::open(grok_home, instance_id.as_str(), create)?;
    let lock = InstanceLock::acquire(&inst)?;
    f(&inst, &lock)
}

fn seal_catalog_entry(entry: &mut CatalogCacheEntry, identity: &ProviderCacheIdentity) {
    entry.version = CATALOG_CACHE_VERSION;
    entry.provider_id = identity.instance_id.as_str().to_owned();
    entry.base_url_origin = identity.endpoint_origin.clone();
    entry.incarnation = Some(identity.incarnation.clone());
    entry.provider_kind = Some(identity.kind);
    entry.api_surface = Some(identity.api_surface);
    entry.credential_route = Some(identity.credential_route);
    entry.credential_binding_id = Some(identity.credential_binding_id.clone());
    entry.org_project_fingerprint = Some(identity.org_project_fingerprint.clone());
}

fn seal_capability_entry(entry: &mut CapabilityCacheEntry, identity: &ProviderCacheIdentity) {
    entry.version = CAPABILITY_CACHE_VERSION;
    entry.provider_id = identity.instance_id.as_str().to_owned();
    entry.base_url_origin = identity.endpoint_origin.clone();
    entry.incarnation = Some(identity.incarnation.clone());
    entry.provider_kind = Some(identity.kind);
    entry.api_surface = Some(identity.api_surface);
    entry.credential_route = Some(identity.credential_route);
    entry.credential_binding_id = Some(identity.credential_binding_id.clone());
    entry.org_project_fingerprint = Some(identity.org_project_fingerprint.clone());
}

fn validate_catalog_envelope(
    entry: &CatalogCacheEntry,
    identity: &ProviderCacheIdentity,
) -> Result<(), CacheValidationError> {
    if entry.version != CATALOG_CACHE_VERSION {
        return Err(CacheValidationError::VersionMismatch {
            found: entry.version,
            expected: CATALOG_CACHE_VERSION,
        });
    }
    if entry.provider_id != identity.instance_id.as_str() {
        return Err(CacheValidationError::ProviderMismatch {
            found: entry.provider_id.clone(),
            expected: identity.instance_id.as_str().to_owned(),
        });
    }
    if entry.base_url_origin != identity.endpoint_origin {
        return Err(CacheValidationError::OriginMismatch);
    }
    match &entry.incarnation {
        Some(inc) if inc == &identity.incarnation => {}
        _ => return Err(CacheValidationError::IncarnationMismatch),
    }
    if entry.provider_kind != Some(identity.kind) {
        return Err(CacheValidationError::KindMismatch);
    }
    if entry.api_surface != Some(identity.api_surface) {
        return Err(CacheValidationError::SurfaceMismatch);
    }
    if entry.credential_route != Some(identity.credential_route) {
        return Err(CacheValidationError::RouteMismatch);
    }
    match &entry.credential_binding_id {
        Some(b) if b == &identity.credential_binding_id => {}
        _ => return Err(CacheValidationError::BindingMismatch),
    }
    let fp = entry.org_project_fingerprint.as_deref().unwrap_or("");
    validate_org_project_fingerprint(fp)
        .map_err(|e| CacheValidationError::Corrupt(e.to_string()))?;
    if fp != identity.org_project_fingerprint {
        return Err(CacheValidationError::OrgProjectMismatch);
    }
    Ok(())
}

fn validate_capability_envelope(
    entry: &CapabilityCacheEntry,
    identity: &ProviderCacheIdentity,
    expected_baseline: &str,
) -> Result<(), CacheValidationError> {
    if entry.version != CAPABILITY_CACHE_VERSION {
        return Err(CacheValidationError::VersionMismatch {
            found: entry.version,
            expected: CAPABILITY_CACHE_VERSION,
        });
    }
    if entry.provider_id != identity.instance_id.as_str() {
        return Err(CacheValidationError::ProviderMismatch {
            found: entry.provider_id.clone(),
            expected: identity.instance_id.as_str().to_owned(),
        });
    }
    if entry.base_url_origin != identity.endpoint_origin {
        return Err(CacheValidationError::OriginMismatch);
    }
    if entry.baseline_version != expected_baseline {
        return Err(CacheValidationError::BaselineMismatch {
            found: entry.baseline_version.clone(),
            expected: expected_baseline.to_owned(),
        });
    }
    match &entry.incarnation {
        Some(inc) if inc == &identity.incarnation => {}
        _ => return Err(CacheValidationError::IncarnationMismatch),
    }
    if entry.provider_kind != Some(identity.kind) {
        return Err(CacheValidationError::KindMismatch);
    }
    if entry.api_surface != Some(identity.api_surface) {
        return Err(CacheValidationError::SurfaceMismatch);
    }
    if entry.credential_route != Some(identity.credential_route) {
        return Err(CacheValidationError::RouteMismatch);
    }
    match &entry.credential_binding_id {
        Some(b) if b == &identity.credential_binding_id => {}
        _ => return Err(CacheValidationError::BindingMismatch),
    }
    let fp = entry.org_project_fingerprint.as_deref().unwrap_or("");
    validate_org_project_fingerprint(fp)
        .map_err(|e| CacheValidationError::Corrupt(e.to_string()))?;
    if fp != identity.org_project_fingerprint {
        return Err(CacheValidationError::OrgProjectMismatch);
    }
    Ok(())
}

fn validate_state_against_identity(
    state: &ProviderCacheState,
    identity: &ProviderCacheIdentity,
) -> Result<(), CacheValidationError> {
    if state.schema_version != STATE_CACHE_VERSION {
        return Err(CacheValidationError::VersionMismatch {
            found: state.schema_version,
            expected: STATE_CACHE_VERSION,
        });
    }
    validate_org_project_fingerprint(&state.org_project_fingerprint)
        .map_err(|e| CacheValidationError::Corrupt(e.to_string()))?;
    validate_org_project_fingerprint(&identity.org_project_fingerprint)
        .map_err(|e| CacheValidationError::Corrupt(e.to_string()))?;
    if state.provider_instance_id != identity.instance_id.as_str() {
        return Err(CacheValidationError::ProviderMismatch {
            found: state.provider_instance_id.clone(),
            expected: identity.instance_id.as_str().to_owned(),
        });
    }
    if state.incarnation != identity.incarnation {
        return Err(CacheValidationError::IncarnationMismatch);
    }
    if state.provider_kind != identity.kind {
        return Err(CacheValidationError::KindMismatch);
    }
    if state.api_surface != identity.api_surface {
        return Err(CacheValidationError::SurfaceMismatch);
    }
    if state.credential_route != identity.credential_route {
        return Err(CacheValidationError::RouteMismatch);
    }
    if state.endpoint_origin != identity.endpoint_origin {
        return Err(CacheValidationError::OriginMismatch);
    }
    if state.org_project_fingerprint != identity.org_project_fingerprint {
        return Err(CacheValidationError::OrgProjectMismatch);
    }
    if state.credential_binding_id != identity.credential_binding_id {
        return Err(CacheValidationError::BindingMismatch);
    }
    Ok(())
}

fn build_state(
    identity: &ProviderCacheIdentity,
    catalog_generation: u64,
    capability_generation: u64,
    lifecycle_generation: u64,
    legacy_import: Option<LegacyImportMarker>,
    tombstoned: bool,
) -> ProviderCacheState {
    ProviderCacheState {
        schema_version: STATE_CACHE_VERSION,
        provider_instance_id: identity.instance_id.as_str().to_owned(),
        incarnation: identity.incarnation.clone(),
        provider_kind: identity.kind,
        api_surface: identity.api_surface,
        credential_route: identity.credential_route,
        endpoint_origin: identity.endpoint_origin.clone(),
        org_project_fingerprint: identity.org_project_fingerprint.clone(),
        credential_binding_id: identity.credential_binding_id.clone(),
        catalog_generation,
        capability_generation,
        lifecycle_generation,
        tombstoned,
        legacy_import,
        updated_at_unix: now_unix(),
    }
}

fn next_generations(
    previous: Option<&ProviderCacheState>,
    identity: &ProviderCacheIdentity,
    bump_catalog: bool,
    bump_capability: bool,
) -> (u64, u64, u64) {
    match previous {
        Some(prev)
            if !prev.tombstoned
                && prev.incarnation == identity.incarnation
                && prev.credential_binding_id == identity.credential_binding_id =>
        {
            let catalog = if bump_catalog {
                prev.catalog_generation.saturating_add(1).max(1)
            } else {
                prev.catalog_generation
            };
            let capability = if bump_capability {
                prev.capability_generation.saturating_add(1).max(1)
            } else {
                prev.capability_generation
            };
            let lifecycle = prev.lifecycle_generation.max(1);
            (catalog, capability, lifecycle)
        }
        _ => (
            if bump_catalog { 1 } else { 0 },
            if bump_capability { 1 } else { 0 },
            1,
        ),
    }
}

fn refuse_same_identity_untombstone(
    previous: Option<&ProviderCacheState>,
    identity: &ProviderCacheIdentity,
) -> Result<(), CacheValidationError> {
    if let Some(prev) = previous
        && prev.tombstoned
        && prev.incarnation == identity.incarnation
        && prev.credential_binding_id == identity.credential_binding_id
    {
        return Err(CacheValidationError::Tombstoned);
    }
    Ok(())
}

fn ensure_lock_live(lock: &InstanceLock, inst: &TrustedInstanceDir) -> io::Result<()> {
    if !lock.still_live(inst) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "provider cache lock is no longer live",
        ));
    }
    Ok(())
}

fn enforce_envelope_matches_state_catalog(
    entry: &CatalogCacheEntry,
    state: &ProviderCacheState,
) -> Result<(), CacheValidationError> {
    match &entry.incarnation {
        Some(inc) if inc == &state.incarnation => {}
        _ => return Err(CacheValidationError::IncarnationMismatch),
    }
    match &entry.credential_binding_id {
        Some(b) if b == &state.credential_binding_id => {}
        _ => return Err(CacheValidationError::BindingMismatch),
    }
    if let Some(kind) = entry.provider_kind
        && kind != state.provider_kind
    {
        return Err(CacheValidationError::KindMismatch);
    }
    if let Some(surface) = entry.api_surface
        && surface != state.api_surface
    {
        return Err(CacheValidationError::SurfaceMismatch);
    }
    if let Some(route) = entry.credential_route
        && route != state.credential_route
    {
        return Err(CacheValidationError::RouteMismatch);
    }
    if let Some(fp) = &entry.org_project_fingerprint {
        validate_org_project_fingerprint(fp)
            .map_err(|e| CacheValidationError::Corrupt(e.to_string()))?;
        if fp != &state.org_project_fingerprint {
            return Err(CacheValidationError::OrgProjectMismatch);
        }
    }
    Ok(())
}

fn enforce_envelope_matches_state_capability(
    entry: &CapabilityCacheEntry,
    state: &ProviderCacheState,
) -> Result<(), CacheValidationError> {
    match &entry.incarnation {
        Some(inc) if inc == &state.incarnation => {}
        _ => return Err(CacheValidationError::IncarnationMismatch),
    }
    match &entry.credential_binding_id {
        Some(b) if b == &state.credential_binding_id => {}
        _ => return Err(CacheValidationError::BindingMismatch),
    }
    if let Some(kind) = entry.provider_kind
        && kind != state.provider_kind
    {
        return Err(CacheValidationError::KindMismatch);
    }
    if let Some(surface) = entry.api_surface
        && surface != state.api_surface
    {
        return Err(CacheValidationError::SurfaceMismatch);
    }
    if let Some(route) = entry.credential_route
        && route != state.credential_route
    {
        return Err(CacheValidationError::RouteMismatch);
    }
    if let Some(fp) = &entry.org_project_fingerprint {
        validate_org_project_fingerprint(fp)
            .map_err(|e| CacheValidationError::Corrupt(e.to_string()))?;
        if fp != &state.org_project_fingerprint {
            return Err(CacheValidationError::OrgProjectMismatch);
        }
    }
    Ok(())
}

fn encode_state(state: &ProviderCacheState) -> Result<Vec<u8>, CacheValidationError> {
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|e| CacheValidationError::Corrupt(e.to_string()))?;
    if bytes.len() as u64 > MAX_STATE_BYTES {
        return Err(CacheValidationError::Corrupt(
            "state exceeds size bound".into(),
        ));
    }
    Ok(bytes)
}

fn load_state_unlocked(
    inst: &TrustedInstanceDir,
) -> Result<Option<ProviderCacheState>, CacheValidationError> {
    let Some(bytes) = read_optional_regular_relative(inst, STATE_FILE, MAX_STATE_BYTES)? else {
        return Ok(None);
    };
    let state: ProviderCacheState =
        serde_json::from_slice(&bytes).map_err(|e| CacheValidationError::Corrupt(e.to_string()))?;
    if state.schema_version != STATE_CACHE_VERSION {
        return Err(CacheValidationError::VersionMismatch {
            found: state.schema_version,
            expected: STATE_CACHE_VERSION,
        });
    }
    validate_org_project_fingerprint(&state.org_project_fingerprint)
        .map_err(|e| CacheValidationError::Corrupt(e.to_string()))?;
    Ok(Some(state))
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn write_marker(inst: &TrustedInstanceDir, marker: &TxnMarker) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(marker)
        .map_err(|e| invalid_data(format!("txn marker serialize: {e}")))?;
    if bytes.len() as u64 > MAX_TXN_BYTES {
        return Err(invalid_data("txn marker exceeds bound"));
    }
    let tmp = stage_bytes_relative(inst, TXN_FILE, &bytes)?;
    rename_relative(inst, &tmp, TXN_FILE)
}

fn read_marker(inst: &TrustedInstanceDir) -> io::Result<Option<TxnMarker>> {
    let Some(bytes) = read_optional_regular_relative(inst, TXN_FILE, MAX_TXN_BYTES)? else {
        return Ok(None);
    };
    let marker: TxnMarker = serde_json::from_slice(&bytes)
        .map_err(|e| invalid_data(format!("invalid provider cache txn marker: {e}")))?;
    if marker.version != TXN_VERSION {
        return Err(invalid_data(
            "unsupported provider cache txn marker version",
        ));
    }
    for name in [
        marker.catalog_tmp.as_deref(),
        marker.capabilities_tmp.as_deref(),
        marker.state_tmp.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        fs::validate_single_component_name(name)?;
    }
    Ok(Some(marker))
}

fn hashes_match(actual: Option<String>, expected: Option<&String>) -> bool {
    match (actual.as_ref(), expected) {
        (None, None) => true,
        (Some(a), Some(e)) => a == e,
        _ => false,
    }
}

fn intent_matches(
    marker: &TxnMarker,
    catalog: &Option<String>,
    caps: &Option<String>,
    state: &Option<String>,
    previous: bool,
) -> bool {
    if previous {
        hashes_match(catalog.clone(), marker.previous_catalog_sha256.as_ref())
            && hashes_match(caps.clone(), marker.previous_capabilities_sha256.as_ref())
            && hashes_match(state.clone(), marker.previous_state_sha256.as_ref())
    } else {
        let cat_ok = if marker.clear_catalog || marker.clear_all {
            catalog.is_none()
        } else if let Some(expected) = &marker.catalog_sha256 {
            catalog.as_ref() == Some(expected)
        } else {
            hashes_match(catalog.clone(), marker.previous_catalog_sha256.as_ref())
        };
        let caps_ok = if marker.clear_capabilities || marker.clear_all {
            caps.is_none()
        } else if let Some(expected) = &marker.capabilities_sha256 {
            caps.as_ref() == Some(expected)
        } else {
            hashes_match(caps.clone(), marker.previous_capabilities_sha256.as_ref())
        };
        let state_ok = if marker.clear_all {
            state.is_none()
        } else if let Some(expected) = &marker.state_sha256 {
            state.as_ref() == Some(expected)
        } else {
            hashes_match(state.clone(), marker.previous_state_sha256.as_ref())
        };
        cat_ok && caps_ok && state_ok
    }
}

fn validate_marker_temp_names(marker: &TxnMarker) -> io::Result<()> {
    if let Some(name) = &marker.catalog_tmp {
        if !is_valid_staged_temp_name(name, CATALOG_FILE) {
            return Err(invalid_data("invalid catalog temp name in journal"));
        }
    }
    if let Some(name) = &marker.capabilities_tmp {
        if !is_valid_staged_temp_name(name, CAPABILITIES_FILE) {
            return Err(invalid_data("invalid capabilities temp name in journal"));
        }
    }
    if let Some(name) = &marker.state_tmp {
        if !is_valid_staged_temp_name(name, STATE_FILE) {
            return Err(invalid_data("invalid state temp name in journal"));
        }
    }
    Ok(())
}

fn temp_hash_matches(
    inst: &TrustedInstanceDir,
    tmp: &str,
    final_name: &str,
    expected: Option<&String>,
    max: u64,
) -> io::Result<bool> {
    if !is_valid_staged_temp_name(tmp, final_name) {
        return Ok(false);
    }
    let Some(expected) = expected else {
        return Ok(false);
    };
    match file_hash_relative(inst, tmp, max)? {
        Some(h) => Ok(&h == expected),
        None => Ok(false),
    }
}

fn cleanup_txn_temps(inst: &TrustedInstanceDir, marker: &TxnMarker) -> io::Result<()> {
    for (name, final_name) in [
        (marker.catalog_tmp.as_deref(), CATALOG_FILE),
        (marker.capabilities_tmp.as_deref(), CAPABILITIES_FILE),
        (marker.state_tmp.as_deref(), STATE_FILE),
    ] {
        if let Some(n) = name {
            if is_valid_staged_temp_name(n, final_name) {
                let _ = unlink_relative(inst, n);
            }
        }
    }
    Ok(())
}

fn abort_txn_preserve_lkg(inst: &TrustedInstanceDir, marker: &TxnMarker) -> io::Result<()> {
    cleanup_txn_temps(inst, marker)?;
    unlink_relative(inst, TXN_FILE)?;
    Ok(())
}

fn apply_one_payload(
    inst: &TrustedInstanceDir,
    dest: &str,
    dest_hash: &Option<String>,
    previous: Option<&String>,
    intended: Option<&String>,
    tmp: Option<&String>,
    clear: bool,
    max: u64,
) -> io::Result<()> {
    if clear {
        if hashes_match(dest_hash.clone(), previous) || dest_hash.is_none() {
            unlink_relative(inst, dest)?;
            if let Some(t) = tmp {
                if is_valid_staged_temp_name(t, dest) {
                    let _ = unlink_relative(inst, t);
                }
            }
            return Ok(());
        }
        if hashes_match(dest_hash.clone(), intended) || (intended.is_none() && dest_hash.is_none())
        {
            return Ok(());
        }
        return Err(invalid_data(
            "cannot clear payload: destination is neither previous nor intended",
        ));
    }
    if intended.is_none() {
        return Ok(());
    }
    if hashes_match(dest_hash.clone(), intended) {
        if let Some(t) = tmp {
            if is_valid_staged_temp_name(t, dest) {
                let _ = unlink_relative(inst, t);
            }
        }
        return Ok(());
    }
    if !hashes_match(dest_hash.clone(), previous) && dest_hash.is_some() {
        return Err(invalid_data(
            "cannot install payload: destination is neither previous nor intended",
        ));
    }
    let Some(t) = tmp else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "staging temp missing for incomplete provider cache transaction",
        ));
    };
    if !temp_hash_matches(inst, t, dest, intended, max)? {
        return Err(invalid_data(
            "staging temp hash does not match journal intent",
        ));
    }
    rename_relative(inst, t, dest)
}

fn recover_transaction(inst: &TrustedInstanceDir) -> io::Result<()> {
    let Some(marker) = read_marker(inst)? else {
        return Ok(());
    };

    if validate_marker_temp_names(&marker).is_err() {
        let catalog_hash = file_hash_relative(inst, CATALOG_FILE, MAX_CATALOG_BYTES)?;
        let caps_hash = file_hash_relative(inst, CAPABILITIES_FILE, MAX_CAPABILITIES_BYTES)?;
        let state_hash = file_hash_relative(inst, STATE_FILE, MAX_STATE_BYTES)?;
        if intent_matches(&marker, &catalog_hash, &caps_hash, &state_hash, true)
            || intent_matches(&marker, &catalog_hash, &caps_hash, &state_hash, false)
        {
            return abort_txn_preserve_lkg(inst, &marker);
        }
        return Err(invalid_data(
            "malformed provider cache journal; refusing torn cache state",
        ));
    }

    if marker.partial_compat {
        let state_hash = file_hash_relative(inst, STATE_FILE, MAX_STATE_BYTES)?;
        if state_hash.is_some() {
            let catalog_hash = file_hash_relative(inst, CATALOG_FILE, MAX_CATALOG_BYTES)?;
            let caps_hash = file_hash_relative(inst, CAPABILITIES_FILE, MAX_CAPABILITIES_BYTES)?;
            if intent_matches(&marker, &catalog_hash, &caps_hash, &state_hash, true) {
                return abort_txn_preserve_lkg(inst, &marker);
            }
            if intent_matches(&marker, &catalog_hash, &caps_hash, &state_hash, false) {
                cleanup_txn_temps(inst, &marker)?;
                unlink_relative(inst, TXN_FILE)?;
                return Ok(());
            }
            return Err(invalid_data(
                "partial-compat journal over authoritative state; refusing recovery install",
            ));
        }
    }

    let catalog_hash = file_hash_relative(inst, CATALOG_FILE, MAX_CATALOG_BYTES)?;
    let caps_hash = file_hash_relative(inst, CAPABILITIES_FILE, MAX_CAPABILITIES_BYTES)?;
    let state_hash = file_hash_relative(inst, STATE_FILE, MAX_STATE_BYTES)?;

    if intent_matches(&marker, &catalog_hash, &caps_hash, &state_hash, false) {
        cleanup_txn_temps(inst, &marker)?;
        unlink_relative(inst, TXN_FILE)?;
        return Ok(());
    }

    let result = (|| {
        apply_one_payload(
            inst,
            CATALOG_FILE,
            &catalog_hash,
            marker.previous_catalog_sha256.as_ref(),
            marker.catalog_sha256.as_ref(),
            marker.catalog_tmp.as_ref(),
            marker.clear_catalog || marker.clear_all,
            MAX_CATALOG_BYTES,
        )?;
        apply_one_payload(
            inst,
            CAPABILITIES_FILE,
            &caps_hash,
            marker.previous_capabilities_sha256.as_ref(),
            marker.capabilities_sha256.as_ref(),
            marker.capabilities_tmp.as_ref(),
            marker.clear_capabilities || marker.clear_all,
            MAX_CAPABILITIES_BYTES,
        )?;
        apply_one_payload(
            inst,
            STATE_FILE,
            &state_hash,
            marker.previous_state_sha256.as_ref(),
            marker.state_sha256.as_ref(),
            marker.state_tmp.as_ref(),
            marker.clear_all,
            MAX_STATE_BYTES,
        )?;
        Ok::<(), io::Error>(())
    })();

    match result {
        Ok(()) => {
            let catalog_hash = file_hash_relative(inst, CATALOG_FILE, MAX_CATALOG_BYTES)?;
            let caps_hash = file_hash_relative(inst, CAPABILITIES_FILE, MAX_CAPABILITIES_BYTES)?;
            let state_hash = file_hash_relative(inst, STATE_FILE, MAX_STATE_BYTES)?;
            if intent_matches(&marker, &catalog_hash, &caps_hash, &state_hash, false) {
                cleanup_txn_temps(inst, &marker)?;
                unlink_relative(inst, TXN_FILE)?;
                return Ok(());
            }
            Err(invalid_data(
                "incomplete provider cache transaction; refusing torn cache state",
            ))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let catalog_hash = file_hash_relative(inst, CATALOG_FILE, MAX_CATALOG_BYTES)?;
            let caps_hash = file_hash_relative(inst, CAPABILITIES_FILE, MAX_CAPABILITIES_BYTES)?;
            let state_hash = file_hash_relative(inst, STATE_FILE, MAX_STATE_BYTES)?;
            if intent_matches(&marker, &catalog_hash, &caps_hash, &state_hash, true) {
                abort_txn_preserve_lkg(inst, &marker)
            } else {
                Err(invalid_data(format!(
                    "incomplete provider cache transaction: {e}"
                )))
            }
        }
        Err(e) if e.kind() == io::ErrorKind::InvalidData => {
            let catalog_hash = file_hash_relative(inst, CATALOG_FILE, MAX_CATALOG_BYTES)?;
            let caps_hash = file_hash_relative(inst, CAPABILITIES_FILE, MAX_CAPABILITIES_BYTES)?;
            let state_hash = file_hash_relative(inst, STATE_FILE, MAX_STATE_BYTES)?;
            if intent_matches(&marker, &catalog_hash, &caps_hash, &state_hash, true) {
                abort_txn_preserve_lkg(inst, &marker)
            } else {
                Err(e)
            }
        }
        Err(e) => Err(e),
    }
}

fn commit_files(
    inst: &TrustedInstanceDir,
    lock: &InstanceLock,
    catalog: Option<(Vec<u8>, String)>,
    capabilities: Option<(Vec<u8>, String)>,
    state_bytes: Vec<u8>,
    clear_catalog: bool,
    clear_capabilities: bool,
) -> Result<(), CacheValidationError> {
    ensure_lock_live(lock, inst)?;
    let previous_catalog = file_hash_relative(inst, CATALOG_FILE, MAX_CATALOG_BYTES)?;
    let previous_caps = file_hash_relative(inst, CAPABILITIES_FILE, MAX_CAPABILITIES_BYTES)?;
    let previous_state = file_hash_relative(inst, STATE_FILE, MAX_STATE_BYTES)?;

    let catalog_tmp = match &catalog {
        Some((bytes, _)) => Some(stage_bytes_relative(inst, CATALOG_FILE, bytes)?),
        None => None,
    };
    let caps_tmp = match &capabilities {
        Some((bytes, _)) => Some(stage_bytes_relative(inst, CAPABILITIES_FILE, bytes)?),
        None => None,
    };
    let state_tmp = stage_bytes_relative(inst, STATE_FILE, &state_bytes)?;

    let marker = TxnMarker {
        version: TXN_VERSION,
        catalog_tmp: catalog_tmp.clone(),
        capabilities_tmp: caps_tmp.clone(),
        state_tmp: Some(state_tmp.clone()),
        catalog_sha256: catalog.as_ref().map(|(_, h)| h.clone()),
        capabilities_sha256: capabilities.as_ref().map(|(_, h)| h.clone()),
        state_sha256: Some(content_hash(&state_bytes)),
        previous_catalog_sha256: previous_catalog,
        previous_capabilities_sha256: previous_caps,
        previous_state_sha256: previous_state,
        clear_catalog,
        clear_capabilities,
        clear_all: false,
        partial_compat: false,
    };
    ensure_lock_live(lock, inst)?;
    write_marker(inst, &marker)?;
    maybe_fault(ProviderCacheTxnFault::AfterJournalFsync)?;

    if clear_catalog {
        ensure_lock_live(lock, inst)?;
        unlink_relative(inst, CATALOG_FILE)?;
    } else if let Some(tmp) = &catalog_tmp {
        ensure_lock_live(lock, inst)?;
        rename_relative(inst, tmp, CATALOG_FILE)?;
        maybe_fault(ProviderCacheTxnFault::AfterCatalogRename)?;
    }
    if clear_capabilities {
        ensure_lock_live(lock, inst)?;
        unlink_relative(inst, CAPABILITIES_FILE)?;
    } else if let Some(tmp) = &caps_tmp {
        ensure_lock_live(lock, inst)?;
        rename_relative(inst, tmp, CAPABILITIES_FILE)?;
        maybe_fault(ProviderCacheTxnFault::AfterCapabilitiesRename)?;
    }
    ensure_lock_live(lock, inst)?;
    rename_relative(inst, &state_tmp, STATE_FILE)?;
    maybe_fault(ProviderCacheTxnFault::AfterStateRename)?;
    maybe_fault(ProviderCacheTxnFault::BeforeMarkerRemoval)?;
    ensure_lock_live(lock, inst)?;
    unlink_relative(inst, TXN_FILE)?;
    Ok(())
}

fn commit_clear_payloads(
    inst: &TrustedInstanceDir,
    lock: &InstanceLock,
    clear_catalog: bool,
    clear_capabilities: bool,
    state_bytes: Vec<u8>,
) -> Result<(), CacheValidationError> {
    ensure_lock_live(lock, inst)?;
    let previous_catalog = file_hash_relative(inst, CATALOG_FILE, MAX_CATALOG_BYTES)?;
    let previous_caps = file_hash_relative(inst, CAPABILITIES_FILE, MAX_CAPABILITIES_BYTES)?;
    let previous_state = file_hash_relative(inst, STATE_FILE, MAX_STATE_BYTES)?;
    let state_tmp = stage_bytes_relative(inst, STATE_FILE, &state_bytes)?;
    let marker = TxnMarker {
        version: TXN_VERSION,
        catalog_tmp: None,
        capabilities_tmp: None,
        state_tmp: Some(state_tmp.clone()),
        catalog_sha256: None,
        capabilities_sha256: None,
        state_sha256: Some(content_hash(&state_bytes)),
        previous_catalog_sha256: previous_catalog,
        previous_capabilities_sha256: previous_caps,
        previous_state_sha256: previous_state,
        clear_catalog,
        clear_capabilities,
        clear_all: false,
        partial_compat: false,
    };
    ensure_lock_live(lock, inst)?;
    write_marker(inst, &marker)?;
    maybe_fault(ProviderCacheTxnFault::AfterJournalFsync)?;
    if clear_catalog {
        ensure_lock_live(lock, inst)?;
        unlink_relative(inst, CATALOG_FILE)?;
    }
    if clear_capabilities {
        ensure_lock_live(lock, inst)?;
        unlink_relative(inst, CAPABILITIES_FILE)?;
    }
    ensure_lock_live(lock, inst)?;
    rename_relative(inst, &state_tmp, STATE_FILE)?;
    maybe_fault(ProviderCacheTxnFault::AfterStateRename)?;
    unlink_relative(inst, TXN_FILE)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Legacy built-in import
// ---------------------------------------------------------------------------

const LEGACY_OPENAI: &str = "openai_models_cache.json";
const LEGACY_OPENROUTER: &str = "openrouter_models_cache.json";
const LEGACY_CODEX: &str = "codex_models_cache.json";
const LEGACY_ANTHROPIC: &str = "anthropic_models_cache.json";

fn legacy_file_for(
    built_in: BuiltInProviderId,
    identity: &ProviderCacheIdentity,
) -> Option<&'static str> {
    match built_in {
        BuiltInProviderId::OpenAi => {
            if identity.credential_route == CredentialRoute::ChatGptOauth
                || identity.api_surface == ApiSurface::ChatGptInference
            {
                Some(LEGACY_CODEX)
            } else {
                Some(LEGACY_OPENAI)
            }
        }
        BuiltInProviderId::OpenRouter => Some(LEGACY_OPENROUTER),
        BuiltInProviderId::Anthropic => Some(LEGACY_ANTHROPIC),
        BuiltInProviderId::Xai => None,
    }
}

fn read_legacy_builtin_catalog(
    grok_home: &Path,
    built_in: BuiltInProviderId,
    identity: &ProviderCacheIdentity,
) -> Result<Option<(&'static str, Vec<serde_json::Value>, Option<u64>)>, CacheValidationError> {
    let Some(name) = legacy_file_for(built_in, identity) else {
        return Ok(None);
    };
    let Some(bytes) = read_home_regular_nofollow(grok_home, name, MAX_CATALOG_BYTES)? else {
        return Ok(None);
    };
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| CacheValidationError::Corrupt(e.to_string()))?;
    let models = if let Some(arr) = value.get("models").and_then(|m| m.as_array()) {
        arr.clone()
    } else if let Some(arr) = value.as_array() {
        arr.clone()
    } else {
        return Err(CacheValidationError::Corrupt(
            "legacy cache missing models".into(),
        ));
    };
    if models.is_empty() {
        return Ok(None);
    }
    let fetched_at = value
        .get("fetched_at")
        .and_then(|v| v.as_u64())
        .or_else(|| value.get("fetched_at_unix").and_then(|v| v.as_u64()));
    Ok(Some((name, models, fetched_at)))
}
