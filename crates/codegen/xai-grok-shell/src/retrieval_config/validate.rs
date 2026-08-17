//! Cross-reference and budget validation for the retrieval graph.
//!
//! Provider references are **exact**: missing, disabled, tombstoned, incapable,
//! or protocol-incompatible providers fail/warn graph validation and NEVER
//! retarget to a sibling or built-in. Explicit manual capabilities are
//! authoritative; generic model discovery must not infer embeddings/rerank.

use crate::agent::config_model_override_parse::{ConfigWarning, ConfigWarningKind};
use xai_grok_config_types::{EmbeddingProtocol, RerankerProtocol, RetrievalGraphConfig};

/// One validation issue (error-level for fail-closed save, or warning-level).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphValidationIssue {
    pub path: String,
    pub message: String,
    /// When true, durable save must reject.
    pub hard_error: bool,
}

impl GraphValidationIssue {
    pub fn error(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
            hard_error: true,
        }
    }

    pub fn warn(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
            hard_error: false,
        }
    }
}

/// Secret-free provider capability snapshot used for graph validation.
///
/// Built by the management service from the provider registry. Never carries
/// credentials or endpoints beyond what is needed for capability checks.
#[derive(Debug, Clone, Default)]
pub struct ProviderCapabilityView {
    pub id: String,
    pub enabled: bool,
    pub tombstoned: bool,
    /// Present when the instance exists in the registry (built-in or configured).
    pub exists: bool,
    /// Explicit manual capability: embeddings.
    pub embeddings: Option<bool>,
    /// Explicit manual capability key for rerank (extra or dedicated).
    pub rerank: Option<bool>,
    /// When capability_mode is manual, unknown capabilities are false.
    pub capability_mode_manual: bool,
    /// API surface label (e.g. openai_compatible) — informational for protocol checks.
    pub api_surface: Option<String>,
}

impl ProviderCapabilityView {
    /// Whether this provider can host embedding models.
    ///
    /// Manual capabilities are authoritative. When manual and embeddings is
    /// explicitly false → incapable. When manual and unset → incapable (no
    /// discovery inference). When not manual and embeddings is Some(false) →
    /// incapable; Some(true)/None → allowed (runtime probe is PR16).
    pub fn can_embed(&self) -> bool {
        if !self.exists || !self.enabled || self.tombstoned {
            return false;
        }
        match self.embeddings {
            Some(true) => true,
            Some(false) => false,
            None if self.capability_mode_manual => false,
            None => true, // non-manual: do not invent; allow config, runtime validates later
        }
    }

    pub fn can_rerank(&self) -> bool {
        if !self.exists || !self.enabled || self.tombstoned {
            return false;
        }
        match self.rerank {
            Some(true) => true,
            Some(false) => false,
            None if self.capability_mode_manual => false,
            None => true,
        }
    }
}

/// Validate the graph without provider registry context (structural only).
pub fn validate_retrieval_graph(graph: &RetrievalGraphConfig) -> Vec<GraphValidationIssue> {
    validate_retrieval_graph_with_providers(graph, &[])
}

