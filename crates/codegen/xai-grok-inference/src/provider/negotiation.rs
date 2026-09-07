//! Backend negotiation — a concrete collaborator, not a port.
//!
//! This is **Phase 6a** of the Provider Adapter Factory refactor. Backend
//! negotiation is the one piece of the adapter seam that is *not* a
//! behavior-preserving refactor: when enabled it can change which backend a
//! provider uses after a first-attempt failure. It is therefore gated behind
//! an off-by-default flag (`provider.negotiate_backends = true`) so the
//! default runtime path is byte-identical to the pre-negotiation end state.
//!
//! ## Role
//!
//! [`BackendNegotiator`] is owned by the `InferenceClient` / actor (one per
//! route task) and is consulted *alongside* the existing 404 classification
//! in `retry.rs` — it is **never** a method on
//! [`ProviderAdapter`](super::ProviderAdapter). The adapter supplies
//! `policy().backend_preference`; the *client* filters that by its model
//! capability (this crate does not define model caps) into a
//! [`BackendPreference`] window and hands it to the negotiator.
//!
//! ## Invariants
//!
//! - **Single writer, wait-free reads.** The negotiated backend lives in a
//!   lock-free [`AtomicU8`] latch with `{UNRESOLVED, CHAT, RESPONSES,
//!   MESSAGES}` states. Only one `compare_exchange` wins a downgrade; every
//!   other contender observes the winner's result instead of overwriting it
//!   (no ABA, no lost-update class).
//! - **Read once per stream.** The caller must call [`Self::current`] once
//!   per stream, outside the chunk loop, and reuse the returned backend for
//!   the whole stream. It must never re-query the latch per chunk.
//! - **Generation-tagged.** The cached decision is bound to a
//!   `binding_generation` (from `route_context.rs`). A model re-bind or
//!   config reload bumps the generation, which invalidates the cached
//!   decision so the next request re-evaluates. The generation is adopted
//!   with an rcu-style monotone comparison — a stale thread can never
//!   clobber a newer binding's decision, and the latch reset only happens
//!   under a genuinely newer generation (never a bare store).
//! - **Explicit backend never downgrades.** When `api_backend` was
//!   explicitly configured, the negotiator never auto-downgrades and never
//!   writes the latch.

use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use xai_grok_inference_types::ApiBackend;

/// Lock-free latch states for the negotiated backend.
///
/// `UNRESOLVED` means no backend has been negotiated yet: [`Self::current`]
/// falls back to the initial backend supplied at construction. The remaining
/// states are the negotiated/fallback backend.
const UNRESOLVED: u8 = 0;
const CHAT: u8 = 1;
const RESPONSES: u8 = 2;
const MESSAGES: u8 = 3;

/// Maps a backend onto its latch state.
const fn backend_to_state(backend: ApiBackend) -> u8 {
    match backend {
        ApiBackend::ChatCompletions => CHAT,
        ApiBackend::Responses => RESPONSES,
        ApiBackend::Messages => MESSAGES,
    }
}

/// Maps a latch state back onto a backend. `UNRESOLVED` (and any unknown
/// state) maps to `None`.
const fn state_to_backend(state: u8) -> Option<ApiBackend> {
    match state {
        CHAT => Some(ApiBackend::ChatCompletions),
        RESPONSES => Some(ApiBackend::Responses),
        MESSAGES => Some(ApiBackend::Messages),
        _ => None,
    }
}

/// A stable wire label for a backend, matching its serde `snake_case`
/// spelling. Used in the downgrade diagnostic message.
const fn backend_label(backend: &ApiBackend) -> &'static str {
    match backend {
        ApiBackend::ChatCompletions => "chat_completions",
        ApiBackend::Responses => "responses",
        ApiBackend::Messages => "messages",
    }
}

/// A fixed, client-side-filtered window of supported backends.
///
/// The client owns model capability and filters `policy().backend_preference`
/// into this window with [`backend_preference_window`], so `None` marks a
/// backend that is not supported for the model at hand. The width is fixed
/// at 3 (matching the `1..=3` preference contract) so no heap allocation is
/// required and no new dependency is introduced.
pub type BackendPreference = [Option<ApiBackend>; 3];

/// The outcome of a one-shot backend downgrade, produced by
/// [`BackendNegotiator::record_failure`].
///
/// Carries both backends so the caller can surface an error naming **both**
/// the backend that was attempted and the one negotiated as the fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendDowngrade {
    /// The backend that was attempted and rejected (404 / unsupported).
    pub attempted: ApiBackend,
    /// The backend negotiated as the fallback for the next attempt.
    pub negotiated: ApiBackend,
}

