//! Grok-owned provider lifecycle identity (incarnations + tombstones).
//!
//! Persisted outside user-authored `config.toml` under
//! `$GROK_HOME/state/provider_lifecycle.json`. Owner-only, no-follow where
//! practical, locked via the shared provider lifecycle flock, unique-temp
//! durable rename. Built-in product providers keep stable compatibility
//! incarnations; configured add/clone always mint a new UUID. Forced remove
//! records a tombstone `(id, incarnation)` so reusing the human ID creates a
//! new incarnation that never matches old route provenance.

use super::id::{BuiltInProviderId, ProviderId};
use super::instance::ProviderIncarnation;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Relative path under `$GROK_HOME`.
pub const LIFECYCLE_STATE_REL: &str = "state/provider_lifecycle.json";
const SCHEMA_VERSION: u32 = 1;
const MAX_STATE_BYTES: u64 = 512 * 1024;
const MAX_TOMBSTONES: usize = 4096;
const MAX_INSTANCES: usize = 2048;

/// Stable built-in incarnations (never change across upgrades).
const BUILTIN_OPENAI: &str = "00000000-0000-4000-8000-000000000001";
const BUILTIN_OPENROUTER: &str = "00000000-0000-4000-8000-000000000002";
const BUILTIN_XAI: &str = "00000000-0000-4000-8000-000000000003";
const BUILTIN_ANTHROPIC: &str = "00000000-0000-4000-8000-000000000004";
const BUILTIN_ZAI: &str = "00000000-0000-4000-8000-000000000005";

/// One live configured (or restored) instance record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceLifecycleRecord {
    pub incarnation: ProviderIncarnation,
    /// When true, this id was restored from a tombstone (distinct from re-add).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub restored: bool,
}

/// Tombstone for a forcibly removed `(id, incarnation)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderTombstone {
    pub id: String,
    pub incarnation: ProviderIncarnation,
}

/// Durable lifecycle registry (secret-free).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProviderLifecycleState {
    #[serde(default = "schema_version_default")]
    pub schema_version: u32,
    /// Live instances keyed by validated provider id.
    #[serde(default)]
    pub instances: BTreeMap<String, InstanceLifecycleRecord>,
    /// Forced-removal tombstones (bounded).
    #[serde(default)]
    pub tombstones: Vec<ProviderTombstone>,
}

fn schema_version_default() -> u32 {
    SCHEMA_VERSION
}