/// Full graph validation including exact provider reference checks.
pub fn validate_retrieval_graph_with_providers(
    graph: &RetrievalGraphConfig,
    providers: &[ProviderCapabilityView],
) -> Vec<GraphValidationIssue> {
    let mut issues = Vec::new();
    let provider_index: std::collections::HashMap<&str, &ProviderCapabilityView> =
        providers.iter().map(|p| (p.id.as_str(), p)).collect();

    // Embedding models: required fields + exact provider refs.
    for (id, emb) in &graph.embedding_models {
        if emb.provider.trim().is_empty() {
            issues.push(GraphValidationIssue::error(
                format!("embedding_models.\"{id}\".provider"),
                "provider must be non-empty",
            ));
        }
        if emb.model.trim().is_empty() {
            issues.push(GraphValidationIssue::error(
                format!("embedding_models.\"{id}\".model"),
                "upstream model must be non-empty",
            ));
        }
        check_provider_ref(
            &mut issues,
            &format!("embedding_models.\"{id}\".provider"),
            emb.provider.trim(),
            &provider_index,
            providers.is_empty(),
            ProviderNeed::Embeddings,
            emb.protocol,
            None,
        );
    }

    // Reranker models.
    for (id, rr) in &graph.reranker_models {
        if rr.provider.trim().is_empty() {
            issues.push(GraphValidationIssue::error(
                format!("reranker_models.\"{id}\".provider"),
                "provider must be non-empty",
            ));
        }
        if rr.model.trim().is_empty() {
            issues.push(GraphValidationIssue::error(
                format!("reranker_models.\"{id}\".model"),
                "upstream model must be non-empty",
            ));
        }
        if let Some(ep) = &rr.endpoint
            && let Err(err) = xai_grok_config_types::validate_relative_endpoint(ep)
        {
            issues.push(GraphValidationIssue::error(
                format!("reranker_models.\"{id}\".endpoint"),
                err,
            ));
        }
        check_provider_ref(
            &mut issues,
            &format!("reranker_models.\"{id}\".provider"),
            rr.provider.trim(),
            &provider_index,
            providers.is_empty(),
            ProviderNeed::Rerank,
            EmbeddingProtocol::OpenaiCompatible, // unused for rerank protocol branch
            Some(rr.protocol),
        );
    }

    // Profiles: ordered routes + budgets.
    for (id, profile) in &graph.retrieval_profiles {
        if profile.embedding_models.is_empty() {
            issues.push(GraphValidationIssue::error(
                format!("retrieval_profiles.\"{id}\".embedding_models"),
                "at least one embedding model id is required",
            ));
        }
        for emb_id in &profile.embedding_models {
            if !graph.embedding_models.contains_key(emb_id.as_str()) {
                issues.push(GraphValidationIssue::error(
                    format!("retrieval_profiles.\"{id}\".embedding_models"),
                    format!(
                        "embedding model `{emb_id}` is not defined; exact reference required \
                         (never retargeted)"
                    ),
                ));
            }
        }
        for rr_id in &profile.reranker_models {
            if !graph.reranker_models.contains_key(rr_id.as_str()) {
                issues.push(GraphValidationIssue::error(
                    format!("retrieval_profiles.\"{id}\".reranker_models"),
                    format!(
                        "reranker model `{rr_id}` is not defined; exact reference required \
                         (never retargeted)"
                    ),
                ));
            }
        }
        if profile.max_candidates < profile.max_results {
            issues.push(GraphValidationIssue::error(
                format!("retrieval_profiles.\"{id}\".max_candidates"),
                format!(
                    "max_candidates ({}) must be >= max_results ({})",
                    profile.max_candidates, profile.max_results
                ),
            ));
        }
        if !(0.0..=1.0).contains(&profile.min_score) {
            issues.push(GraphValidationIssue::error(
                format!("retrieval_profiles.\"{id}\".min_score"),
                "min_score must be in 0.0..=1.0",
            ));
        }
    }

    // Prime consumers.
    validate_prime_consumer(
        &mut issues,
        "prime.skills",
        graph.prime.skills.enabled,
        graph.prime.skills.retrieval_profile.as_deref(),
        graph.prime.skills.max_results,
        &graph.retrieval_profiles,
    );
    validate_prime_consumer(
        &mut issues,
        "prime.agents",
        graph.prime.agents.enabled,
        graph.prime.agents.retrieval_profile.as_deref(),
        graph.prime.agents.max_results,
        &graph.retrieval_profiles,
    );

    // Memory selection.
    if let Some(profile_id) = &graph.memory_retrieval_profile {
        if !graph.retrieval_profiles.contains_key(profile_id.as_str()) {
            issues.push(GraphValidationIssue::error(
                "memory.retrieval_profile",
                format!(
                    "retrieval profile `{profile_id}` is not defined; exact reference required"
                ),
            ));
        } else if let Some(profile) = graph.retrieval_profiles.get(profile_id.as_str()) {
            // Consumer result limit for memory uses profile max_results; profile
            // candidate >= result already enforced above.
            let _ = profile;
        }
    }

    issues
}

enum ProviderNeed {
    Embeddings,
    Rerank,
}

