//! Disable / remove / incarnation fail-closed checks for request boundaries.
//!
//! In-flight work may complete. The next request/turn and any retry against a
//! disabled, removed, or replaced incarnation fails closed with an actionable,
//! secret-free error. Sibling accounts are never borrowed.
//!
//! ## Generation semantics (intentional split)
//!
//! **Legacy soft generation (this module):** a generation mismatch only fails
//! closed when `is_retry` is true **or** `provenance_incarnation` is set. A
//! fresh non-retry request that carries a stale generation pin without an
//! incarnation pin is allowed to proceed. Chat/auxiliary callers historically
//! rely on this soft path.
//!
//! **Retrieval-strict generation:** the retrieval runtime
//! (`retrieval_runtime`) performs its **own** precheck before calling this
//! guard: `session_registry_generation: Some(stale)` always fails against
//! live generation, independent of retry/incarnation. Do not assume
//! retrieval-strict behavior from this shared guard alone.

use super::id::ProviderId;
use super::instance::ProviderIncarnation;
use super::lifecycle_state::{
    ProviderLifecycleState, load_lifecycle_state, provenance_matches_lifecycle,
};
use super::service::ProviderService;
use std::path::Path;

/// Why a route cannot be used for a new request or retry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteGuardError {
    ProviderMissing {
        id: String,
    },
    ProviderDisabled {
        id: String,
    },
    IncarnationMismatch {
        id: String,
    },
    Tombstoned {
        id: String,
    },
    GenerationReplaced {
        id: String,
        expected: u64,
        live: u64,
    },
    SiblingIsolation {
        id: String,
    },
    /// Lifecycle state unreadable/corrupt — fail closed.
    LifecycleCorrupt {
        id: String,
    },
}

impl std::fmt::Display for RouteGuardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProviderMissing { id } => write!(
                f,
                "provider `{id}` is not configured; re-select a model or restore the provider (never remaps to a sibling account)"
            ),
            Self::ProviderDisabled { id } => write!(
                f,
                "provider `{id}` is disabled; enable it or switch models. In-flight work may finish but retries are blocked"
            ),
            Self::IncarnationMismatch { id } => write!(
                f,
                "provider `{id}` was recreated with a new incarnation; old sessions cannot reuse this route. Select the model again or restore the prior incarnation explicitly"
            ),
            Self::Tombstoned { id } => write!(
                f,
                "provider `{id}` was forcibly removed (tombstoned); old durable references will not rebind"
            ),
            Self::GenerationReplaced { id, expected, live } => write!(
                f,
                "provider `{id}` registry generation changed (session had {expected}, live {live}); reload and retry the request against the current route"
            ),
            Self::SiblingIsolation { id } => write!(
                f,
                "refusing to borrow credentials or routes from a sibling of `{id}`"
            ),
            Self::LifecycleCorrupt { id } => write!(
                f,
                "provider `{id}` lifecycle state is unreadable or corrupt; refusing route until state is repaired"
            ),
        }
    }
}

impl std::error::Error for RouteGuardError {}

/// Safe category for telemetry (bounded enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteGuardErrorCategory {
    Missing,
    Disabled,
    IncarnationMismatch,
    Tombstoned,
    GenerationReplaced,
    SiblingIsolation,
    LifecycleCorrupt,
}

impl RouteGuardError {
    pub fn category(&self) -> RouteGuardErrorCategory {
        match self {
            Self::ProviderMissing { .. } => RouteGuardErrorCategory::Missing,
            Self::ProviderDisabled { .. } => RouteGuardErrorCategory::Disabled,
            Self::IncarnationMismatch { .. } => RouteGuardErrorCategory::IncarnationMismatch,
            Self::Tombstoned { .. } => RouteGuardErrorCategory::Tombstoned,
            Self::GenerationReplaced { .. } => RouteGuardErrorCategory::GenerationReplaced,
            Self::SiblingIsolation { .. } => RouteGuardErrorCategory::SiblingIsolation,
            Self::LifecycleCorrupt { .. } => RouteGuardErrorCategory::LifecycleCorrupt,
        }
    }

    pub fn provider_id(&self) -> &str {
        match self {
            Self::ProviderMissing { id }
            | Self::ProviderDisabled { id }
            | Self::IncarnationMismatch { id }
            | Self::Tombstoned { id }
            | Self::GenerationReplaced { id, .. }
            | Self::SiblingIsolation { id }
            | Self::LifecycleCorrupt { id } => id,
        }
    }
}

impl RouteGuardErrorCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Disabled => "disabled",
            Self::IncarnationMismatch => "incarnation_mismatch",
            Self::Tombstoned => "tombstoned",
            Self::GenerationReplaced => "generation_replaced",
            Self::SiblingIsolation => "sibling_isolation",
            Self::LifecycleCorrupt => "lifecycle_corrupt",
        }
    }
}