impl ProviderLifecycleState {
    pub fn empty() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            instances: BTreeMap::new(),
            tombstones: Vec::new(),
        }
    }

    /// Resolve the live incarnation for `id`, minting built-in stable values.
    pub fn incarnation_for(&self, id: &str) -> Option<ProviderIncarnation> {
        if let Some(rec) = self.instances.get(id) {
            return Some(rec.incarnation.clone());
        }
        stable_builtin_incarnation(id)
    }

    /// Whether `(id, incarnation)` is tombstoned (old provenance must not rebind).
    pub fn is_tombstoned(&self, id: &str, incarnation: &ProviderIncarnation) -> bool {
        self.tombstones
            .iter()
            .any(|t| t.id == id && t.incarnation == *incarnation)
    }

    /// Whether the human id currently has an active (non-tombstone) record, or
    /// any tombstone blocking silent re-add (force-remove without restore).
    pub fn has_blocking_tombstone_for_id(&self, id: &str) -> bool {
        // If a live record exists, re-add is blocked by config presence instead.
        // Tombstones without a live instance block ordinary re-add.
        !self.instances.contains_key(id) && self.tombstones.iter().any(|t| t.id == id)
    }

    /// Latest tombstone incarnation for an id, if any.
    pub fn latest_tombstone_incarnation(&self, id: &str) -> Option<&ProviderIncarnation> {
        self.tombstones
            .iter()
            .rev()
            .find(|t| t.id == id)
            .map(|t| &t.incarnation)
    }

    /// Mint a new incarnation for add/clone. Fails if id has a blocking tombstone
    /// unless `restore` is true (restore reuses the tombstoned incarnation).
    pub fn mint_or_restore(
        &mut self,
        id: &ProviderId,
        restore: bool,
    ) -> Result<ProviderIncarnation, LifecycleStateError> {
        let key = id.as_str();
        if BuiltInProviderId::parse(key).is_some() || key == crate::agent::zai::ZAI_PROVIDER_ID {
            return stable_builtin_incarnation(key).ok_or(LifecycleStateError::InvalidId);
        }
        if let Some(existing) = self.instances.get(key) {
            return Ok(existing.incarnation.clone());
        }
        if restore {
            if let Some(inc) = self
                .tombstones
                .iter()
                .rev()
                .find(|t| t.id == key)
                .map(|t| t.incarnation.clone())
            {
                // Drop matching tombstones for this incarnation (restored).
                self.tombstones
                    .retain(|t| !(t.id == key && t.incarnation == inc));
                self.instances.insert(
                    key.to_owned(),
                    InstanceLifecycleRecord {
                        incarnation: inc.clone(),
                        restored: true,
                    },
                );
                return Ok(inc);
            }
            return Err(LifecycleStateError::NoTombstoneToRestore);
        }
        if self.has_blocking_tombstone_for_id(key) {
            return Err(LifecycleStateError::TombstonedId { id: key.to_owned() });
        }
        if self.instances.len() >= MAX_INSTANCES {
            return Err(LifecycleStateError::BoundExceeded("instances"));
        }
        let incarnation = mint_incarnation();
        self.instances.insert(
            key.to_owned(),
            InstanceLifecycleRecord {
                incarnation: incarnation.clone(),
                restored: false,
            },
        );
        Ok(incarnation)
    }

    /// Ensure a live record exists for a configured id (lazy mint on first use).
    pub fn ensure_live(
        &mut self,
        id: &ProviderId,
    ) -> Result<ProviderIncarnation, LifecycleStateError> {
        if let Some(inc) = self.incarnation_for(id.as_str()) {
            if !self.instances.contains_key(id.as_str())
                && stable_builtin_incarnation(id.as_str()).is_some()
            {
                // Built-ins stay virtual; do not persist unless needed.
                return Ok(inc);
            }
            if self.instances.contains_key(id.as_str()) {
                return Ok(inc);
            }
        }
        // Configured provider present in TOML but missing lifecycle row: mint
        // only when not blocked by tombstone.
        self.mint_or_restore(id, false)
    }

    /// Record a forced-removal tombstone and drop the live instance row.
    pub fn tombstone_remove(
        &mut self,
        id: &ProviderId,
        expected_incarnation: Option<&ProviderIncarnation>,
    ) -> Result<ProviderIncarnation, LifecycleStateError> {
        let key = id.as_str();
        if BuiltInProviderId::parse(key).is_some() {
            return Err(LifecycleStateError::BuiltInNotRemovable);
        }
        let incarnation = if let Some(rec) = self.instances.remove(key) {
            if let Some(exp) = expected_incarnation
                && rec.incarnation != *exp
            {
                // Put back; incarnation mismatch.
                let inc = rec.incarnation.clone();
                self.instances.insert(key.to_owned(), rec);
                return Err(LifecycleStateError::IncarnationMismatch {
                    expected: exp.as_str().to_owned(),
                    actual: inc.as_str().to_owned(),
                });
            }
            rec.incarnation
        } else if let Some(exp) = expected_incarnation {
            exp.clone()
        } else {
            return Err(LifecycleStateError::NotFound);
        };
        if self.tombstones.len() >= MAX_TOMBSTONES {
            // Drop oldest to stay bounded.
            self.tombstones.remove(0);
        }
        self.tombstones.push(ProviderTombstone {
            id: key.to_owned(),
            incarnation: incarnation.clone(),
        });
        Ok(incarnation)
    }

    /// Drop live record without tombstone (clean remove when no refs — still
    /// mint-on-readd yields a new incarnation). Preferred path still tombstones
    /// on forced remove only; clean remove without tombstone is allowed when
    /// the caller opts into non-forced delete with zero references.
    pub fn forget_live(&mut self, id: &str) {
        self.instances.remove(id);
    }
}

/// Errors for lifecycle state operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleStateError {
    Io(String),
    Corrupt(String),
    TombstonedId { id: String },
    NoTombstoneToRestore,
    BuiltInNotRemovable,
    NotFound,
    IncarnationMismatch { expected: String, actual: String },
    BoundExceeded(&'static str),
    InvalidId,
}

