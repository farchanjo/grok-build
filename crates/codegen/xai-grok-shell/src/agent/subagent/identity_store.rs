//! Typed, secret-free durable exact-route assignment records.
//!
//! Assigned metadata is always addressed from the parent session root. The
//! fixed `subagents/{id}` target is trusted-walked by the storage layer; callers
//! never supply a pre-joined target or a child-session root.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{SubagentMeta, assignment::AssignmentKey, exact_route::ExactRoute};

const MAX_META_BYTES: usize = 1024 * 1024;
const MAX_IDENTITY_BYTES: usize = 16 * 1024;
const VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct AssignmentIdentity {
    version: u32,
    assignment_key: String,
    child_session_id: String,
    canonical_model: String,
    upstream_model: String,
    provider_instance_id: String,
    provider_incarnation: Option<String>,
    provider_kind: String,
    api_surface: String,
    credential_route: String,
    registry_generation: u64,
    binding_generation: u64,
    model_partition: Option<String>,
}

impl AssignmentIdentity {
    fn from_route(key: &AssignmentKey, child_session_id: &str, route: &ExactRoute) -> Self {
        let context = route.context();
        Self {
            version: VERSION,
            assignment_key: key.as_str().to_owned(),
            child_session_id: child_session_id.to_owned(),
            canonical_model: route.canonical().as_str().to_owned(),
            upstream_model: route.upstream().as_str().to_owned(),
            provider_instance_id: context.instance_id().to_owned(),
            provider_incarnation: context.incarnation().map(str::to_owned),
            provider_kind: context.provider_kind().as_str().to_owned(),
            api_surface: context.api_surface().as_str().to_owned(),
            credential_route: context.credential_route().as_str().to_owned(),
            registry_generation: context.registry_generation(),
            binding_generation: context.binding_generation(),
            model_partition: context.model_partition().map(str::to_owned),
        }
    }

    pub(crate) fn matches_live(&self, live: &ExactRoute) -> bool {
        let context = live.context();
        self.canonical_model == live.canonical().as_str()
            && self.upstream_model == live.upstream().as_str()
            && self.provider_instance_id == context.instance_id()
            && self.provider_incarnation.as_deref() == context.incarnation()
            && self.provider_kind == context.provider_kind().as_str()
            && self.api_surface == context.api_surface().as_str()
            && self.credential_route == context.credential_route().as_str()
            && self.registry_generation == context.registry_generation()
            && self.binding_generation == context.binding_generation()
            && self.model_partition.as_deref() == context.model_partition()
            && self.model_partition.as_deref() == Some(self.upstream_model.as_str())
    }
}

/// Capability for mutating one assigned metadata record. Both the durable
/// transaction generation and exact assignment identity travel together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AssignedMetaOwner {
    owner_generation: String,
    identity: AssignmentIdentity,
}

impl AssignedMetaOwner {
    pub(crate) fn identity(&self) -> &AssignmentIdentity {
        &self.identity
    }

    pub(crate) fn matches(&self, other: &Self) -> bool {
        self == other
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Lookup {
    Missing,
    LegacyUnassigned {
        meta: SubagentMeta,
    },
    Assigned {
        meta: SubagentMeta,
        owner: AssignedMetaOwner,
    },
}

#[cfg(test)]
pub(crate) fn owner_for_test(label: &str) -> AssignedMetaOwner {
    AssignedMetaOwner {
        owner_generation: format!("00000000-0000-0000-0000-{label:0>12}"),
        identity: AssignmentIdentity {
            version: VERSION,
            assignment_key: format!("key-{label}"),
            child_session_id: format!("child-{label}"),
            canonical_model: format!("account-{label}:gpt-4o"),
            upstream_model: "gpt-4o".into(),
            provider_instance_id: format!("account-{label}"),
            provider_incarnation: None,
            provider_kind: "openai".into(),
            api_surface: "openai_platform".into(),
            credential_route: "api_key".into(),
            registry_generation: 1,
            binding_generation: 1,
            model_partition: Some("gpt-4o".into()),
        },
    }
}

fn serialize_meta(meta: &SubagentMeta) -> io::Result<Vec<u8>> {
    let bytes = serde_json::to_vec_pretty(meta).map_err(io::Error::other)?;
    if bytes.len() > MAX_META_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "subagent metadata exceeds size limit",
        ));
    }
    Ok(bytes)
}

