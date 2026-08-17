//! Immutable versioned [`RetrievalRegistry`] with atomic last-known-good publish.
//!
//! Readers take a short lock (or ArcSwap load) and clone an `Arc` snapshot.
//! Writers build candidates completely off-lock and publish only when the
//! expected base generation still matches. No I/O under the write lock.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use arc_swap::ArcSwap;
use parking_lot::Mutex;

use super::clock::{Clock, SystemClock};
use super::cooldown::CooldownTable;
use super::graph::RetrievalSnapshot;
use super::reload::{
    ReloadOutcome, SnapshotBuildInput, build_snapshot, load_build_input_from_home,
};

/// Registry publish state (generation CAS counter).
struct PublishState {
    /// Next generation to assign on successful publish.
    next_generation: u64,
}

/// Shell-owned retrieval registry: one immutable Arc snapshot at a time.
pub struct RetrievalRegistry {
    snapshot: ArcSwap<RetrievalSnapshot>,
    publish: Mutex<PublishState>,
    /// Generation of the currently published snapshot (lock-free read).
    live_generation: AtomicU64,
    home: PathBuf,
    cooldown: Arc<CooldownTable>,
    clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for RetrievalRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let snap = self.load();
        f.debug_struct("RetrievalRegistry")
            .field("generation", &snap.generation)
            .field("enabled", &snap.enabled)
            .field("fingerprint", &snap.fingerprint)
            .field("home", &self.home)
            .finish()
    }
}

impl RetrievalRegistry {
    /// Create a disabled registry (no graph).
    pub fn disabled(home: impl Into<PathBuf>) -> Arc<Self> {
        Self::disabled_with_clock(home, Arc::new(SystemClock))
    }

    pub fn disabled_with_clock(home: impl Into<PathBuf>, clock: Arc<dyn Clock>) -> Arc<Self> {
        let home = home.into();
        let snap = RetrievalSnapshot::disabled(0);
        Arc::new(Self {
            snapshot: ArcSwap::from(snap),
            publish: Mutex::new(PublishState { next_generation: 1 }),
            live_generation: AtomicU64::new(0),
            home,
            cooldown: Arc::new(CooldownTable::new(clock.clone())),
            clock,
        })
    }