fn check_provider_ref(
    issues: &mut Vec<GraphValidationIssue>,
    path: &str,
    provider_id: &str,
    index: &std::collections::HashMap<&str, &ProviderCapabilityView>,
    skip_provider_checks: bool,
    need: ProviderNeed,
    emb_protocol: EmbeddingProtocol,
    rr_protocol: Option<RerankerProtocol>,
) {
    if skip_provider_checks || provider_id.is_empty() {
        return;
    }
    let Some(view) = index.get(provider_id).copied() else {
        // Not in index: treat as missing when a non-empty registry was supplied.
        issues.push(GraphValidationIssue::error(
            path,
            format!(
                "provider `{provider_id}` is not registered; exact reference required \
                 (never retargeted to a sibling or built-in)"
            ),
        ));
        return;
    };
    if !view.exists {
        issues.push(GraphValidationIssue::error(
            path,
            format!(
                "provider `{provider_id}` is not registered; exact reference required \
                 (never retargeted)"
            ),
        ));
        return;
    }
    if view.tombstoned {
        issues.push(GraphValidationIssue::error(
            path,
            format!("provider `{provider_id}` is tombstoned; exact reference cannot be retargeted"),
        ));
        return;
    }
    if !view.enabled {
        issues.push(GraphValidationIssue::error(
            path,
            format!(
                "provider `{provider_id}` is disabled; enable it or choose another exact id \
                 (never retargeted)"
            ),
        ));
        return;
    }
    match need {
        ProviderNeed::Embeddings => {
            if !view.can_embed() {
                issues.push(GraphValidationIssue::error(
                    path,
                    format!(
                        "provider `{provider_id}` is not capable of embeddings \
                         (manual capabilities are authoritative; discovery does not infer \
                         embeddings)"
                    ),
                ));
            }
            // Protocol surface: openai_compatible is the only embedding protocol;
            // incompatible api_surface is a hard error when explicitly incompatible.
            if let Some(surface) = view.api_surface.as_deref() {
                let surface_l = surface.to_ascii_lowercase();
                if matches!(emb_protocol, EmbeddingProtocol::OpenaiCompatible)
                    && (surface_l.contains("anthropic") || surface_l == "messages")
                {
                    issues.push(GraphValidationIssue::error(
                        path,
                        format!(
                            "provider `{provider_id}` api_surface `{surface}` is incompatible \
                             with embedding protocol {}",
                            emb_protocol.as_str()
                        ),
                    ));
                }
            }
        }
        ProviderNeed::Rerank => {
            if !view.can_rerank() {
                issues.push(GraphValidationIssue::error(
                    path,
                    format!(
                        "provider `{provider_id}` is not capable of reranking \
                         (manual capabilities are authoritative; discovery does not infer rerank)"
                    ),
                ));
            }
            let _ = rr_protocol;
        }
    }
}

fn validate_prime_consumer(
    issues: &mut Vec<GraphValidationIssue>,
    path: &str,
    enabled: bool,
    profile: Option<&str>,
    max_results: u32,
    profiles: &indexmap::IndexMap<String, xai_grok_config_types::RetrievalProfileConfig>,
) {
    if !enabled {
        return;
    }
    let Some(profile_id) = profile.filter(|s| !s.is_empty()) else {
        issues.push(GraphValidationIssue::error(
            format!("{path}.retrieval_profile"),
            "enabled prime consumer requires a retrieval_profile",
        ));
        return;
    };
    match profiles.get(profile_id) {
        None => issues.push(GraphValidationIssue::error(
            format!("{path}.retrieval_profile"),
            format!("retrieval profile `{profile_id}` is not defined; exact reference required"),
        )),
        Some(p) => {
            if p.max_candidates < max_results {
                issues.push(GraphValidationIssue::error(
                    format!("{path}.max_results"),
                    format!(
                        "consumer max_results ({max_results}) exceeds profile max_candidates ({})",
                        p.max_candidates
                    ),
                ));
            }
            if p.max_results < max_results {
                // Soft: profile result limit is the pool cap for consumers.
                issues.push(GraphValidationIssue::warn(
                    format!("{path}.max_results"),
                    format!(
                        "consumer max_results ({max_results}) exceeds profile max_results ({}); \
                         runtime will clamp",
                        p.max_results
                    ),
                ));
            }
        }
    }
}

/// Convert hard issues into ConfigWarning-compatible summary strings.
pub fn issues_to_warnings(issues: &[GraphValidationIssue]) -> Vec<ConfigWarning> {
    issues
        .iter()
        .map(|i| {
            ConfigWarning::memory_retrieval(
                Some(i.path.as_str()),
                if i.hard_error {
                    ConfigWarningKind::InvalidValue
                } else {
                    ConfigWarningKind::InvalidValue
                },
                i.message.clone(),
            )
        })
        .collect()
}