fn serialize_identity(identity: &AssignmentIdentity) -> io::Result<Vec<u8>> {
    let bytes = serde_json::to_vec(identity).map_err(io::Error::other)?;
    if bytes.len() > MAX_IDENTITY_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "assignment identity exceeds size limit",
        ));
    }
    Ok(bytes)
}

fn validate_assigned(
    subagent_id: &str,
    meta: &SubagentMeta,
    identity: &AssignmentIdentity,
) -> io::Result<()> {
    if identity.version != VERSION
        || AssignmentKey::new(identity.assignment_key.clone()).is_none()
        || meta.subagent_id != subagent_id
        || identity.child_session_id != meta.child_session_id
        || meta.effective_model_id.as_deref() != Some(identity.canonical_model.as_str())
        || identity.model_partition.as_deref() != Some(identity.upstream_model.as_str())
        || xai_grok_models::CanonicalModelId::new(identity.canonical_model.clone()).is_err()
        || xai_grok_models::UpstreamModelId::new(identity.upstream_model.clone()).is_err()
        || xai_grok_inference::RouteProviderKind::parse(&identity.provider_kind).is_none()
        || xai_grok_inference::RouteApiSurface::parse(&identity.api_surface).is_none()
        || xai_grok_inference::RouteCredentialRoute::parse(&identity.credential_route).is_none()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "assigned metadata and identity are inconsistent",
        ));
    }
    Ok(())
}

pub(crate) fn commit_initial(
    parent_session_dir: &Path,
    subagent_id: &str,
    meta: &SubagentMeta,
    key: &AssignmentKey,
    route: &ExactRoute,
) -> io::Result<AssignedMetaOwner> {
    let identity = AssignmentIdentity::from_route(key, &meta.child_session_id, route);
    validate_assigned(subagent_id, meta, &identity)?;
    let primary = serialize_meta(meta)?;
    let companion = serialize_identity(&identity)?;
    let owner_generation =
        crate::session::storage::model_route::commit_private_identity_pair_for_subagent(
            parent_session_dir,
            subagent_id,
            &primary,
            &companion,
        )?;
    Ok(AssignedMetaOwner {
        owner_generation,
        identity,
    })
}

pub(crate) fn update_expected(
    parent_session_dir: &Path,
    subagent_id: &str,
    meta: &SubagentMeta,
    owner: &AssignedMetaOwner,
) -> io::Result<()> {
    validate_assigned(subagent_id, meta, &owner.identity)?;
    let primary = serialize_meta(meta)?;
    crate::session::storage::model_route::update_private_identity_pair_for_subagent(
        parent_session_dir,
        subagent_id,
        &owner.owner_generation,
        Some(&primary),
        None,
    )
}

pub(crate) fn replace(
    parent_session_dir: &Path,
    subagent_id: &str,
    meta: &SubagentMeta,
    key: &AssignmentKey,
    route: &ExactRoute,
) -> io::Result<AssignedMetaOwner> {
    let identity = AssignmentIdentity::from_route(key, &meta.child_session_id, route);
    validate_assigned(subagent_id, meta, &identity)?;
    let primary = serialize_meta(meta)?;
    let companion = serialize_identity(&identity)?;
    let owner_generation =
        crate::session::storage::model_route::replace_private_identity_pair_for_subagent(
            parent_session_dir,
            subagent_id,
            &primary,
            &companion,
        )?;
    Ok(AssignedMetaOwner {
        owner_generation,
        identity,
    })
}