impl std::fmt::Display for LifecycleStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "lifecycle state I/O: {e}"),
            Self::Corrupt(e) => write!(f, "lifecycle state corrupt: {e}"),
            Self::TombstonedId { id } => write!(
                f,
                "provider `{id}` was forcibly removed; use explicit restore or a new id (Clone)"
            ),
            Self::NoTombstoneToRestore => {
                write!(f, "no tombstone exists to restore for this provider id")
            }
            Self::BuiltInNotRemovable => write!(f, "built-in providers cannot be removed"),
            Self::NotFound => write!(f, "provider lifecycle record not found"),
            Self::IncarnationMismatch { expected, actual } => write!(
                f,
                "incarnation mismatch: expected {expected}, live {actual}"
            ),
            Self::BoundExceeded(what) => write!(f, "lifecycle state bound exceeded ({what})"),
            Self::InvalidId => write!(f, "invalid provider id for lifecycle state"),
        }
    }
}

impl std::error::Error for LifecycleStateError {}

/// Path to the durable lifecycle state file.
pub fn lifecycle_state_path(home: &Path) -> PathBuf {
    home.join(LIFECYCLE_STATE_REL)
}

/// Load lifecycle state (empty when missing). Corrupt files fail closed.
pub fn load_lifecycle_state(home: &Path) -> Result<ProviderLifecycleState, LifecycleStateError> {
    let path = lifecycle_state_path(home);
    match fs::metadata(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(ProviderLifecycleState::empty()),
        Err(e) => Err(LifecycleStateError::Io(e.to_string())),
        Ok(meta) => {
            if meta.len() > MAX_STATE_BYTES {
                return Err(LifecycleStateError::Corrupt(format!(
                    "state file exceeds {} bytes",
                    MAX_STATE_BYTES
                )));
            }
            let raw = fs::read(&path).map_err(|e| LifecycleStateError::Io(e.to_string()))?;
            if raw.is_empty() {
                return Ok(ProviderLifecycleState::empty());
            }
            let state: ProviderLifecycleState = serde_json::from_slice(&raw)
                .map_err(|e| LifecycleStateError::Corrupt(e.to_string()))?;
            if state.schema_version > SCHEMA_VERSION {
                return Err(LifecycleStateError::Corrupt(format!(
                    "unsupported schema_version {}",
                    state.schema_version
                )));
            }
            Ok(state)
        }
    }
}

/// Persist lifecycle state with unique temp + rename (best-effort owner-only).
pub fn store_lifecycle_state(
    home: &Path,
    state: &ProviderLifecycleState,
) -> Result<(), LifecycleStateError> {
    let path = lifecycle_state_path(home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| LifecycleStateError::Io(e.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
        }
    }
    // Refuse symlink final path.
    if path
        .symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(LifecycleStateError::Io(
            "refusing to write lifecycle state through a symlink".into(),
        ));
    }
    let bytes =
        serde_json::to_vec_pretty(state).map_err(|e| LifecycleStateError::Io(e.to_string()))?;
    let tmp_name = format!("provider_lifecycle.{}.tmp", std::process::id());
    let tmp = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(tmp_name);
    {
        let mut f = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&tmp)
            .map_err(|e| LifecycleStateError::Io(e.to_string()))?;
        f.write_all(&bytes)
            .map_err(|e| LifecycleStateError::Io(e.to_string()))?;
        f.sync_all()
            .map_err(|e| LifecycleStateError::Io(e.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
        }
    }
    fs::rename(&tmp, &path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        LifecycleStateError::Io(e.to_string())
    })?;
    // Best-effort parent sync.
    if let Some(parent) = path.parent()
        && let Ok(dir) = File::open(parent)
    {
        let _ = dir.sync_all();
    }
    Ok(())
}

/// Load-or-empty, apply `f`, store when `f` returns Ok(true) or always stores
/// when the mutation returns a value requiring persistence.
pub fn with_lifecycle_state_mut<T>(
    home: &Path,
    f: impl FnOnce(&mut ProviderLifecycleState) -> Result<(T, bool), LifecycleStateError>,
) -> Result<T, LifecycleStateError> {
    let mut state = load_lifecycle_state(home)?;
    let (out, dirty) = f(&mut state)?;
    if dirty {
        store_lifecycle_state(home, &state)?;
    }
    Ok(out)
}

/// Stable built-in incarnation for product providers.
pub fn stable_builtin_incarnation(id: &str) -> Option<ProviderIncarnation> {
    let raw = match BuiltInProviderId::parse(id) {
        Some(BuiltInProviderId::OpenAi) => BUILTIN_OPENAI,
        Some(BuiltInProviderId::OpenRouter) => BUILTIN_OPENROUTER,
        Some(BuiltInProviderId::Xai) => BUILTIN_XAI,
        Some(BuiltInProviderId::Anthropic) => BUILTIN_ANTHROPIC,
        None if id == crate::agent::zai::ZAI_PROVIDER_ID => BUILTIN_ZAI,
        None => return None,
    };
    ProviderIncarnation::new(raw).ok()
}