    /// Load from home (or disabled on empty/invalid). Never panics.
    pub fn load_from_home(home: impl Into<PathBuf>) -> Arc<Self> {
        let home = home.into();
        let reg = Self::disabled(home.clone());
        let _ = reg.reload_from_home();
        reg
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn clock(&self) -> Arc<dyn Clock> {
        self.clock.clone()
    }

    pub fn cooldown(&self) -> Arc<CooldownTable> {
        self.cooldown.clone()
    }

    /// Short-lock free load of the current immutable snapshot.
    pub fn load(&self) -> Arc<RetrievalSnapshot> {
        self.snapshot.load_full()
    }

    /// Current generation (lock-free).
    pub fn generation(&self) -> u64 {
        self.live_generation.load(Ordering::Acquire)
    }

    /// Clone service handle over this registry.
    pub fn service(self: &Arc<Self>) -> super::service::RetrievalService {
        super::service::RetrievalService::new(self.clone())
    }

    /// Publish a pre-built candidate if `expected_generation` matches live.
    ///
    /// Candidate must already carry the generation it should publish as.
    /// No I/O is performed here.
    pub fn try_publish(
        &self,
        expected_generation: u64,
        candidate: Arc<RetrievalSnapshot>,
    ) -> ReloadOutcome {
        let mut guard = self.publish.lock();
        let live = self.live_generation.load(Ordering::Acquire);
        if live != expected_generation {
            return ReloadOutcome::StaleDropped {
                expected_generation,
                live_generation: live,
            };
        }
        let current = self.snapshot.load_full();
        if current.fingerprint == candidate.fingerprint && current.enabled == candidate.enabled {
            return ReloadOutcome::Unchanged {
                generation: live,
                fingerprint: current.fingerprint.clone(),
            };
        }
        // Assign generation from CAS state (candidate.generation should match).
        let published_generation = candidate.generation;
        if published_generation < guard.next_generation && published_generation != 0 {
            // Allow explicit generation from builder when it equals next-1 after load.
        }
        guard.next_generation = published_generation
            .saturating_add(1)
            .max(guard.next_generation);
        self.snapshot.store(candidate.clone());
        self.live_generation
            .store(published_generation, Ordering::Release);
        self.cooldown
            .prune_before_generation(published_generation.saturating_sub(2));
        if !candidate.enabled {
            return ReloadOutcome::Disabled {
                generation: published_generation,
            };
        }
        ReloadOutcome::Published {
            generation: published_generation,
            fingerprint: candidate.fingerprint.clone(),
            warnings: candidate.warnings.clone(),
        }
    }

    /// Build from `input` off-lock, then CAS-publish.
    pub fn publish_build_input(
        &self,
        expected_generation: u64,
        input: SnapshotBuildInput,
    ) -> ReloadOutcome {
        let next_gen = {
            let guard = self.publish.lock();
            let live = self.live_generation.load(Ordering::Acquire);
            if live != expected_generation {
                return ReloadOutcome::StaleDropped {
                    expected_generation,
                    live_generation: live,
                };
            }
            // next generation id for the candidate
            guard.next_generation.max(live.saturating_add(1))
        };
        match build_snapshot(input, next_gen) {
            Ok(candidate) => self.try_publish(expected_generation, candidate),
            Err(err) => {
                let live = self.generation();
                ReloadOutcome::RetainedLastKnownGood {
                    generation: live,
                    reasons: err.reasons,
                }
            }
        }
    }

    /// Force-publish a snapshot (tests), bumping generation unconditionally.
    pub fn force_publish(&self, mut candidate: Arc<RetrievalSnapshot>) -> u64 {
        let mut guard = self.publish.lock();
        let published_generation = guard.next_generation;
        guard.next_generation = published_generation.saturating_add(1);
        // Rebuild Arc with correct generation if needed.
        if candidate.generation != published_generation {
            let mut owned = (*candidate).clone();
            owned.generation = published_generation;
            candidate = Arc::new(owned);
        }
        self.snapshot.store(candidate.clone());
        self.live_generation
            .store(published_generation, Ordering::Release);
        published_generation
    }

    /// Reload from `$GROK_HOME` (or registry home). Retains LKG on failure.
    pub fn reload_from_home(&self) -> ReloadOutcome {
        let expected = self.generation();
        match load_build_input_from_home(&self.home) {
            Ok(input) => {
                // Empty graph → disabled snapshot (still a successful publish).
                if input.graph.retrieval_profiles.is_empty()
                    && input.graph.embedding_models.is_empty()
                    && input.graph.reranker_models.is_empty()
                {
                    let next_gen = {
                        let guard = self.publish.lock();
                        let live = self.live_generation.load(Ordering::Acquire);
                        if live != expected {
                            return ReloadOutcome::StaleDropped {
                                expected_generation: expected,
                                live_generation: live,
                            };
                        }
                        guard.next_generation.max(live.saturating_add(1))
                    };
                    let disabled = RetrievalSnapshot::disabled(next_gen);
                    return self.try_publish(expected, disabled);
                }
                self.publish_build_input(expected, input)
            }
            Err(e) => ReloadOutcome::RetainedLastKnownGood {
                generation: expected,
                reasons: vec![e],
            },
        }
    }

    /// Rebuild from an in-memory build input (composition / tests).
    pub fn reload_from_input(&self, input: SnapshotBuildInput) -> ReloadOutcome {
        let expected = self.generation();
        self.publish_build_input(expected, input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::reload::{test_graph_two_embed_routes, test_provider_views_capable};

    fn sample_input() -> SnapshotBuildInput {
        let (views, meta) = test_provider_views_capable(&["acct-a", "acct-b"]);
        SnapshotBuildInput {
            graph: test_graph_two_embed_routes(),
            graph_generation: 1,
            provider_generation: 1,
            provider_views: views,
            provider_meta: meta,
            parse_warnings: Vec::new(),
        }
    }

    #[test]
    fn publish_load_and_stale_drop() {
        let reg = RetrievalRegistry::disabled("/tmp/retrieval-reg-test");
        assert!(!reg.load().enabled);
        let out = reg.publish_build_input(0, sample_input());
        assert!(matches!(out, ReloadOutcome::Published { .. }), "{out:?}");
        let snap = reg.load();
        assert!(snap.enabled);
        assert_eq!(snap.profiles.len(), 1);
        let live_generation = reg.generation();

        // Stale concurrent publish drops.
        let out2 = reg.publish_build_input(0, sample_input());
        assert!(
            matches!(
                out2,
                ReloadOutcome::StaleDropped {
                    expected_generation: 0,
                    ..
                }
            ),
            "{out2:?}"
        );
        assert_eq!(reg.generation(), live_generation);
        // Same Arc generation retained.
        assert_eq!(reg.load().generation, snap.generation);
    }

    #[test]
    fn invalid_reload_keeps_lkg() {
        let reg = RetrievalRegistry::disabled("/tmp/retrieval-reg-test-2");
        let _ = reg.publish_build_input(0, sample_input());
        let good = reg.load();
        let mut bad = sample_input();
        bad.graph
            .retrieval_profiles
            .get_mut("default")
            .unwrap()
            .embedding_models = vec!["missing-route".into()];
        let out = reg.publish_build_input(reg.generation(), bad);
        assert!(
            matches!(out, ReloadOutcome::RetainedLastKnownGood { .. }),
            "{out:?}"
        );
        let after = reg.load();
        assert!(Arc::ptr_eq(&good, &after) || after.generation == good.generation);
        assert_eq!(after.fingerprint, good.fingerprint);
    }
}
