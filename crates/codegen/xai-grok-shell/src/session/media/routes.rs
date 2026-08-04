//! Ordered route resolution for media-understanding categories (plan section 5).
//!
//! Routes are configured per category in ordered lists: `routes[0]` is
//! primary and the remaining entries are fallbacks. This module resolves each
//! configured route against the live model catalog and decides its runtime
//! eligibility without ever touching inference:
//!
//! - **Unresolved**: the catalog does not currently contain the model ID.
//!   Catalog completeness is transient (plan §5.2): the route is preserved,
//!   marked unresolved, skipped at runtime, and may become valid after
//!   refresh.
//! - **Capability ineligible**: the model's semantic modality support does
//!   not meet the route policy. Per-route TUI-only overrides
//!   (`allow_unknown_capability`, `force_unsupported_capability`) may accept
//!   uncertainty or force an unsupported route — but they never appear in
//!   tool arguments and never bypass managed policy, ZDR, consent,
//!   credentials, or concrete transport requirements (plan §5.3).
//! - **Transport ineligible**: no concrete wire path exists for the route's
//!   strategy (see [`super::transport`]).
//! - **Missing credentials**: the route's model cannot resolve any
//!   credential today. Credential availability can change at runtime, so the
//!   invoker re-checks it immediately before building the route's dedicated
//!   `InferenceClient`.

use indexmap::IndexMap;
use xai_grok_tools::media::domain::{
    MediaCapabilities, MediaCategory, MediaCategoryStrategy, MediaModalitySupport,
    MediaRouteMetadata,
};

use crate::agent::config::{
    ModelEntry, ResolvedMediaCategoryConfig, ResolvedMediaRoute, ResolvedMediaUnderstandingConfig,
};

use super::transport::route_is_transport_eligible;

/// Runtime eligibility of one resolved route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RouteEligibility {
    /// The route can be invoked right now (semantic policy + transport are
    /// both satisfied; credentials are re-verified by the invoker).
    Eligible,
    /// The catalog does not contain the configured model ID.
    Unresolved,
    /// The model's modality support does not satisfy the route policy and no
    /// TUI-only override accepts it.
    CapabilityIneligible,
    /// No concrete wire path exists for the route's strategy.
    TransportIneligible,
    /// The route's model has no resolvable credential.
    MissingCredentials,
}

/// One configured route resolved against the live catalog.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedRoute {
    /// Index within the configured route list (`0` = primary).
    pub config_index: usize,
    pub category: MediaCategory,
    /// Catalog model ID of the delegate route.
    pub model_id: String,
    /// Concrete strategy for this route. `Auto` is resolved to the first
    /// concretely supported strategy during resolution; the resolved value is
    /// what the invoker executes.
    pub strategy: MediaCategoryStrategy,
    /// TUI-user-only: accept semantic capability uncertainty. Never appears
    /// in tool arguments.
    pub allow_unknown_capability: bool,
    /// TUI-user-only: force an unsupported-capability route. Never appears
    /// in tool arguments.
    pub force_unsupported_capability: bool,
    pub eligibility: RouteEligibility,
}

impl ResolvedRoute {
    /// Whether the route may be invoked.
    pub(crate) fn eligible(&self) -> bool {
        self.eligibility == RouteEligibility::Eligible
    }
}

/// Everything route resolution needs from the live session, extracted so the
/// resolution logic stays pure and unit-testable.
pub(crate) struct RouteResolution<'a> {
    /// Resolved `[media_understanding]` config (defaults applied).
    pub config: &'a ResolvedMediaUnderstandingConfig,
    /// Live model catalog snapshot (`ModelsManager::models()`).
    pub models: &'a IndexMap<String, ModelEntry>,
    /// Credential probe: returns whether `model_id` can resolve a credential
    /// today. The invoker re-verifies immediately before building the
    /// dedicated `InferenceClient`. The `Sync` bound keeps the probe
    /// holdable across awaits in `Send` futures.
    pub has_credentials: &'a (dyn Fn(&str) -> bool + Sync),
}

/// Outcome of resolving one configured route.
struct ResolvedOutcome {
    strategy: MediaCategoryStrategy,
    eligibility: RouteEligibility,
}

