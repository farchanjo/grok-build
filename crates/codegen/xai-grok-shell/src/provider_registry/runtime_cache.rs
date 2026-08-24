//! Cached provider service + lifecycle snapshot for hot paths.
//!
//! Invalidated when the lifecycle generation sidecar fingerprint changes or
//! when [`invalidate_for_home`] is called after a management mutation /
//! providers/update notify. Avoids full TOML parse + service rebuild on every
//! turn while remaining fail-closed on corrupt lifecycle state.

use super::lifecycle_state::{ProviderLifecycleState, lifecycle_state_path, load_lifecycle_state};
use super::management::ProviderManagementService;
use super::service::ProviderService;
use crate::agent::model_providers::parse_model_providers;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const MAX_AGE: Duration = Duration::from_secs(2);

struct CachedRuntime {
    home: PathBuf,
    /// generation file raw content fingerprint (generation + config sha).
    gen_fingerprint: String,
    generation: u64,
    service: ProviderService,
    lifecycle: ProviderLifecycleState,
    loaded_at: Instant,
}

static CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedRuntime>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<PathBuf, CachedRuntime>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn generation_fingerprint(home: &Path) -> String {
    let path = home.join("state/provider_lifecycle_generation");
    std::fs::read_to_string(&path).unwrap_or_default()
}

fn config_mtime_hint(home: &Path) -> u64 {
    std::fs::metadata(home.join("config.toml"))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Drop any cached snapshot for `home` (call after durable mutations).
pub fn invalidate_for_home(home: &Path) {
    if let Ok(mut map) = cache().lock() {
        map.remove(&home.to_path_buf());
    }
}

/// Invalidate all homes (tests).
#[cfg(test)]
pub fn invalidate_all() {
    if let Ok(mut map) = cache().lock() {
        map.clear();
    }
}

/// Load (or reuse) a coherent provider runtime for `home`.
///
/// Rebuilds when generation fingerprint, config mtime hint, or max age changes.
/// Fail-closed on corrupt lifecycle JSON (propagates as Err).
pub fn load_runtime(home: &Path) -> Result<(ProviderService, ProviderLifecycleState, u64), String> {
    let home_key = home.to_path_buf();
    let fp = generation_fingerprint(home);
    let mtime = config_mtime_hint(home);
    let combined_fp = format!("{fp}|{mtime}");

    if let Ok(map) = cache().lock()
        && let Some(hit) = map.get(&home_key)
        && hit.gen_fingerprint == combined_fp
        && hit.loaded_at.elapsed() < MAX_AGE
    {
        return Ok((hit.service.clone(), hit.lifecycle.clone(), hit.generation));
    }

    let generation = ProviderManagementService::new(home)
        .current_generation()
        .get();
    let lifecycle = load_lifecycle_state(home).map_err(|e| e.to_string())?;
    // Refuse symlink lifecycle path already handled inside load.
    let _ = lifecycle_state_path(home);

    let (entries, _) = match std::fs::read_to_string(home.join("config.toml")) {
        Ok(raw) => match toml::from_str::<toml::Value>(&raw) {
            Ok(val) => parse_model_providers(&val),
            Err(e) => return Err(format!("parse config.toml: {e}")),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            (indexmap::IndexMap::new(), Vec::new())
        }
        Err(e) => return Err(format!("read config.toml: {e}")),
    };
    let service = ProviderService::from_model_providers(&entries)
        .map_err(|e| e.to_string())?
        .with_lifecycle_incarnations(home)
        .with_generation(generation);

    if let Ok(mut map) = cache().lock() {
        map.insert(
            home_key,
            CachedRuntime {
                home: home.to_path_buf(),
                gen_fingerprint: combined_fp,
                generation,
                service: service.clone(),
                lifecycle: lifecycle.clone(),
                loaded_at: Instant::now(),
            },
        );
    }
    Ok((service, lifecycle, generation))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_registry::management::ProviderManagementService;
    use crate::provider_registry::management::dto::ProviderAddRequest;
    use tempfile::TempDir;

    #[test]
    fn cache_hits_until_invalidation() {
        invalidate_all();
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        let svc = ProviderManagementService::new(home);
        let g0 = svc.current_generation();
        assert!(
            svc.add(ProviderAddRequest {
                id: "lab".into(),
                kind: "openai_compatible".into(),
                base_url: "http://127.0.0.1:9/v1".into(),
                display_name: None,
                admin_base_url: None,
                enabled: true,
                expected_generation: g0,
            })
            .ok
        );
        invalidate_for_home(home);
        let (s1, _, g1) = load_runtime(home).unwrap();
        let (s2, _, g2) = load_runtime(home).unwrap();
        assert_eq!(g1, g2);
        assert_eq!(s1.generation(), s2.generation());
        assert!(s1.get("lab").is_some());
        // Mutation invalidates.
        let _ = svc.set_enabled("lab", false, svc.current_generation());
        invalidate_for_home(home);
        let (s3, _, _) = load_runtime(home).unwrap();
        assert!(!s3.get("lab").unwrap().enabled);
    }
}