/// Inputs for a next-request / retry boundary check.
#[derive(Debug, Clone)]
pub struct RouteGuardRequest<'a> {
    pub provider_instance_id: &'a str,
    pub provenance_incarnation: Option<&'a str>,
    pub session_registry_generation: Option<u64>,
    /// When true, this is a retry of in-flight work (still fail closed if
    /// disabled/removed/replaced; first attempt of in-flight may still finish
    /// outside this guard).
    pub is_retry: bool,
}

/// Validate that `provider_instance_id` may accept a new request or retry.
///
/// On failure, emits bounded `ProviderRouteGuardFailed` telemetry (no secrets).
pub fn assert_route_usable(
    home: &Path,
    service: &ProviderService,
    req: &RouteGuardRequest<'_>,
) -> Result<(), RouteGuardError> {
    match assert_route_usable_inner(home, service, req) {
        Ok(()) => Ok(()),
        Err(e) => {
            emit_route_guard_telemetry(&e, req);
            Err(e)
        }
    }
}

fn assert_route_usable_inner(
    home: &Path,
    service: &ProviderService,
    req: &RouteGuardRequest<'_>,
) -> Result<(), RouteGuardError> {
    let id = req.provider_instance_id;
    if ProviderId::new(id).is_err() {
        return Err(RouteGuardError::ProviderMissing { id: id.to_owned() });
    }

    let desc = service
        .get(id)
        .ok_or_else(|| RouteGuardError::ProviderMissing { id: id.to_owned() })?;

    if !desc.enabled {
        return Err(RouteGuardError::ProviderDisabled { id: id.to_owned() });
    }

    // Fail closed on unreadable/corrupt lifecycle state.
    let lifecycle = load_lifecycle_state(home)
        .map_err(|_| RouteGuardError::LifecycleCorrupt { id: id.to_owned() })?;

    if let Some(prov_inc) = req.provenance_incarnation {
        if let Ok(parsed) = ProviderIncarnation::new(prov_inc)
            && lifecycle.is_tombstoned(id, &parsed)
        {
            return Err(RouteGuardError::Tombstoned { id: id.to_owned() });
        }
        if !provenance_matches_lifecycle(&lifecycle, id, Some(prov_inc)) {
            return Err(RouteGuardError::IncarnationMismatch { id: id.to_owned() });
        }
        if let Some(live) = desc.incarnation.as_ref() {
            if live.as_str() != prov_inc {
                return Err(RouteGuardError::IncarnationMismatch { id: id.to_owned() });
            }
        } else if let Some(live) = lifecycle.incarnation_for(id)
            && live.as_str() != prov_inc
        {
            return Err(RouteGuardError::IncarnationMismatch { id: id.to_owned() });
        }
    }

    // Soft generation: only fail closed on mismatch for retries or when an
    // incarnation pin is present. Fresh non-retry requests without incarnation
    // do not fail here (see module docs). Retrieval applies a strict precheck.
    if let Some(expected_gen) = req.session_registry_generation {
        let live = service.generation();
        if live != 0 && expected_gen != 0 && live != expected_gen {
            if req.is_retry || req.provenance_incarnation.is_some() {
                return Err(RouteGuardError::GenerationReplaced {
                    id: id.to_owned(),
                    expected: expected_gen,
                    live,
                });
            }
        }
    }

    let _ = req.is_retry;
    Ok(())
}

fn emit_route_guard_telemetry(err: &RouteGuardError, req: &RouteGuardRequest<'_>) {
    use xai_grok_telemetry::events::{
        ProviderPurpose, ProviderRouteGuardFailed, sanitize_provider_instance_id,
    };
    let Some(safe_id) = sanitize_provider_instance_id(err.provider_id()) else {
        return;
    };
    xai_grok_telemetry::session_ctx::log_event(ProviderRouteGuardFailed {
        provider_instance_id: safe_id,
        purpose: ProviderPurpose::Chat,
        error_category: err.category().as_str().to_owned(),
        registry_generation: req.session_registry_generation.unwrap_or(0),
        is_retry: req.is_retry,
    });
}