impl RouteResolution<'_> {
    /// Resolve the ordered route list for one category.
    ///
    /// `Auto` is a request-time hint resolved against the image route list;
    /// the resolved routes carry the concrete `Image` category (the backend
    /// sniffs the concrete category from the bytes before resolving, so this
    /// only matters for callers that pass `Auto` directly).
    ///
    /// Traced as `media.route.resolve` (plan section 17) with the category
    /// only — never model IDs, credentials, or route policy fields.
    #[tracing::instrument(
        name = "media.route.resolve",
        level = "debug",
        skip_all,
        fields(category = ?category)
    )]
    pub(crate) fn category_routes(&self, category: MediaCategory) -> Vec<ResolvedRoute> {
        let category = concrete_category(category);
        let category_config = resolve_category_config(self.config, category);
        category_config
            .routes
            .iter()
            .enumerate()
            .map(|(config_index, route)| {
                let outcome = self.resolve_route(category, route);
                ResolvedRoute {
                    config_index,
                    category,
                    model_id: route.model.clone(),
                    strategy: outcome.strategy,
                    allow_unknown_capability: route.allow_unknown_capability,
                    force_unsupported_capability: route.force_unsupported_capability,
                    eligibility: outcome.eligibility,
                }
            })
            .collect()
    }

    fn resolve_route(
        &self,
        category: MediaCategory,
        route: &ResolvedMediaRoute,
    ) -> ResolvedOutcome {
        let Some(entry) = self.models.get(&route.model) else {
            return ResolvedOutcome {
                strategy: route.strategy,
                eligibility: RouteEligibility::Unresolved,
            };
        };
        if !(self.has_credentials)(&route.model) {
            return ResolvedOutcome {
                strategy: route.strategy,
                eligibility: RouteEligibility::MissingCredentials,
            };
        }
        // Strategy: `Auto` resolves against the concrete wire paths the
        // provider advertises; otherwise the configured concrete strategy is
        // used verbatim (keeping ordered fallback semantics authoritative).
        let strategy = if route.strategy == MediaCategoryStrategy::Auto {
            match super::transport::concrete_strategy_for_auto(category, &entry.media_transport) {
                Some(strategy) => strategy,
                None => {
                    return ResolvedOutcome {
                        strategy: route.strategy,
                        eligibility: RouteEligibility::TransportIneligible,
                    };
                }
            }
        } else {
            route.strategy
        };
        if !route_is_transport_eligible(category, strategy, &entry.media_transport) {
            return ResolvedOutcome {
                strategy,
                eligibility: RouteEligibility::TransportIneligible,
            };
        }
        if !semantic_capability_eligible(
            category,
            &entry.media_capabilities,
            route.allow_unknown_capability,
            route.force_unsupported_capability,
        ) {
            return ResolvedOutcome {
                strategy,
                eligibility: RouteEligibility::CapabilityIneligible,
            };
        }
        ResolvedOutcome {
            strategy,
            eligibility: RouteEligibility::Eligible,
        }
    }

    /// Whether the category has at least one eligible route right now.
    pub(crate) fn category_has_eligible_route(&self, category: MediaCategory) -> bool {
        self.category_routes(category)
            .iter()
            .any(ResolvedRoute::eligible)
    }

    /// Availability metadata for one category in configured order.
    pub(crate) fn category_availability(&self, category: MediaCategory) -> Vec<MediaRouteMetadata> {
        self.category_routes(category)
            .iter()
            .map(|route| MediaRouteMetadata {
                model_id: route.model_id.clone(),
                strategy: route.strategy,
                unresolved: route.eligibility == RouteEligibility::Unresolved,
                eligible: route.eligible(),
            })
            .collect()
    }
}

/// Resolve the per-category config slot (image/audio/video).
///
/// `Auto` is a request-time hint, not a configurable route category; it
/// resolves against the image route list.
pub(crate) fn resolve_category_config<'a>(
    config: &'a ResolvedMediaUnderstandingConfig,
    category: MediaCategory,
) -> &'a ResolvedMediaCategoryConfig {
    match concrete_category(category) {
        MediaCategory::Image => &config.image,
        MediaCategory::Audio => &config.audio,
        MediaCategory::Video => &config.video,
        MediaCategory::Auto => unreachable!("concrete_category never returns Auto"),
    }
}