/// True when any hard error is present.
pub fn has_hard_errors(issues: &[GraphValidationIssue]) -> bool {
    issues.iter().any(|i| i.hard_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_config_types::{
        EmbeddingModelConfig, RerankerModelConfig, RetrievalProfileConfig, SkillPrimeConfig,
    };

    fn emb(provider: &str) -> EmbeddingModelConfig {
        EmbeddingModelConfig {
            provider: provider.into(),
            model: "m".into(),
            ..Default::default()
        }
    }

    #[test]
    fn missing_provider_never_retargets() {
        let mut g = RetrievalGraphConfig::default();
        g.embedding_models.insert("e1".into(), emb("missing-lab"));
        let providers = vec![ProviderCapabilityView {
            id: "other".into(),
            enabled: true,
            exists: true,
            embeddings: Some(true),
            ..Default::default()
        }];
        let issues = validate_retrieval_graph_with_providers(&g, &providers);
        assert!(
            issues
                .iter()
                .any(|i| i.hard_error && i.message.contains("not registered")),
            "{issues:?}"
        );
        assert!(
            issues.iter().all(|i| !i.message.contains("other")),
            "must not retarget to sibling"
        );
    }

    #[test]
    fn disabled_provider_fails() {
        let mut g = RetrievalGraphConfig::default();
        g.embedding_models.insert("e1".into(), emb("lab"));
        let providers = vec![ProviderCapabilityView {
            id: "lab".into(),
            enabled: false,
            exists: true,
            embeddings: Some(true),
            ..Default::default()
        }];
        let issues = validate_retrieval_graph_with_providers(&g, &providers);
        assert!(issues.iter().any(|i| i.message.contains("disabled")));
    }

    #[test]
    fn tombstoned_provider_fails() {
        let mut g = RetrievalGraphConfig::default();
        g.embedding_models.insert("e1".into(), emb("lab"));
        let providers = vec![ProviderCapabilityView {
            id: "lab".into(),
            enabled: true,
            exists: true,
            tombstoned: true,
            embeddings: Some(true),
            ..Default::default()
        }];
        let issues = validate_retrieval_graph_with_providers(&g, &providers);
        assert!(issues.iter().any(|i| i.message.contains("tombstoned")));
    }

    #[test]
    fn manual_incapable_embeddings_fails() {
        let mut g = RetrievalGraphConfig::default();
        g.embedding_models.insert("e1".into(), emb("lab"));
        let providers = vec![ProviderCapabilityView {
            id: "lab".into(),
            enabled: true,
            exists: true,
            embeddings: Some(false),
            capability_mode_manual: true,
            ..Default::default()
        }];
        let issues = validate_retrieval_graph_with_providers(&g, &providers);
        assert!(
            issues
                .iter()
                .any(|i| i.message.contains("not capable of embeddings"))
        );
    }

    #[test]
    fn profile_candidate_ge_results() {
        let mut g = RetrievalGraphConfig::default();
        g.embedding_models.insert("e1".into(), emb("lab"));
        g.retrieval_profiles.insert(
            "p".into(),
            RetrievalProfileConfig {
                embedding_models: vec!["e1".into()],
                max_candidates: 5,
                max_results: 10,
                ..Default::default()
            },
        );
        let issues = validate_retrieval_graph(&g);
        assert!(issues.iter().any(|i| i.message.contains("max_candidates")));
    }

    #[test]
    fn profile_missing_embedding_ref() {
        let mut g = RetrievalGraphConfig::default();
        g.retrieval_profiles.insert(
            "p".into(),
            RetrievalProfileConfig {
                embedding_models: vec!["nope".into()],
                ..Default::default()
            },
        );
        let issues = validate_retrieval_graph(&g);
        assert!(issues.iter().any(|i| i.message.contains("nope")));
    }

    #[test]
    fn prime_requires_profile_when_enabled() {
        let mut g = RetrievalGraphConfig::default();
        g.prime.skills = SkillPrimeConfig {
            enabled: true,
            retrieval_profile: None,
            ..Default::default()
        };
        let issues = validate_retrieval_graph(&g);
        assert!(issues.iter().any(|i| i.path.contains("prime.skills")));
    }

    #[test]
    fn endpoint_attack_in_graph_is_hard_error() {
        let mut g = RetrievalGraphConfig::default();
        g.reranker_models.insert(
            "r1".into(),
            RerankerModelConfig {
                provider: "lab".into(),
                model: "m".into(),
                endpoint: Some("../etc/passwd".into()),
                ..Default::default()
            },
        );
        let issues = validate_retrieval_graph(&g);
        assert!(
            issues
                .iter()
                .any(|i| i.path.contains("endpoint") && i.hard_error)
        );
    }

    #[test]
    fn memory_profile_must_exist() {
        let mut g = RetrievalGraphConfig::default();
        g.memory_retrieval_profile = Some("missing".into());
        let issues = validate_retrieval_graph(&g);
        assert!(
            issues
                .iter()
                .any(|i| i.path.contains("memory.retrieval_profile"))
        );
    }
}