pub(crate) fn lookup(parent_session_dir: &Path, subagent_id: &str) -> io::Result<Lookup> {
    use crate::session::storage::model_route::PrivateIdentityPair;

    match crate::session::storage::model_route::load_private_identity_pair_for_subagent(
        parent_session_dir,
        subagent_id,
        MAX_META_BYTES,
        MAX_IDENTITY_BYTES,
    ) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Lookup::Missing),
        Err(error) => Err(error),
        Ok(PrivateIdentityPair::Missing) => Ok(Lookup::Missing),
        Ok(PrivateIdentityPair::LegacyPrimary(primary)) => {
            let meta = serde_json::from_slice(&primary).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "malformed subagent metadata")
            })?;
            Ok(Lookup::LegacyUnassigned { meta })
        }
        Ok(PrivateIdentityPair::ValidPair {
            primary,
            companion,
            owner_generation,
        }) => {
            let meta: SubagentMeta = serde_json::from_slice(&primary).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "malformed assigned subagent metadata",
                )
            })?;
            let identity: AssignmentIdentity =
                serde_json::from_slice(&companion).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "malformed assignment identity")
                })?;
            validate_assigned(subagent_id, &meta, &identity)?;
            Ok(Lookup::Assigned {
                meta,
                owner: AssignedMetaOwner {
                    owner_generation,
                    identity,
                },
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_inference::{
        ProviderRouteContext, RouteApiSurface, RouteAuthority, RouteCredentialRoute,
        RouteProviderKind,
    };
    use xai_grok_models::{CanonicalModelId, UpstreamModelId};

    fn route(instance: &str, incarnation: &str, binding_generation: u64) -> ExactRoute {
        ExactRoute::new(
            CanonicalModelId::new(format!("{instance}:gpt-4o")).unwrap(),
            UpstreamModelId::new("gpt-4o").unwrap(),
            ProviderRouteContext::builder()
                .instance_id(instance)
                .incarnation(incarnation)
                .provider_kind(RouteProviderKind::OpenAi)
                .api_surface(RouteApiSurface::OpenAiPlatform)
                .credential_route(RouteCredentialRoute::ApiKey)
                .registry_generation(9)
                .binding_generation(binding_generation)
                .authority(RouteAuthority::Authoritative)
                .model_partition("gpt-4o")
                .build()
                .unwrap(),
        )
        .unwrap()
    }

    fn meta(id: &str, canonical: &str) -> SubagentMeta {
        SubagentMeta {
            subagent_id: id.to_owned(),
            parent_session_id: "parent".into(),
            child_session_id: format!("child-{id}"),
            subagent_type: "general-purpose".into(),
            description: "assigned task".into(),
            prompt: "work".into(),
            status: "running".into(),
            started_at: chrono::Utc::now(),
            completed_at: None,
            duration_ms: None,
            tool_calls: None,
            turns: None,
            error: None,
            effective_context_source: Some("new".into()),
            context_normalized: false,
            fork_copy_error: None,
            persona: None,
            resumed_from: None,
            child_cwd: Some("/workspace".into()),
            worktree_path: None,
            snapshot_ref: None,
            effective_model_id: Some(canonical.into()),
            codex_thread_id: None,
            codex_provider: None,
            codex_sandbox: None,
            external_runtime_kind: None,
            external_session_pointer: None,
        }
    }

    #[test]
    fn parent_target_commits_only_under_fixed_subagent_directory() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("parent");
        std::fs::create_dir(&parent).unwrap();
        let route = route("account-a", "11111111-1111-1111-1111-111111111111", 4);
        let meta = meta("assigned-1", route.canonical().as_str());
        commit_initial(
            &parent,
            "assigned-1",
            &meta,
            &AssignmentKey::new("key-1").unwrap(),
            &route,
        )
        .unwrap();

        let target = parent.join("subagents/assigned-1");
        for name in [
            "meta.json",
            "assignment_identity.json",
            "private_identity.meta",
        ] {
            assert!(target.join(name).is_file(), "missing {name}");
        }
        assert!(!parent.join("child-assigned-1").exists());
        assert!(!root.path().join("child-assigned-1").exists());
    }

    #[cfg(unix)]
    #[test]
    fn parent_target_rejects_invalid_id_and_symlinked_components() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("parent");
        let outside = root.path().join("outside");
        std::fs::create_dir(&parent).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let route = route("account-a", "11111111-1111-1111-1111-111111111111", 4);
        let meta = meta("assigned-1", route.canonical().as_str());
        let key = AssignmentKey::new("key-1").unwrap();
        for bad in ["", ".", "..", "a/b", "a\\b", "unsafe id"] {
            assert!(
                commit_initial(&parent, bad, &meta, &key, &route).is_err(),
                "{bad:?}"
            );
        }

        symlink(&outside, parent.join("subagents")).unwrap();
        assert!(commit_initial(&parent, "assigned-1", &meta, &key, &route).is_err());
        std::fs::remove_file(parent.join("subagents")).unwrap();
        std::fs::create_dir(parent.join("subagents")).unwrap();
        std::fs::set_permissions(parent.join("subagents"), PermissionsExt::from_mode(0o700))
            .unwrap();
        symlink(&outside, parent.join("subagents/assigned-1")).unwrap();
        assert!(commit_initial(&parent, "assigned-1", &meta, &key, &route).is_err());
    }

    #[test]
    fn assigned_lifecycle_round_trips_and_stale_owner_cannot_update_replacement() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("parent");
        std::fs::create_dir(&parent).unwrap();
        let route_a = route("account-a", "11111111-1111-1111-1111-111111111111", 4);
        let mut meta_a = meta("assigned-1", route_a.canonical().as_str());
        let owner_a = commit_initial(
            &parent,
            "assigned-1",
            &meta_a,
            &AssignmentKey::new("key-a").unwrap(),
            &route_a,
        )
        .unwrap();
        meta_a.status = "completed".into();
        update_expected(&parent, "assigned-1", &meta_a, &owner_a).unwrap();
        meta_a.snapshot_ref = Some("refs/grok/subagents/assigned-1".into());
        update_expected(&parent, "assigned-1", &meta_a, &owner_a).unwrap();

        let route_b = route("account-b", "22222222-2222-2222-2222-222222222222", 8);
        let meta_b = meta("assigned-1", route_b.canonical().as_str());
        let owner_b = replace(
            &parent,
            "assigned-1",
            &meta_b,
            &AssignmentKey::new("key-b").unwrap(),
            &route_b,
        )
        .unwrap();
        assert!(!owner_a.matches(&owner_b));
        assert!(update_expected(&parent, "assigned-1", &meta_a, &owner_a).is_err());

        let Lookup::Assigned { meta, owner } = lookup(&parent, "assigned-1").unwrap() else {
            panic!("expected assigned lookup")
        };
        assert_eq!(
            meta.effective_model_id.as_deref(),
            Some(route_b.canonical().as_str())
        );
        assert!(owner.matches(&owner_b));
        assert!(owner.identity().matches_live(&route_b));
        assert!(!owner.identity().matches_live(&route_a));

        let mut coordinator = super::super::SubagentCoordinator::new();
        assert!(coordinator.insert_pending(super::super::PendingSubagent {
            subagent_id: "assigned-1".into(),
            subagent_type: "general-purpose".into(),
            description: "replacement".into(),
            persona: None,
            parent_prompt_id: None,
            parent_session_id: "parent".into(),
            owner: xai_grok_tools::implementations::grok_build::task::types::SubagentOwner::Task,
            started_at: std::time::Instant::now(),
            run_in_background: false,
            surface_completion: true,
            color: None,
            cancel_token: tokio_util::sync::CancellationToken::new(),
            assigned_meta_owner: Some(owner_b.clone()),
        }));
        let _ = coordinator.move_pending_to_failed_owned("assigned-1", "late A", Some(&owner_a));
        assert_eq!(coordinator.registry_snapshot(), (1, 0, 0));
        let _ = coordinator.move_pending_to_cancelled_owned("assigned-1", "late A", Some(&owner_a));
        assert_eq!(coordinator.registry_snapshot(), (1, 0, 0));
        assert!(!coordinator.remove_pending_owned("assigned-1", Some(&owner_a)));
        assert_eq!(coordinator.registry_snapshot(), (1, 0, 0));
        let mut replacement = meta_b.clone();
        replacement.status = "completed".into();
        replacement.snapshot_ref = Some("refs/grok/subagents/replacement".into());
        assert!(update_expected(&parent, "assigned-1", &replacement, &owner_b).is_ok());
        meta_a.status = "failed".into();
        meta_a.snapshot_ref = Some("refs/grok/subagents/stale".into());
        assert!(update_expected(&parent, "assigned-1", &meta_a, &owner_a).is_err());
        let Lookup::Assigned { meta, owner } = lookup(&parent, "assigned-1").unwrap() else {
            panic!("expected replacement after stale lifecycle operations")
        };
        assert!(owner.matches(&owner_b));
        assert_eq!(meta.status, "completed");
        assert_eq!(
            meta.snapshot_ref.as_deref(),
            Some("refs/grok/subagents/replacement")
        );
    }

    #[test]
    fn assigned_resume_identity_rejects_provider_binding_and_incarnation_drift() {
        let assigned = route("account-a", "11111111-1111-1111-1111-111111111111", 4);
        let identity = AssignmentIdentity::from_route(
            &AssignmentKey::new("key-a").unwrap(),
            "child-assigned",
            &assigned,
        );
        assert!(identity.matches_live(&assigned));
        assert!(!identity.matches_live(&route(
            "account-b",
            "11111111-1111-1111-1111-111111111111",
            4,
        )));
        assert!(!identity.matches_live(&route(
            "account-a",
            "11111111-1111-1111-1111-111111111111",
            5,
        )));
        assert!(!identity.matches_live(&route(
            "account-a",
            "22222222-2222-2222-2222-222222222222",
            4,
        )));
    }

    #[test]
    fn legacy_standalone_meta_remains_compatible_and_partial_or_mismatch_fails() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("parent");
        let target = parent.join("subagents/legacy-1");
        std::fs::create_dir_all(&target).unwrap();
        let legacy = meta("legacy-1", "account-a:gpt-4o");
        std::fs::write(target.join("meta.json"), serialize_meta(&legacy).unwrap()).unwrap();
        assert!(matches!(
            lookup(&parent, "legacy-1").unwrap(),
            Lookup::LegacyUnassigned { .. }
        ));
        std::fs::write(target.join("assignment_identity.json"), b"{}").unwrap();
        assert!(lookup(&parent, "legacy-1").is_err());

        let target = parent.join("subagents/mismatch-1");
        std::fs::create_dir(&target).unwrap();
        let route = route("account-a", "11111111-1111-1111-1111-111111111111", 4);
        let mut assigned = meta("mismatch-1", route.canonical().as_str());
        commit_initial(
            &parent,
            "mismatch-1",
            &assigned,
            &AssignmentKey::new("key-mismatch").unwrap(),
            &route,
        )
        .unwrap();
        assigned.child_session_id = "tampered-child".into();
        let primary = serialize_meta(&assigned).unwrap();
        crate::session::storage::model_route::replace_private_identity_pair_for_subagent(
            &parent,
            "mismatch-1",
            &primary,
            &serialize_identity(&AssignmentIdentity::from_route(
                &AssignmentKey::new("key-mismatch").unwrap(),
                "child-mismatch-1",
                &route,
            ))
            .unwrap(),
        )
        .unwrap();
        assert!(lookup(&parent, "mismatch-1").is_err());
    }
}