/// Normalize the request-time `Auto` hint to the concrete image category.
fn concrete_category(category: MediaCategory) -> MediaCategory {
    match category {
        MediaCategory::Auto => MediaCategory::Image,
        other => other,
    }
}

/// Whether the model's semantic modality support satisfies the route policy.
///
/// `Supported` is always eligible. `Unknown` requires the TUI-only
/// `allow_unknown_capability` override. `Unsupported` requires the TUI-only
/// `force_unsupported_capability` override. Neither override bypasses
/// transport, consent, ZDR, credentials, managed policy, or budgets.
fn semantic_capability_eligible(
    category: MediaCategory,
    capabilities: &MediaCapabilities,
    allow_unknown: bool,
    force_unsupported: bool,
) -> bool {
    let support = match category {
        MediaCategory::Image | MediaCategory::Auto => capabilities.image,
        MediaCategory::Audio => capabilities.audio,
        MediaCategory::Video => capabilities.video,
    };
    match support {
        MediaModalitySupport::Supported => true,
        MediaModalitySupport::Unknown => allow_unknown,
        MediaModalitySupport::Unsupported => force_unsupported,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::{
        ModelInfo, ResolvedMediaCategoryConfig, ResolvedMediaCircuitBreakerConfig,
    };
    use indexmap::IndexMap;
    use xai_grok_tools::media::domain::MediaTransportCapabilities;

    fn entry_with(
        model: &str,
        caps: MediaCapabilities,
        transport: MediaTransportCapabilities,
    ) -> ModelEntry {
        let mut info = ModelInfo::fallback(model);
        info.media_capabilities = caps;
        info.media_transport = transport;
        ModelEntry {
            info,
            model_provider: None,
            api_key: Some("k".to_string()),
            env_key: None,
            auth_provider: None,
            api_base_url: None,
        }
    }

    fn image_inline() -> MediaTransportCapabilities {
        MediaTransportCapabilities {
            image_inline: true,
            json_schema: true,
            ..MediaTransportCapabilities::default()
        }
    }

    // `ResolvedMediaUnderstandingConfig` is a plain struct (no Default), so
    // tests build it field-by-field through this helper.
    fn base_config() -> ResolvedMediaUnderstandingConfig {
        ResolvedMediaUnderstandingConfig {
            enabled: true,
            auto_enrich: false,
            compaction_enrichment: false,
            active_model_unknown_policy: Default::default(),
            compaction_preflight_policy: Default::default(),
            max_output_chars: 20_000,
            max_aux_tokens_per_call: 8_192,
            max_aux_budget_usd_ticks: 1_000_000_000,
            max_media_bytes: 256 * 1024 * 1024,
            max_audio_seconds: 1_800,
            max_video_seconds: 900,
            max_video_frames: 32,
            max_contact_sheet_side_px: 2_048,
            max_preprocess_wallclock_ms: 120_000,
            preprocess_concurrency: 2,
            circuit_breaker: ResolvedMediaCircuitBreakerConfig {
                failures: 5,
                window_secs: 300,
            },
            image: empty_category(),
            audio: empty_category(),
            video: empty_category(),
        }
    }

    fn empty_category() -> ResolvedMediaCategoryConfig {
        ResolvedMediaCategoryConfig {
            routes: vec![],
            max_seconds: None,
            max_frames: None,
        }
    }

    fn image_route(model: &str, strategy: MediaCategoryStrategy) -> ResolvedMediaRoute {
        ResolvedMediaRoute {
            model: model.to_string(),
            strategy,
            allow_unknown_capability: false,
            force_unsupported_capability: false,
        }
    }

    #[test]
    fn media_routes_ordered_fallback_semantics_preserved() {
        let mut config = base_config();
        config.image.routes = vec![
            image_route("primary-vision", MediaCategoryStrategy::Native),
            image_route("fallback-vision", MediaCategoryStrategy::Native),
        ];
        let mut models = IndexMap::new();
        for model in ["primary-vision", "fallback-vision"] {
            models.insert(
                model.to_string(),
                entry_with(
                    model,
                    MediaCapabilities {
                        image: MediaModalitySupport::Supported,
                        ..Default::default()
                    },
                    image_inline(),
                ),
            );
        }

        let resolution = RouteResolution {
            config: &config,
            models: &models,
            has_credentials: &|_| true,
        };
        let routes = resolution.category_routes(MediaCategory::Image);
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].model_id, "primary-vision");
        assert_eq!(routes[0].config_index, 0);
        assert!(routes[0].eligible());
        assert_eq!(routes[1].model_id, "fallback-vision");
        assert_eq!(routes[1].config_index, 1);
        assert!(routes[1].eligible());
    }

    #[test]
    fn media_routes_unresolved_and_missing_credentials_marked() {
        let mut config = base_config();
        config.image.routes = vec![
            image_route("ghost-model", MediaCategoryStrategy::Native),
            image_route("locked-model", MediaCategoryStrategy::Native),
        ];
        let mut models = IndexMap::new();
        models.insert(
            "locked-model".to_string(),
            entry_with(
                "locked-model",
                MediaCapabilities {
                    image: MediaModalitySupport::Supported,
                    ..Default::default()
                },
                image_inline(),
            ),
        );

        let resolution = RouteResolution {
            config: &config,
            models: &models,
            has_credentials: &|model| model != "locked-model",
        };
        let routes = resolution.category_routes(MediaCategory::Image);
        assert_eq!(routes[0].eligibility, RouteEligibility::Unresolved);
        assert_eq!(routes[1].eligibility, RouteEligibility::MissingCredentials);
        assert!(!resolution.category_has_eligible_route(MediaCategory::Image));
        let availability = resolution.category_availability(MediaCategory::Image);
        assert_eq!(availability.len(), 2);
        assert!(availability[0].unresolved);
        assert!(!availability[1].eligible);
    }

    #[test]
    fn media_routes_capability_policy_and_overrides() {
        let mut config = base_config();
        config.image.routes = vec![
            image_route("unknown-model", MediaCategoryStrategy::Native),
            ResolvedMediaRoute {
                model: "unknown-ack".to_string(),
                strategy: MediaCategoryStrategy::Native,
                allow_unknown_capability: true,
                force_unsupported_capability: false,
            },
            ResolvedMediaRoute {
                model: "unsupported-forced".to_string(),
                strategy: MediaCategoryStrategy::Native,
                allow_unknown_capability: false,
                force_unsupported_capability: true,
            },
            image_route("unsupported-model", MediaCategoryStrategy::Native),
        ];
        let mut models = IndexMap::new();
        for model in [
            "unknown-model",
            "unknown-ack",
            "unsupported-forced",
            "unsupported-model",
        ] {
            let support = if model == "unsupported-forced" || model == "unsupported-model" {
                MediaModalitySupport::Unsupported
            } else {
                MediaModalitySupport::Unknown
            };
            models.insert(
                model.to_string(),
                entry_with(
                    model,
                    MediaCapabilities {
                        image: support,
                        ..Default::default()
                    },
                    image_inline(),
                ),
            );
        }

        let resolution = RouteResolution {
            config: &config,
            models: &models,
            has_credentials: &|_| true,
        };
        let routes = resolution.category_routes(MediaCategory::Image);
        assert_eq!(
            routes[0].eligibility,
            RouteEligibility::CapabilityIneligible
        );
        assert!(routes[1].eligible(), "allow_unknown accepts uncertainty");
        assert!(routes[2].eligible(), "force_unsupported overrides policy");
        assert_eq!(
            routes[3].eligibility,
            RouteEligibility::CapabilityIneligible
        );
    }

    #[test]
    fn media_routes_transport_ineligible_skipped() {
        let mut config = base_config();
        config.image.routes = vec![image_route(
            "no-inline-model",
            MediaCategoryStrategy::Native,
        )];
        let mut models = IndexMap::new();
        models.insert(
            "no-inline-model".to_string(),
            entry_with(
                "no-inline-model",
                MediaCapabilities {
                    image: MediaModalitySupport::Supported,
                    ..Default::default()
                },
                MediaTransportCapabilities::default(),
            ),
        );

        let resolution = RouteResolution {
            config: &config,
            models: &models,
            has_credentials: &|_| true,
        };
        let routes = resolution.category_routes(MediaCategory::Image);
        assert_eq!(routes[0].eligibility, RouteEligibility::TransportIneligible);
        assert!(!resolution.category_has_eligible_route(MediaCategory::Image));
    }

    #[test]
    fn media_routes_auto_strategy_resolves_transport_first() {
        let mut config = base_config();
        // A video `auto` route on a provider with image inline but no native
        // video adapter resolves to `frames`.
        config.video.routes = vec![ResolvedMediaRoute {
            model: "video-model".to_string(),
            strategy: MediaCategoryStrategy::Auto,
            allow_unknown_capability: false,
            force_unsupported_capability: false,
        }];
        let mut models = IndexMap::new();
        models.insert(
            "video-model".to_string(),
            entry_with(
                "video-model",
                MediaCapabilities {
                    video: MediaModalitySupport::Supported,
                    image: MediaModalitySupport::Supported,
                    ..Default::default()
                },
                image_inline(),
            ),
        );

        let resolution = RouteResolution {
            config: &config,
            models: &models,
            has_credentials: &|_| true,
        };
        let routes = resolution.category_routes(MediaCategory::Video);
        assert_eq!(routes[0].strategy, MediaCategoryStrategy::Frames);
        assert!(routes[0].eligible());
    }

    #[test]
    fn media_routes_audio_never_eligible_until_adapter() {
        let mut config = base_config();
        config.audio.routes = vec![ResolvedMediaRoute {
            model: "audio-model".to_string(),
            strategy: MediaCategoryStrategy::Transcription,
            allow_unknown_capability: false,
            force_unsupported_capability: false,
        }];
        let mut models = IndexMap::new();
        models.insert(
            "audio-model".to_string(),
            entry_with(
                "audio-model",
                MediaCapabilities {
                    audio: MediaModalitySupport::Supported,
                    ..Default::default()
                },
                MediaTransportCapabilities {
                    transcription_endpoint: true,
                    ..MediaTransportCapabilities::default()
                },
            ),
        );

        let resolution = RouteResolution {
            config: &config,
            models: &models,
            has_credentials: &|_| true,
        };
        let routes = resolution.category_routes(MediaCategory::Audio);
        assert_eq!(routes[0].eligibility, RouteEligibility::TransportIneligible);
        assert!(!resolution.category_has_eligible_route(MediaCategory::Audio));
    }

    #[test]
    fn media_routes_category_availability_reflects_eligibility() {
        let mut config = base_config();
        config.image.routes = vec![image_route("ready-model", MediaCategoryStrategy::Native)];
        let mut models = IndexMap::new();
        models.insert(
            "ready-model".to_string(),
            entry_with(
                "ready-model",
                MediaCapabilities {
                    image: MediaModalitySupport::Supported,
                    ..Default::default()
                },
                image_inline(),
            ),
        );
        let resolution = RouteResolution {
            config: &config,
            models: &models,
            has_credentials: &|_| true,
        };
        assert!(resolution.category_has_eligible_route(MediaCategory::Image));
        let availability = resolution.category_availability(MediaCategory::Image);
        assert_eq!(availability.len(), 1);
        assert!(availability[0].eligible);
        assert!(!availability[0].unresolved);
    }

    #[test]
    fn media_routes_auto_category_uses_image_config_slot() {
        let mut config = base_config();
        config.image.routes = vec![image_route(
            "auto-category-model",
            MediaCategoryStrategy::Native,
        )];
        let mut models = IndexMap::new();
        models.insert(
            "auto-category-model".to_string(),
            entry_with(
                "auto-category-model",
                MediaCapabilities {
                    image: MediaModalitySupport::Supported,
                    ..Default::default()
                },
                image_inline(),
            ),
        );
        let resolution = RouteResolution {
            config: &config,
            models: &models,
            has_credentials: &|_| true,
        };
        // `Auto` is a request-time hint resolved against the image routes;
        // the resolved routes carry the concrete `Image` category.
        let routes = resolution.category_routes(MediaCategory::Auto);
        assert_eq!(routes.len(), 1);
        assert!(routes[0].eligible());
        assert_eq!(routes[0].category, MediaCategory::Image);
    }
}