/// Refuse resolving credentials for `requested` using material from `other`.
pub fn assert_not_sibling_borrow(requested: &str, other: &str) -> Result<(), RouteGuardError> {
    if requested != other {
        return Err(RouteGuardError::SiblingIsolation {
            id: requested.to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::model_providers::ModelProviderConfig;
    use crate::provider_registry::lifecycle_state::{
        ProviderLifecycleState, store_lifecycle_state,
    };
    use crate::provider_registry::management::ProviderManagementService;
    use crate::provider_registry::management::dto::{ProviderAddRequest, RegistryGeneration};
    use indexmap::IndexMap;
    use tempfile::TempDir;

    #[test]
    fn disabled_blocks_next_request_and_retry() {
        let dir = TempDir::new().unwrap();
        let svc = ProviderManagementService::new(dir.path());
        let g = svc.current_generation();
        assert!(
            svc.add(ProviderAddRequest {
                id: "lab".into(),
                kind: "openai_compatible".into(),
                base_url: "http://127.0.0.1:9/v1".into(),
                display_name: None,
                admin_base_url: None,
                enabled: true,
                expected_generation: g,
            })
            .ok
        );
        let g2 = svc.current_generation();
        assert!(svc.set_enabled("lab", false, g2).ok);

        let (entries, _) = {
            let raw = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
            let val: toml::Value = toml::from_str(&raw).unwrap();
            crate::agent::model_providers::parse_model_providers(&val)
        };
        let service = ProviderService::from_model_providers(&entries).unwrap();
        let err = assert_route_usable(
            dir.path(),
            &service,
            &RouteGuardRequest {
                provider_instance_id: "lab",
                provenance_incarnation: None,
                session_registry_generation: None,
                is_retry: false,
            },
        )
        .unwrap_err();
        assert!(matches!(err, RouteGuardError::ProviderDisabled { .. }));
        let err2 = assert_route_usable(
            dir.path(),
            &service,
            &RouteGuardRequest {
                provider_instance_id: "lab",
                provenance_incarnation: None,
                session_registry_generation: None,
                is_retry: true,
            },
        )
        .unwrap_err();
        assert!(matches!(err2, RouteGuardError::ProviderDisabled { .. }));
    }

    #[test]
    fn reincarnation_mismatch_fail_closed() {
        let dir = TempDir::new().unwrap();
        let mut state = ProviderLifecycleState::empty();
        let pid = ProviderId::new("lab").unwrap();
        let old = state.mint_or_restore(&pid, false).unwrap();
        state.tombstone_remove(&pid, Some(&old)).unwrap();
        // Restore is not used; re-add blocked — simulate clone new id instead.
        let pid2 = ProviderId::new("lab2").unwrap();
        let _new = state.mint_or_restore(&pid2, false).unwrap();
        store_lifecycle_state(dir.path(), &state).unwrap();

        let mut entries = IndexMap::new();
        entries.insert(
            "lab2".into(),
            ModelProviderConfig {
                base_url: Some("http://127.0.0.1:9/v1".into()),
                enabled: true,
                ..Default::default()
            },
        );
        let service = ProviderService::from_model_providers(&entries).unwrap();
        // Old incarnation on lab2 must not match.
        let err = assert_route_usable(
            dir.path(),
            &service,
            &RouteGuardRequest {
                provider_instance_id: "lab2",
                provenance_incarnation: Some(old.as_str()),
                session_registry_generation: None,
                is_retry: true,
            },
        )
        .unwrap_err();
        assert!(matches!(
            err,
            RouteGuardError::IncarnationMismatch { .. } | RouteGuardError::Tombstoned { .. }
        ));
        let _ = RegistryGeneration(0);
    }

    #[test]
    fn sibling_isolation() {
        assert!(assert_not_sibling_borrow("a", "a").is_ok());
        assert!(assert_not_sibling_borrow("a", "b").is_err());
    }

    #[test]
    fn shared_generation_mismatch_is_soft_without_retry_or_incarnation() {
        // Documents the intentional split: soft guard allows stale generation on
        // a fresh non-retry request without incarnation pin. Retrieval applies a
        // strict precheck outside this function.
        let dir = TempDir::new().unwrap();
        let mut entries = IndexMap::new();
        entries.insert(
            "lab".into(),
            ModelProviderConfig {
                base_url: Some("http://127.0.0.1:9/v1".into()),
                enabled: true,
                ..Default::default()
            },
        );
        let service = ProviderService::from_model_providers(&entries)
            .unwrap()
            .with_generation(5);
        // Soft: no retry, no incarnation → ok despite stale pin.
        assert!(
            assert_route_usable(
                dir.path(),
                &service,
                &RouteGuardRequest {
                    provider_instance_id: "lab",
                    provenance_incarnation: None,
                    session_registry_generation: Some(1),
                    is_retry: false,
                },
            )
            .is_ok()
        );
        // Hard on retry.
        let err = assert_route_usable(
            dir.path(),
            &service,
            &RouteGuardRequest {
                provider_instance_id: "lab",
                provenance_incarnation: None,
                session_registry_generation: Some(1),
                is_retry: true,
            },
        )
        .unwrap_err();
        assert!(matches!(err, RouteGuardError::GenerationReplaced { .. }));
    }

    #[test]
    fn corrupt_lifecycle_state_fails_closed() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("state");
        std::fs::create_dir_all(&state_path).unwrap();
        std::fs::write(state_path.join("provider_lifecycle.json"), b"{not json").unwrap();
        let mut entries = IndexMap::new();
        entries.insert(
            "lab".into(),
            ModelProviderConfig {
                base_url: Some("http://127.0.0.1:9/v1".into()),
                enabled: true,
                ..Default::default()
            },
        );
        let service = ProviderService::from_model_providers(&entries).unwrap();
        let err = assert_route_usable(
            dir.path(),
            &service,
            &RouteGuardRequest {
                provider_instance_id: "lab",
                provenance_incarnation: None,
                session_registry_generation: None,
                is_retry: true,
            },
        )
        .unwrap_err();
        assert!(matches!(err, RouteGuardError::LifecycleCorrupt { .. }));
    }
}