impl BackendDowngrade {
    /// A diagnostic message naming **both** attempted backends.
    ///
    /// A bare 404 is far more often a wrong model id than an unsupported
    /// backend, so the message always names what was attempted and what the
    /// negotiation selected, never just the numeric status.
    pub fn message(&self) -> String {
        format!(
            "backend '{}' was rejected for this endpoint (attempted '{}', \
             negotiated fallback '{}')",
            backend_label(&self.attempted),
            backend_label(&self.attempted),
            backend_label(&self.negotiated),
        )
    }
}

/// A lock-free, single-writer backend negotiator.
///
/// Construct one per route task with the provider's filtered preference
/// window, then call [`Self::current`] once per stream (outside the chunk
/// loop) to select the backend, and [`Self::record_failure`] on a
/// first-attempt 404 / backend-unsupported error to downgrade once.
#[derive(Debug)]
pub struct BackendNegotiator {
    /// Gate: `provider.negotiate_backends`. When `false` (default) the
    /// negotiator is a no-op — `record_failure` never downgrades and never
    /// writes the latch.
    enabled: bool,
    /// Whether `api_backend` was explicitly configured. When `true` the
    /// negotiator never auto-downgrades and never writes the latch.
    api_backend_explicit: bool,
    /// The backend to serve until a downgrade resolves the latch. When
    /// `api_backend_explicit` this is the configured backend; otherwise it
    /// is the highest-priority supported entry in `preferred`.
    initial: ApiBackend,
    /// The client-side-filtered supported preference window, in priority
    /// order. `None` marks a backend unsupported for the model.
    preferred: BackendPreference,
    /// The negotiated backend latch. `UNRESOLVED` until a downgrade writes
    /// it; reads are wait-free.
    latch: AtomicU8,
    /// The `binding_generation` most recently adopted. The cached decision is
    /// valid only under this generation; a newer generation invalidates it.
    generation: AtomicU64,
}

impl BackendNegotiator {
    /// Build a negotiator.
    ///
    /// - `enabled` — the `provider.negotiate_backends` gate. `false` makes
    ///   the negotiator a no-op.
    /// - `api_backend_explicit` — whether `api_backend` was explicitly
    ///   configured. `true` disables auto-downgrade and latch writes.
    /// - `initial` — the backend to serve until a downgrade: the explicit
    ///   `api_backend` when configured, else the highest-priority supported
    ///   entry in `preferred`.
    /// - `preferred` — the client-side-filtered supported preference window.
    ///
    /// The caller must pass an `initial` that is also present in `preferred`
    /// when `api_backend_explicit` is `false`, so a downgrade can move off it.
    pub fn new(
        enabled: bool,
        api_backend_explicit: bool,
        initial: ApiBackend,
        preferred: BackendPreference,
    ) -> Self {
        Self {
            enabled,
            api_backend_explicit,
            initial,
            preferred,
            latch: AtomicU8::new(UNRESOLVED),
            generation: AtomicU64::new(0),
        }
    }

    /// Whether negotiation is enabled (the `provider.negotiate_backends`
    /// gate the negotiator was constructed with).
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// The backend to use for the current stream.
    ///
    /// **Read once per stream, outside the chunk loop.** The caller must
    /// call this a single time per stream and reuse the returned value for
    /// the whole stream; it must never re-query the latch per chunk.
    ///
    /// `generation` is the route's `binding_generation`. When it is newer
    /// than the generation the cached decision was made under, the decision
    /// is invalidated (the latch resets to `UNRESOLVED`) so the next request
    /// re-evaluates — rcu-style adopt-newer-generation, never a bare store.
    ///
    /// Returns `initial` while the latch is unresolved, or the negotiated
    /// fallback once a downgrade has been recorded.
    pub fn current(&self, generation: u64) -> ApiBackend {
        self.ensure_generation(generation);
        match state_to_backend(self.latch.load(Ordering::Acquire)) {
            Some(backend) => backend,
            None => self.initial.clone(),
        }
    }

