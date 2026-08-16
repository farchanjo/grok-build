//! PR5's secret-free durable exact-route assignment record.
//!
//! The pair commit itself is intentionally delegated to the tested PR4
//! dirfd-rooted transaction primitive. This module defines only PR5 semantics:
//! stable assignment identity plus the exact non-secret route provenance.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{assignment::AssignmentKey, exact_route::ExactRoute};

const MAX_META_BYTES: usize = 1024 * 1024;
const MAX_BYTES: usize = 16 * 1024;
const VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    pub(crate) fn from_route(
        key: &AssignmentKey,
        child_session_id: &str,
        route: &ExactRoute,
    ) -> Self {
        let route_context = route.context();
        Self {
            version: VERSION,
            assignment_key: key.as_str().to_owned(),
            child_session_id: child_session_id.to_owned(),
            canonical_model: route.canonical().as_str().to_owned(),
            upstream_model: route.upstream().as_str().to_owned(),
            provider_instance_id: route_context.instance_id().to_owned(),
            provider_incarnation: route_context.incarnation().map(str::to_owned),
            provider_kind: route_context.provider_kind().as_str().to_owned(),
            api_surface: route_context.api_surface().as_str().to_owned(),
            credential_route: route_context.credential_route().as_str().to_owned(),
            registry_generation: route_context.registry_generation(),
            binding_generation: route_context.binding_generation(),
            model_partition: route_context.model_partition().map(str::to_owned),
        }
    }
}

pub(crate) enum Lookup {
    Missing,
    LegacyPrimary,
    Valid(AssignmentIdentity),
}

pub(crate) fn commit(
    target_dir: &Path,
    primary_meta: &[u8],
    key: &AssignmentKey,
    child_session_id: &str,
    route: &ExactRoute,
) -> io::Result<String> {
    if primary_meta.len() > MAX_META_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "subagent metadata exceeds size limit",
        ));
    }
    let companion = serde_json::to_vec(&AssignmentIdentity::from_route(
        key,
        child_session_id,
        route,
    ))
    .map_err(io::Error::other)?;
    if companion.len() > MAX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "assignment identity exceeds size limit",
        ));
    }
    crate::session::storage::model_route::commit_private_identity_pair(
        target_dir,
        primary_meta,
        &companion,
    )
}

pub(crate) fn lookup(target_dir: &Path) -> io::Result<Lookup> {
    use crate::session::storage::model_route::PrivateIdentityPair;
    match crate::session::storage::model_route::load_private_identity_pair(
        target_dir,
        MAX_META_BYTES,
        MAX_BYTES,
    )? {
        PrivateIdentityPair::Missing => Ok(Lookup::Missing),
        PrivateIdentityPair::LegacyPrimary(_) => Ok(Lookup::LegacyPrimary),
        PrivateIdentityPair::ValidPair { companion, .. } => serde_json::from_slice(&companion)
            .map(Lookup::Valid)
            .map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "malformed assignment identity")
            }),
    }
}
