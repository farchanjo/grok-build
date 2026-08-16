//! Bounded private assignment store for exact child routes.

use std::collections::BTreeMap;

use super::exact_route::ExactRoute;

pub(super) const MAX_ASSIGNMENT_ENTRIES: usize = 1_024;
pub(super) const MAX_ASSIGNMENT_BYTES: usize = 4 * 1024 * 1024;
const _: [(); MAX_ASSIGNMENT_ENTRIES] = [(); xai_workflow::MAX_AGENT_BUDGET as usize];
const _: [(); xai_workflow::MAX_AGENT_BUDGET as usize] = [(); MAX_ASSIGNMENT_ENTRIES];
const _: [(); 1_024] = [(); MAX_ASSIGNMENT_ENTRIES];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct AssignmentKey(String);

impl AssignmentKey {
    pub(super) fn workflow(run_id: &str, sequence: u64) -> Option<Self> {
        // The sequence belongs to the contextual workflow envelope, never a
        // process-global receive order. Reject invalid contextual ids instead
        // of constructing an unbounded trusted key.
        Self::new(format!("wf-{run_id}-seq-{sequence}"))
    }

    pub(super) fn goal(goal_id: &str, role: &str, skeptic_idx: Option<u32>) -> Option<Self> {
        let suffix = match skeptic_idx {
            Some(idx) => format!("skeptic-{idx}"),
            None => role.to_owned(),
        };
        Self::new(format!("goal-{goal_id}-{suffix}"))
    }

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
pub(crate) enum AssignmentError {
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
    #[must_use]
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
        let bytes = assignment_bytes(&key, &route).ok_or(AssignmentError::TooLarge)?;
        let total = self
            .bytes
            .checked_add(bytes)
            .ok_or(AssignmentError::TooLarge)?;
        if total > MAX_ASSIGNMENT_BYTES {
            return Err(AssignmentError::TooLarge);
        }
        self.bytes = total;
        self.routes.insert(key, route);
        Ok(())
    }

    pub(super) fn take(&mut self, key: &AssignmentKey) -> Option<ExactRoute> {
        let route = self.routes.remove(key)?;
        let bytes = assignment_bytes(key, &route)
            .expect("accepted assignment route size must remain representable");
        self.bytes = self
            .bytes
            .checked_sub(bytes)
            .expect("assignment byte accounting must match stored routes");
        Some(route)
    }

    #[cfg(test)]
    pub(super) fn take_without_accounting_for_test(
        &mut self,
        key: &AssignmentKey,
    ) -> Option<ExactRoute> {
        self.routes.remove(key)
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.routes.len()
    }

    #[cfg(test)]
    pub(super) fn bytes(&self) -> usize {
        self.bytes
    }

    #[cfg(test)]
    pub(super) fn set_bytes_for_test(&mut self, bytes: usize) -> usize {
        std::mem::replace(&mut self.bytes, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assignment_limits_match_workflow_budget_and_four_mib() {
        assert_eq!(MAX_ASSIGNMENT_ENTRIES, 1_024);
        assert_eq!(xai_workflow::MAX_AGENT_BUDGET, 1_024);
        assert_eq!(MAX_ASSIGNMENT_BYTES, 4 * 1024 * 1024);
    }

    #[test]
    fn workflow_keys_are_stable_and_goal_roles_are_distinct() {
        assert_eq!(
            AssignmentKey::workflow("run", 7).unwrap().as_str(),
            "wf-run-seq-7"
        );
        assert_eq!(
            AssignmentKey::goal("goal", "planner", None)
                .unwrap()
                .as_str(),
            "goal-goal-planner"
        );
        assert_eq!(
            AssignmentKey::goal("goal", "skeptic", Some(2))
                .unwrap()
                .as_str(),
            "goal-goal-skeptic-2"
        );
        assert!(AssignmentKey::workflow(&"x".repeat(512), 0).is_none());
        assert!(AssignmentKey::new("line\nbreak").is_none());
        assert!(AssignmentKey::new(" ").is_some());
        assert!(AssignmentKey::new("x".repeat(513)).is_none());
    }
}

fn assignment_bytes(key: &AssignmentKey, route: &ExactRoute) -> Option<usize> {
    [
        key.as_str().len(),
        route.canonical().as_str().len(),
        route.upstream().as_str().len(),
        route.context().instance_id().len(),
        route.context().incarnation().map_or(0, str::len),
        route.context().origin().map_or(0, str::len),
        route.context().model_partition().map_or(0, str::len),
        128,
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)
}