    /// Record a first-attempt 404 / backend-unsupported failure for
    /// `attempted` and downgrade once along the supported preference.
    ///
    /// Returns `Some(BackendDowngrade)` only when negotiation is enabled,
    /// `api_backend` was **not** explicitly configured, the latch has not
    /// already been resolved by a prior downgrade, and `attempted` is the
    /// backend currently being served (`initial`). The downgrade selects the
    /// first supported backend in the preference window that differs from
    /// `attempted`; the latch is written with a single `compare_exchange`
    /// so exactly one concurrent request wins the negotiation. Once a
    /// downgrade has been recorded, a later failure is **not** re-negotiated
    /// ("downgrade once").
    ///
    /// Returns `None` for every other case — crucially, this includes the
    /// default `enabled = false` path, which makes the negotiator a no-op
    /// and leaves the latch untouched.
    pub fn record_failure(
        &self,
        generation: u64,
        attempted: ApiBackend,
    ) -> Option<BackendDowngrade> {
        if !self.enabled {
            return None;
        }
        if self.api_backend_explicit {
            // Explicitly configured backend never auto-downgrades and never
            // writes the cache.
            return None;
        }
        self.ensure_generation(generation);

        // Only the still-unresolved initial backend may initiate a downgrade.
        // A prior downgrade resolves the latch to a fallback, so a later
        // failure is never re-negotiated.
        if self.latch.load(Ordering::Acquire) != UNRESOLVED {
            return None;
        }
        if attempted != self.initial {
            return None;
        }

        // The next supported backend in priority order that differs from the
        // attempted one. `None` entries (unsupported caps) are skipped.
        let next = self
            .preferred
            .iter()
            .flatten()
            .find(|backend| **backend != attempted)
            .cloned()?;

        // Single writer: only the winning compare_exchange reports the
        // downgrade; every racing loser observes the winner's result.
        match self.latch.compare_exchange(
            UNRESOLVED,
            backend_to_state(next.clone()),
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => Some(BackendDowngrade {
                attempted,
                negotiated: next,
            }),
            Err(_) => None,
        }
    }

    /// Adopt `generation` if it is newer than the one currently held.
    ///
    /// rcu-style: only a genuinely newer generation resets the latch to
    /// `UNRESOLVED`. A stale thread (same or older generation) is a no-op
    /// and never clobbers a newer binding's decision. The generation is
    /// written with a monotone `compare_exchange`, never a bare store.
    fn ensure_generation(&self, generation: u64) {
        let current = self.generation.load(Ordering::Acquire);
        if generation <= current {
            return;
        }
        match self.generation.compare_exchange(
            current,
            generation,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                // We won the generation bump: invalidate the cached decision.
                self.latch.store(UNRESOLVED, Ordering::Release);
            }
            Err(latest) => {
                // Lost the bump. If a peer already adopted a newer
                // generation, leave it; if `latest` is still older than
                // `generation`, retry the monotonically-forward write so a
                // conflicting pair always settles on the newest generation.
                if latest < generation {
                    self.ensure_generation(generation);
                }
            }
        }
    }
}

/// Build a client-side-filtered [`BackendPreference`] window from an
/// adapter's `policy().backend_preference` and a model-capability predicate.
///
/// The client owns model capability, so the `supports` closure decides which
/// backends are available. Order is preserved (highest-priority first) and
/// unsupported backends are dropped, up to the three-entry window.
pub fn backend_preference_window(
    unfiltered: &'static [ApiBackend],
    supports: impl Fn(&ApiBackend) -> bool,
) -> BackendPreference {
    let mut window: BackendPreference = [None, None, None];
    let mut idx = 0;
    for backend in unfiltered.iter() {
        if idx >= 3 {
            break;
        }
        if supports(backend) {
            window[idx] = Some(backend.clone());
            idx += 1;
        }
    }
    window
}

#[cfg(test)]
mod tests {
    use super::*;

    // A representative preference: Responses preferred, Chat Completions as
    // a fallback, Messages unsupported.
    const RESP_CHAT: BackendPreference = [
        Some(ApiBackend::Responses),
        Some(ApiBackend::ChatCompletions),
        None,
    ];

    #[test]
    fn downgrade_once_on_first_failure() {
        let negotiator = BackendNegotiator::new(true, false, ApiBackend::Responses, RESP_CHAT);

        // The stream uses the initial backend until a failure.
        assert_eq!(negotiator.current(1), ApiBackend::Responses);

        // First 404 → one downgrade, cached.
        let downgrade = negotiator.record_failure(1, ApiBackend::Responses);
        assert_eq!(
            downgrade,
            Some(BackendDowngrade {
                attempted: ApiBackend::Responses,
                negotiated: ApiBackend::ChatCompletions,
            })
        );

        // The downgrade is now cached for subsequent streams.
        assert_eq!(negotiator.current(1), ApiBackend::ChatCompletions);

        // A second failure is never re-negotiated ("downgrade once").
        assert_eq!(negotiator.record_failure(1, ApiBackend::Responses), None);
        assert_eq!(
            negotiator.record_failure(1, ApiBackend::ChatCompletions),
            None
        );
        assert_eq!(negotiator.current(1), ApiBackend::ChatCompletions);
    }