fn mint_incarnation() -> ProviderIncarnation {
    // uuid crate is already a workspace dependency via shell.
    let raw = uuid::Uuid::new_v4().to_string();
    ProviderIncarnation::new(raw).expect("uuid v4 is canonical")
}

/// Whether live route provenance may match: id + incarnation, and not tombstoned.
pub fn provenance_matches_lifecycle(
    state: &ProviderLifecycleState,
    provider_id: &str,
    provenance_incarnation: Option<&str>,
) -> bool {
    let Some(live) = state.incarnation_for(provider_id) else {
        // No lifecycle row and not built-in: only allow when provenance has no incarnation
        // (pre-upgrade legacy).
        return provenance_incarnation.is_none();
    };
    match provenance_incarnation {
        None => true, // pre-upgrade reference: resolve through frozen legacy rules
        Some(inc) => {
            if live.as_str() != inc {
                return false;
            }
            if let Ok(parsed) = ProviderIncarnation::new(inc) {
                if state.is_tombstoned(provider_id, &parsed) {
                    return false;
                }
            }
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn mint_tombstone_blocks_readd_until_restore() {
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        let pid = ProviderId::new("work_openai").unwrap();
        let mut state = ProviderLifecycleState::empty();
        let inc = state.mint_or_restore(&pid, false).unwrap();
        store_lifecycle_state(home, &state).unwrap();

        let mut state = load_lifecycle_state(home).unwrap();
        let tomb = state.tombstone_remove(&pid, Some(&inc)).unwrap();
        assert_eq!(tomb, inc);
        assert!(state.has_blocking_tombstone_for_id("work_openai"));
        store_lifecycle_state(home, &state).unwrap();

        let mut state = load_lifecycle_state(home).unwrap();
        let err = state.mint_or_restore(&pid, false).unwrap_err();
        assert!(matches!(err, LifecycleStateError::TombstonedId { .. }));

        let restored = state.mint_or_restore(&pid, true).unwrap();
        assert_eq!(restored, inc);
        assert!(!state.has_blocking_tombstone_for_id("work_openai"));
    }

    #[test]
    fn readd_without_tombstone_gets_new_incarnation() {
        let mut state = ProviderLifecycleState::empty();
        let pid = ProviderId::new("lab").unwrap();
        let a = state.mint_or_restore(&pid, false).unwrap();
        state.forget_live("lab");
        let b = state.mint_or_restore(&pid, false).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn tombstone_blocks_provenance_match() {
        let mut state = ProviderLifecycleState::empty();
        let pid = ProviderId::new("lab").unwrap();
        let inc = state.mint_or_restore(&pid, false).unwrap();
        assert!(provenance_matches_lifecycle(
            &state,
            "lab",
            Some(inc.as_str())
        ));
        state.tombstone_remove(&pid, Some(&inc)).unwrap();
        // After tombstone without live row, incarnation_for is None → provenance with
        // incarnation must not match a missing live record for recreated id.
        // Re-add would mint new; old provenance must fail.
        assert!(!provenance_matches_lifecycle(
            &state,
            "lab",
            Some(inc.as_str())
        ));
        let new_inc = state.mint_or_restore(&pid, false);
        // blocked by tombstone
        assert!(new_inc.is_err());
    }

    #[test]
    fn builtins_are_stable() {
        let a = stable_builtin_incarnation("openai").unwrap();
        let b = stable_builtin_incarnation("openai").unwrap();
        assert_eq!(a, b);
        assert_ne!(
            stable_builtin_incarnation("openai").unwrap(),
            stable_builtin_incarnation("openrouter").unwrap()
        );
    }

    #[test]
    fn durable_round_trip() {
        let dir = TempDir::new().unwrap();
        let mut state = ProviderLifecycleState::empty();
        let pid = ProviderId::new("a").unwrap();
        let inc = state.mint_or_restore(&pid, false).unwrap();
        store_lifecycle_state(dir.path(), &state).unwrap();
        let loaded = load_lifecycle_state(dir.path()).unwrap();
        assert_eq!(loaded.incarnation_for("a").as_ref(), Some(&inc));
    }
}
