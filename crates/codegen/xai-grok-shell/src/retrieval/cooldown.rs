//! Short per-route cooldown after repeated retryable failures.
//!
//! Keys include profile/stage/provider instance/incarnation/model/protocol/
//! embedding-space identity as applicable — never credential secrets.
//! Auth/config/permanent failures do not create broad host/kind cooldowns.
//! Old snapshot generations cannot poison a replaced route.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use super::clock::Clock;
use super::error::{RetrievalStage, RouteFailureClass};
use super::graph::EmbeddingSpaceId;

/// Default failures before a route enters cooldown.
pub const DEFAULT_FAILURE_THRESHOLD: u32 = 2;
/// Default cooldown duration.
pub const DEFAULT_COOLDOWN: Duration = Duration::from_secs(30);
/// Absolute maximum cooldown.
pub const MAX_COOLDOWN: Duration = Duration::from_secs(300);

/// Secret-free cooldown key (exact route pins only).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CooldownKey {
    pub snapshot_generation: u64,
    pub profile_id: String,
    pub stage: RetrievalStage,
    pub route_model_id: String,
    pub provider_instance_id: String,
    pub incarnation: String,
    pub model: String,
    pub protocol: String,
    pub embedding_space_fp: Option<String>,
}

impl CooldownKey {
    pub fn for_embedding(
        snapshot_generation: u64,
        profile_id: &str,
        route_model_id: &str,
        provider_instance_id: &str,
        incarnation: Option<&str>,
        model: &str,
        protocol: &str,
        space: Option<&EmbeddingSpaceId>,
    ) -> Self {
        Self {
            snapshot_generation,
            profile_id: profile_id.to_owned(),
            stage: RetrievalStage::Embed,
            route_model_id: route_model_id.to_owned(),
            provider_instance_id: provider_instance_id.to_owned(),
            incarnation: incarnation.unwrap_or("").to_owned(),
            model: model.to_owned(),
            protocol: protocol.to_owned(),
            embedding_space_fp: space.map(|s| s.fingerprint().to_owned()),
        }
    }

    pub fn for_rerank(
        snapshot_generation: u64,
        profile_id: &str,
        route_model_id: &str,
        provider_instance_id: &str,
        incarnation: Option<&str>,
        model: &str,
        protocol: &str,
    ) -> Self {
        Self {
            snapshot_generation,
            profile_id: profile_id.to_owned(),
            stage: RetrievalStage::Rerank,
            route_model_id: route_model_id.to_owned(),
            provider_instance_id: provider_instance_id.to_owned(),
            incarnation: incarnation.unwrap_or("").to_owned(),
            model: model.to_owned(),
            protocol: protocol.to_owned(),
            embedding_space_fp: None,
        }
    }
}

#[derive(Debug, Clone)]
struct CooldownEntry {
    failure_count: u32,
    cool_until: Option<Instant>,
}

/// Process-local cooldown table (mutex; short critical sections).
pub struct CooldownTable {
    inner: Mutex<HashMap<CooldownKey, CooldownEntry>>,
    clock: Arc<dyn Clock>,
    failure_threshold: u32,
    cooldown: Duration,
}

impl CooldownTable {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            clock,
            failure_threshold: DEFAULT_FAILURE_THRESHOLD,
            cooldown: DEFAULT_COOLDOWN.min(MAX_COOLDOWN),
        }
    }

    pub fn with_params(clock: Arc<dyn Clock>, failure_threshold: u32, cooldown: Duration) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            clock,
            failure_threshold: failure_threshold.max(1),
            cooldown: cooldown.min(MAX_COOLDOWN),
        }
    }

    pub fn is_cooling(&self, key: &CooldownKey) -> bool {
        let now = self.clock.now();
        let mut guard = self.inner.lock();
        let Some(entry) = guard.get_mut(key) else {
            return false;
        };
        if let Some(until) = entry.cool_until {
            if now < until {
                return true;
            }
            // Expired: clear.
            entry.cool_until = None;
            entry.failure_count = 0;
        }
        false
    }

    /// Record a route failure. Only retryable classes increment cooldown.
    pub fn record_failure(&self, key: CooldownKey, class: RouteFailureClass) {
        if !class.is_cooldown_eligible() {
            return;
        }
        let now = self.clock.now();
        let mut guard = self.inner.lock();
        let entry = guard.entry(key).or_insert(CooldownEntry {
            failure_count: 0,
            cool_until: None,
        });
        entry.failure_count = entry.failure_count.saturating_add(1);
        if entry.failure_count >= self.failure_threshold {
            entry.cool_until = Some(now + self.cooldown);
            entry.failure_count = 0;
        }
    }

    /// Clear failure streak on success.
    pub fn record_success(&self, key: &CooldownKey) {
        let mut guard = self.inner.lock();
        guard.remove(key);
    }

    /// Drop all keys for generations older than `keep_generation` (bound growth).
    pub fn prune_before_generation(&self, keep_generation: u64) {
        let mut guard = self.inner.lock();
        guard.retain(|k, _| k.snapshot_generation >= keep_generation);
    }

    /// Test helper: number of live keys.
    #[cfg(test)]
    pub fn len_for_test(&self) -> usize {
        self.inner.lock().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::clock::MockClock;

    fn key(generation: u64, provider: &str) -> CooldownKey {
        CooldownKey::for_embedding(
            generation,
            "prof",
            "emb-a",
            provider,
            Some("inc"),
            "m",
            "openai_compatible",
            None,
        )
    }

    #[test]
    fn cooldown_after_threshold_and_generation_isolation() {
        let clock = Arc::new(MockClock::new());
        let table = CooldownTable::with_params(clock.clone(), 2, Duration::from_secs(10));
        let k = key(1, "acct-a");
        table.record_failure(k.clone(), RouteFailureClass::Timeout);
        assert!(!table.is_cooling(&k));
        table.record_failure(k.clone(), RouteFailureClass::Timeout);
        assert!(table.is_cooling(&k));
        // Sibling exact key is unaffected.
        let sibling = key(1, "acct-b");
        assert!(!table.is_cooling(&sibling));
        // Auth does not create cooldown.
        table.record_failure(sibling.clone(), RouteFailureClass::Auth);
        table.record_failure(sibling.clone(), RouteFailureClass::Auth);
        assert!(!table.is_cooling(&sibling));
        // Old generation key does not affect new generation.
        let new_gen = key(2, "acct-a");
        assert!(!table.is_cooling(&new_gen));
        // Advance past cooldown.
        clock.advance(Duration::from_secs(11));
        assert!(!table.is_cooling(&k));
    }
}