    #[test]
    fn no_downgrade_when_backend_explicit() {
        let negotiator = BackendNegotiator::new(true, true, ApiBackend::ChatCompletions, RESP_CHAT);

        // Explicit backend never auto-downgrades and never writes the latch.
        assert_eq!(
            negotiator.record_failure(1, ApiBackend::ChatCompletions),
            None
        );
        assert_eq!(negotiator.current(1), ApiBackend::ChatCompletions);

        // Even with negotiation enabled, the exposed value stays fixed.
        let d = negotiator.record_failure(1, ApiBackend::ChatCompletions);
        assert!(d.is_none());
        assert_eq!(negotiator.current(1), ApiBackend::ChatCompletions);
    }

    #[test]
    fn generation_bump_invalidates_cached_decision() {
        let negotiator = BackendNegotiator::new(true, false, ApiBackend::Responses, RESP_CHAT);

        // Resolve to the fallback under generation 1.
        assert_eq!(negotiator.current(1), ApiBackend::Responses);
        assert!(
            negotiator
                .record_failure(1, ApiBackend::Responses)
                .is_some()
        );
        assert_eq!(negotiator.current(1), ApiBackend::ChatCompletions);

        // A model re-bind bumps the generation: the cached decision is
        // invalidated and the next request re-evaluates to the new initial.
        assert_eq!(negotiator.current(2), ApiBackend::Responses);

        // The new generation may negotiate again.
        let downgrade = negotiator.record_failure(2, ApiBackend::Responses);
        assert_eq!(
            downgrade.map(|d| d.negotiated),
            Some(ApiBackend::ChatCompletions)
        );

        // An older generation cannot clobber the newer one's decision.
        assert_eq!(negotiator.current(1), ApiBackend::ChatCompletions);
    }

    #[test]
    fn flag_off_is_no_op() {
        let negotiator = BackendNegotiator::new(false, false, ApiBackend::Responses, RESP_CHAT);

        // Enabled=false is the default: record_failure never downgrades and
        // never writes the latch, so the exposed backend never changes.
        assert_eq!(negotiator.record_failure(1, ApiBackend::Responses), None);
        assert_eq!(negotiator.current(1), ApiBackend::Responses);
        assert_eq!(negotiator.current(2), ApiBackend::Responses);

        let d = negotiator.record_failure(1, ApiBackend::Responses);
        assert!(d.is_none());
        assert_eq!(negotiator.current(1), ApiBackend::Responses);
    }

    #[test]
    fn error_message_names_both_backends() {
        let downgrade = BackendDowngrade {
            attempted: ApiBackend::Responses,
            negotiated: ApiBackend::ChatCompletions,
        };
        let message = downgrade.message();
        assert!(
            message.contains("responses"),
            "message must name the attempted backend: {message}"
        );
        assert!(
            message.contains("chat_completions"),
            "message must name the negotiated backend: {message}"
        );
        assert_eq!(
            downgrade,
            BackendDowngrade {
                attempted: ApiBackend::Responses,
                negotiated: ApiBackend::ChatCompletions,
            }
        );
    }

    #[test]
    fn no_fallback_when_only_supported_backend_is_current() {
        let negotiator = BackendNegotiator::new(
            true,
            false,
            ApiBackend::ChatCompletions,
            [Some(ApiBackend::ChatCompletions), None, None],
        );
        // No alternate supported backend exists, so there is nothing to
        // downgrade to and no latch write.
        assert_eq!(
            negotiator.record_failure(1, ApiBackend::ChatCompletions),
            None
        );
        assert_eq!(negotiator.current(1), ApiBackend::ChatCompletions);
    }

    #[test]
    fn unsupported_caps_are_skipped_in_the_window() {
        // Messages in the middle is unsupported for the model; the window
        // must drop it and keep priority order.
        let window = backend_preference_window(
            &[
                ApiBackend::Responses,
                ApiBackend::Messages,
                ApiBackend::ChatCompletions,
            ],
            |backend| *backend != ApiBackend::Messages,
        );
        assert_eq!(
            window,
            [
                Some(ApiBackend::Responses),
                Some(ApiBackend::ChatCompletions),
                None,
            ]
        );
    }

    #[test]
    fn preference_window_preserves_priority_and_applies_caps() {
        // A model that only supports Chat Completions yields a single-entry
        // window.
        let window = backend_preference_window(
            &[ApiBackend::Responses, ApiBackend::ChatCompletions],
            |backend| *backend == ApiBackend::ChatCompletions,
        );
        assert_eq!(window, [Some(ApiBackend::ChatCompletions), None, None]);
    }
}
