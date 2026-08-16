//! Bounded private assignment store for exact child routes.

use std::collections::BTreeMap;

use super::exact_route::ExactRoute;

pub(super) const MAX_ASSIGNMENT_ENTRIES: usize = xai_workflow::MAX_AGENT_BUDGET as usize;
pub(super) const MAX_ASSIGNMENT_BYTES: usize = 4 * 1024 * 1024;
const _: [(); MAX_ASSIGNMENT_ENTRIES] = [(); xai_workflow::MAX_AGENT_BUDGET as usize];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct AssignmentKey(String);

impl AssignmentKey {
    pub(super) fn new(raw: impl Into<String>) -> Option<Self> {
        let raw = raw.into();
        (!raw.is_empty()
            && raw.len() <= 512
            && raw.bytes().all(|b| b.is_ascii_graphic() || b == b' '))
        .then_some(Self(raw))
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(super) enum AssignmentError {
    #[error("invalid assignment key")]
    InvalidKey,
    #[error("assignment capacity exhausted")]
    Full,
    #[error("assignment byte cap exceeded")]
    TooLarge,
    #[error("assignment key already exists")]
    Duplicate,
}

#[derive(Default)]
pub(super) struct Assignments {
    routes: BTreeMap<AssignmentKey, ExactRoute>,
    bytes: usize,
}

impl Assignments {
    pub(super) fn insert(
        &mut self,
        key: AssignmentKey,
        route: ExactRoute,
    ) -> Result<(), AssignmentError> {
        if self.routes.contains_key(&key) {
            return Err(AssignmentError::Duplicate);
        }
        if self.routes.len() >= MAX_ASSIGNMENT_ENTRIES {
            return Err(AssignmentError::Full);
        }
        let bytes = assignment_bytes(&key, &route);
        if self.bytes.saturating_add(bytes) > MAX_ASSIGNMENT_BYTES {
            return Err(AssignmentError::TooLarge);
        }
        self.bytes += bytes;
        self.routes.insert(key, route);
        Ok(())
    }

    pub(super) fn take(&mut self, key: &AssignmentKey) -> Option<ExactRoute> {
        let route = self.routes.remove(key)?;
        self.bytes = self.bytes.saturating_sub(assignment_bytes(key, &route));
        Some(route)
    }
}

fn assignment_bytes(key: &AssignmentKey, route: &ExactRoute) -> usize {
    key.as_str().len()
        + route.canonical().as_str().len()
        + route.upstream().as_str().len()
        + route.context().instance_id().len()
        + route.context().model_partition().map_or(0, str::len)
        + 128
}
