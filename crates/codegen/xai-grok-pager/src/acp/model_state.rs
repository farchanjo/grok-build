//! Model state — tracks available models and current selection.

use agent_client_protocol as acp;
use indexmap::IndexMap;
use xai_grok_shell::inference::types::{
    ReasoningEffort, ReasoningEffortOption, ReasoningEffortSelection, parse_reasoning_effort_meta,
    parse_reasoning_effort_selection_meta, parse_reasoning_efforts_meta,
};

/// Why an effort token could not be applied to a model. Shared by every effort
/// surface (`/effort`, the CLI deferred switch, and headless) so they classify
/// the same input identically and differ only in how they surface the error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EffortTokenError {
    /// The target model does not advertise `supportsReasoningEffort`.
    Unsupported,
    /// The token is neither a menu id nor a canonical value offered by this
    /// model's menu. `offered` is the model-specific list of option ids the
    /// user can type (never a hardcoded global set — so we do not advertise
    /// `none`/`minimal` when the model does not offer them).
    UnknownToken { token: String, offered: Vec<String> },
    /// No active model to resolve the effort against.
    NoActiveModel,
}

impl EffortTokenError {
    pub(crate) fn message(&self) -> String {
        match self {
            Self::Unsupported => "current model does not support reasoning effort".to_string(),
            Self::UnknownToken { token, offered } => {
                if offered.is_empty() {
                    format!(
                        "unknown effort level '{token}'; this model has no selectable effort levels"
                    )
                } else {
                    format!(
                        "unknown effort level '{token}'; use one of: {}",
                        offered.join(", ")
                    )
                }
            }
            Self::NoActiveModel => "no active model to apply effort to".to_string(),
        }
    }
}

/// Outcome of resolving a user-typed name/id against the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelResolveResult {
    /// Unique match (exact id, unique display name, or unique alias).
    Resolved(acp::ModelId),
    /// Multiple distinct catalog entries share the label — never auto-pick.
    Ambiguous {
        query: String,
        candidates: Vec<acp::ModelId>,
    },
    /// No match.
    Missing { query: String },
}

impl ModelResolveResult {
    pub fn ok(self) -> Option<acp::ModelId> {
        match self {
            Self::Resolved(id) => Some(id),
            Self::Ambiguous { .. } | Self::Missing { .. } => None,
        }
    }

    pub fn is_ambiguous(&self) -> bool {
        matches!(self, Self::Ambiguous { .. })
    }
}

/// Per-agent model state.
#[derive(Debug, Clone, Default)]
pub struct ModelState {
    pub available: IndexMap<acp::ModelId, acp::ModelInfo>,
    pub current: Option<acp::ModelId>,
    pub reasoning_effort: Option<ReasoningEffort>,
    /// The normalized reasoning effort selection state for the current model.
    /// Derived from ACP meta with legacy projection fallback.
    pub reasoning_effort_selection: ReasoningEffortSelection,
    /// Catalog publication generation from the last atomic models/update.
    /// Stale open-picker selections that predate this generation are rejected.
    pub catalog_generation: u64,
    /// External override for the context window size (tokens).
    /// When set, `get_context_window()` returns this instead of
    /// reading from the current model's metadata. Used for subagent
    /// views where SubagentProgress reports the actual window size.
    context_window_override: Option<u64>,
}

impl ModelState {
    pub fn is_empty(&self) -> bool {
        self.available.is_empty()
    }

    /// Display name for the current model.
    pub fn current_model_name(&self) -> Option<String> {
        let current = self.current.as_ref()?;
        if let Some(model_info) = self.available.get(current) {
            Some(model_info.name.clone())
        } else {
            Some(current.0.to_string())
        }
    }

    /// Machine-readable model ID string for the current model (e.g. "grok-4.5").
    pub fn current_model_id_str(&self) -> Option<&str> {
        Some(self.current.as_ref()?.0.as_ref())
    }

    /// Total context window tokens for the current model (if available).
    fn current_context_window_tokens(&self) -> Option<u64> {
        let meta = self.available.get(self.current.as_ref()?)?.meta.as_ref()?;
        meta.get("totalContextTokens")
            .and_then(|value| match value {
                serde_json::Value::Number(number) => number.as_u64(),
                _ => None,
            })
    }

    /// Whether the current model accepts image input, read from the model's
    /// `meta` (the ACP extension point — same source as `totalContextTokens`).
    ///
    /// Honors an explicit `acceptsImages` bool, else an `inputModalities` array
    /// containing `"image"`. DEFAULTS TO `true` when neither key is present:
    /// correct today (all current Grok models accept images, so nothing is
    /// suppressed) and forward-compatible (suppresses non-vision models once the
    /// ACP server populates the key). Populating that key server-side is a
    /// separate change.
    pub fn current_model_accepts_images(&self) -> bool {
        let Some(meta) = self
            .current
            .as_ref()
            .and_then(|id| self.available.get(id))
            .and_then(|info| info.meta.as_ref())
        else {
            return true;
        };
        if let Some(accepts) = meta.get("acceptsImages").and_then(|v| v.as_bool()) {
            return accepts;
        }
        if let Some(modalities) = meta.get("inputModalities").and_then(|v| v.as_array()) {
            return modalities
                .iter()
                .any(|m| m.as_str().is_some_and(|s| s.eq_ignore_ascii_case("image")));
        }
        true
    }

    /// Get the effective context window size (tokens).
    ///
    /// Returns the override if set, otherwise reads from the current model's
    /// metadata. The override is set by `override_context_window()` when an
    /// external source (e.g., SubagentProgress) reports the actual window size.
    pub fn get_context_window(&self) -> Option<u64> {
        self.context_window_override
            .or_else(|| self.current_context_window_tokens())
    }

    /// Override the context window size.
    ///
    /// Used for subagent views where the actual context window is reported
    /// via SubagentProgress and may differ from the inherited model's metadata.
    pub fn override_context_window(&mut self, tokens: u64) {
        self.context_window_override = Some(tokens);
    }

    /// Replace the available models, preserving current selection if still valid.
    ///
    /// Ungenerated path (tests / local assignment) always applies content.
    pub fn update_catalog(
        &mut self,
        new_available: IndexMap<acp::ModelId, acp::ModelInfo>,
        fallback_current: Option<acp::ModelId>,
    ) {
        let _ = self.update_catalog_versioned(new_available, fallback_current, None);
    }

    /// Versioned catalog update used by `x.ai/models/update`.
    ///
    /// Returns `true` when the catalog was applied. Returns `false` when the
    /// update is rejected as stale so callers can skip slash rebuilds.
    ///
    /// Policy:
    /// - `incoming_generation < current` (and current > 0): reject entirely —
    ///   neither `available` nor generation changes.
    /// - `incoming_generation == current` (and current > 0): no-op (shell bumps
    ///   generation on every real catalog change).
    /// - `incoming_generation > current` or generation omitted: apply content
    ///   and advance generation atomically with the catalog swap.
    pub fn update_catalog_versioned(
        &mut self,
        new_available: IndexMap<acp::ModelId, acp::ModelInfo>,
        fallback_current: Option<acp::ModelId>,
        catalog_generation: Option<u64>,
    ) -> bool {
        if let Some(incoming_generation) = catalog_generation {
            if self.catalog_generation > 0 && incoming_generation < self.catalog_generation {
                tracing::debug!(
                    incoming = incoming_generation,
                    current = self.catalog_generation,
                    "rejecting stale models/update (lower catalog generation)"
                );
                return false;
            }
            if self.catalog_generation > 0 && incoming_generation == self.catalog_generation {
                tracing::debug!(
                    generation = incoming_generation,
                    "ignoring models/update with equal catalog generation"
                );
                return false;
            }
        }

        let previous_current_model = self.current.clone();
        self.available = new_available;
        if let Some(incoming_generation) = catalog_generation {
            self.catalog_generation = incoming_generation;
        }
        if let Some(ref id) = self.current {
            if !self.available.contains_key(id) {
                // Preserve exact canonical selection when possible; only fall
                // back when the selected catalog key was removed.
                self.current = fallback_current;
            }
        } else {
            self.current = fallback_current;
        }
        // The models/update broadcast carries each model's static default effort,
        // not this session's choice; only re-derive when the model changed so a
        // catalog refresh can't clobber a user-set effort.
        if self.current != previous_current_model {
            if let Some(id) = &self.current {
                if let Some(info) = self.available.get(id) {
                    let options = parse_reasoning_efforts_meta(info.meta.as_ref());
                    self.reasoning_effort_selection = parse_reasoning_effort_selection_meta(
                        info.meta.as_ref(),
                        &options.unwrap_or_default(),
                    );
                    self.reasoning_effort = parse_reasoning_effort_meta(info.meta.as_ref());
                }
            }
        }
        true
    }

    /// Secret-free provider instance id from model meta (if present).
    pub fn provider_instance_id(&self, id: &acp::ModelId) -> Option<&str> {
        self.available
            .get(id)
            .and_then(|info| info.meta.as_ref())
            .and_then(|m| m.get("providerInstanceId"))
            .and_then(|v| v.as_str())
    }

    /// Secret-free provider kind from model meta (if present).
    pub fn provider_kind(&self, id: &acp::ModelId) -> Option<&str> {
        self.available
            .get(id)
            .and_then(|info| info.meta.as_ref())
            .and_then(|m| m.get("providerKind"))
            .and_then(|v| v.as_str())
    }

    /// Set the current model and resolve reasoning effort from catalog meta.
    pub fn set_current(
        &mut self,
        model_id: acp::ModelId,
        effort_override: Option<ReasoningEffort>,
    ) {
        self.current = Some(model_id.clone());
        if let Some(info) = self.available.get(&model_id) {
            let options = parse_reasoning_efforts_meta(info.meta.as_ref());
            self.reasoning_effort_selection = parse_reasoning_effort_selection_meta(
                info.meta.as_ref(),
                &options.unwrap_or_default(),
            );
        }
        self.reasoning_effort = effort_override.or_else(|| {
            self.available
                .get(&model_id)
                .and_then(|info| parse_reasoning_effort_meta(info.meta.as_ref()))
        });
    }

    /// Derive the normalized reasoning effort selection for a model id from its meta.
    pub(crate) fn derive_reasoning_effort_selection(
        &self,
        id: &acp::ModelId,
    ) -> ReasoningEffortSelection {
        let Some(info) = self.available.get(id) else {
            return ReasoningEffortSelection::default();
        };
        let options = parse_reasoning_efforts_meta(info.meta.as_ref()).unwrap_or_default();
        parse_reasoning_effort_selection_meta(info.meta.as_ref(), &options)
    }

    /// The reasoning-effort menu for the current model. Gate-first: an unset or
    /// unsupported model yields no menu; a supported model uses the server list
    /// when present, else the built-in fallback.
    pub fn reasoning_effort_options(&self) -> Vec<ReasoningEffortOption> {
        match self.current.as_ref() {
            Some(id) => self.reasoning_effort_options_for(id),
            None => Vec::new(),
        }
    }

    /// Menu for a specific catalog model id (used by `/model`'s effort phase).
    /// Uses the normalized selection state to determine the menu:
    /// - Unknown/Unsupported: empty menu
    /// - LegacyFallback: xhigh/high/medium/low
    /// - Exact: the server-provided options
    /// - Unrestricted: all canonical values in strongest-first order
    pub(crate) fn reasoning_effort_options_for(
        &self,
        id: &acp::ModelId,
    ) -> Vec<ReasoningEffortOption> {
        let Some(info) = self.available.get(id) else {
            return Vec::new();
        };
        let selection = self.derive_reasoning_effort_selection(id);
        let options = parse_reasoning_efforts_meta(info.meta.as_ref()).unwrap_or_default();
        selection.menu_options(&options)
    }

    /// Map a typed/selected effort token to its canonical value for the current
    /// model. Accepts a menu option id (case-insensitive) or a canonical level
    /// that appears as a **value** in that model's menu. Levels the model does
    /// not offer (e.g. `none` on grok-4.5) are rejected so we fail in the TUI
    /// instead of sending a blocked effort to the API.
    pub fn resolve_effort_token(&self, token: &str) -> Option<ReasoningEffort> {
        match self.current.as_ref() {
            Some(id) => self.resolve_effort_token_for(id, token),
            // No model yet: still parse so deferred CLI can hold a token; it is
            // re-validated with `resolve_effort_for_model` once a model is active.
            None => token.parse::<ReasoningEffort>().ok(),
        }
    }

    /// [`Self::resolve_effort_token`] scoped to a specific catalog model id.
    pub(crate) fn resolve_effort_token_for(
        &self,
        id: &acp::ModelId,
        token: &str,
    ) -> Option<ReasoningEffort> {
        let selection = self.derive_reasoning_effort_selection(id);
        let options = self.reasoning_effort_options_for(id);
        selection.resolve_token(token, &options).ok()
    }

    /// Canonical effort-token policy: gate on the model's support flag first,
    /// then resolve the token (menu id or canonical level). This is the single
    /// decision shared by `/effort`, the CLI deferred switch, and headless —
    /// each caller only maps the [`EffortTokenError`] to its own surface.
    pub(crate) fn resolve_effort_for_model(
        &self,
        id: &acp::ModelId,
        token: &str,
    ) -> Result<ReasoningEffort, EffortTokenError> {
        let selection = self.derive_reasoning_effort_selection(id);
        if !selection.accepts_canonical() {
            return Err(EffortTokenError::Unsupported);
        }
        let options = self.reasoning_effort_options_for(id);
        let result = selection.resolve_token(token, &options).map_err(|msg| {
            // Parse the error message to extract offered options if present
            // The shared helper returns a formatted error message
            if let Some(offered) =
                msg.strip_prefix(&format!("unknown effort level '{}'; use one of: ", token))
            {
                EffortTokenError::UnknownToken {
                    token: token.to_string(),
                    offered: offered.split(',').map(|s| s.trim().to_string()).collect(),
                }
            } else if msg.contains("not in legacy ladder") {
                // LegacyFallback state error
                EffortTokenError::UnknownToken {
                    token: token.to_string(),
                    offered: selection
                        .canonical_ladder()
                        .iter()
                        .map(|e| e.as_str().to_string())
                        .collect(),
                }
            } else {
                // Generic unknown token error
                EffortTokenError::UnknownToken {
                    token: token.to_string(),
                    offered: options.iter().map(|o| o.id.clone()).collect(),
                }
            }
        })?;
        Ok(result)
    }

    /// Resolve a user-supplied name to a `ModelId` via case-insensitive
    /// ASCII match against the catalog.
    ///
    /// **Deterministic, fail-closed on ambiguity:** exact catalog id wins;
    /// unique display-name match resolves; multiple distinct ids that share a
    /// label return `None` (use [`Self::resolve_by_name_or_id_detailed`] for
    /// candidate lists). Never silently picks a sibling account.
    pub fn resolve_by_name_or_id(&self, query: &str) -> Option<acp::ModelId> {
        self.resolve_by_name_or_id_detailed(query).ok()
    }

    /// Detailed resolve used by `/model` error surfaces.
    pub fn resolve_by_name_or_id_detailed(&self, query: &str) -> ModelResolveResult {
        let query = query.trim();
        if query.is_empty() {
            return ModelResolveResult::Missing {
                query: query.to_owned(),
            };
        }

        // 1. Exact canonical selection id (case-insensitive).
        if let Some((id, _)) = self
            .available
            .iter()
            .find(|(id, _)| id.0.as_ref().eq_ignore_ascii_case(query))
        {
            return ModelResolveResult::Resolved(id.clone());
        }

        // 2. Collect display-name and meta-upstream matches.
        let mut candidates: Vec<acp::ModelId> = self
            .available
            .iter()
            .filter(|(id, info)| {
                info.name.eq_ignore_ascii_case(query)
                    || info
                        .meta
                        .as_ref()
                        .and_then(|m| m.get("upstreamModelId"))
                        .and_then(|v| v.as_str())
                        .is_some_and(|u| u.eq_ignore_ascii_case(query))
                    // Bare id after first colon (openai:gpt-4o → gpt-4o).
                    || id
                        .0
                        .as_ref()
                        .split_once(':')
                        .is_some_and(|(_, rest)| rest.eq_ignore_ascii_case(query))
            })
            .map(|(id, _)| id.clone())
            .collect();
        candidates.sort_by(|a, b| a.0.as_ref().cmp(b.0.as_ref()));
        candidates.dedup_by(|a, b| a.0.as_ref() == b.0.as_ref());

        match candidates.len() {
            0 => ModelResolveResult::Missing {
                query: query.to_owned(),
            },
            1 => ModelResolveResult::Resolved(candidates.remove(0)),
            _ => ModelResolveResult::Ambiguous {
                query: query.to_owned(),
                candidates,
            },
        }
    }

    /// Look up the display name for a `ModelId` in the catalog.
    pub fn display_name_for(&self, id: &acp::ModelId) -> String {
        self.available
            .get(id)
            .map(|info| info.name.clone())
            .unwrap_or_else(|| id.0.to_string())
    }

    /// Cycle to the next model.
    pub fn next_model(&self) -> Option<acp::ModelId> {
        if self.available.is_empty() {
            None
        } else if let Some(ref current) = self.current {
            let idx = self.available.get_index_of(current)?;
            let idx = (idx + 1) % self.available.len();
            Some(self.available.get_index(idx)?.0.clone())
        } else {
            Some(self.available.first()?.0.clone())
        }
    }
}

impl From<Option<acp::SessionModelState>> for ModelState {
    fn from(state: Option<acp::SessionModelState>) -> Self {
        state
            .map(|state| {
                let catalog_generation = state
                    .meta
                    .as_ref()
                    .and_then(|m| m.get("catalogGeneration"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let mut models = IndexMap::new();
                for model in state.available_models {
                    models.insert(model.model_id.clone(), model);
                }
                let current_model = models
                    .contains_key(&state.current_model_id)
                    .then_some(state.current_model_id);
                let (reasoning_effort_selection, reasoning_effort) = current_model
                    .as_ref()
                    .and_then(|id| models.get(id))
                    .map(|info| {
                        let options =
                            parse_reasoning_efforts_meta(info.meta.as_ref()).unwrap_or_default();
                        let selection =
                            parse_reasoning_effort_selection_meta(info.meta.as_ref(), &options);
                        let effort = parse_reasoning_effort_meta(info.meta.as_ref());
                        (selection, effort)
                    })
                    .unwrap_or((ReasoningEffortSelection::default(), None));
                Self {
                    available: models,
                    current: current_model,
                    reasoning_effort,
                    reasoning_effort_selection,
                    catalog_generation,
                    context_window_override: None,
                }
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn sample_models() -> ModelState {
        let mut state = ModelState::default();
        let id_a = acp::ModelId::new(Arc::from("model-a"));
        let id_b = acp::ModelId::new(Arc::from("model-b"));
        state.available.insert(
            id_a.clone(),
            acp::ModelInfo::new(id_a.clone(), "Model A".to_string()),
        );
        state.available.insert(
            id_b.clone(),
            acp::ModelInfo::new(id_b.clone(), "Model B".to_string()),
        );
        state.current = Some(id_a);
        state
    }

    #[test]
    fn test_current_model_name() {
        let state = sample_models();
        assert_eq!(state.current_model_name(), Some("Model A".to_string()));
    }

    #[test]
    fn test_next_model_cycles() {
        let state = sample_models();
        let next = state.next_model().unwrap();
        assert_eq!(next.0.as_ref(), "model-b");
    }

    #[test]
    fn test_next_model_wraps() {
        let mut state = sample_models();
        state.current = Some(acp::ModelId::new(Arc::from("model-b")));
        let next = state.next_model().unwrap();
        assert_eq!(next.0.as_ref(), "model-a");
    }

    #[test]
    fn test_empty_state() {
        let state = ModelState::default();
        assert!(state.is_empty());
        assert!(state.current_model_name().is_none());
        assert!(state.next_model().is_none());
    }

    fn model_with_effort(id: &str, name: &str, effort: &str) -> acp::ModelInfo {
        acp::ModelInfo::new(acp::ModelId::new(Arc::from(id)), name.to_string()).meta(
            serde_json::json!({
                "supportsReasoningEffort": true,
                "reasoningEffort": effort,
            })
            .as_object()
            .cloned(),
        )
    }

    #[test]
    fn update_catalog_preserves_user_effort_when_model_unchanged() {
        let id = acp::ModelId::new(Arc::from("grok-build"));
        let mut state = ModelState::default();
        state.available.insert(
            id.clone(),
            model_with_effort("grok-build", "Grok Build", "high"),
        );
        state.set_current(id.clone(), Some(ReasoningEffort::Xhigh));
        assert_eq!(state.reasoning_effort, Some(ReasoningEffort::Xhigh));

        // The broadcast carries the model's static default (high) for the same model.
        let mut refreshed = IndexMap::new();
        refreshed.insert(
            id.clone(),
            model_with_effort("grok-build", "Grok Build", "high"),
        );
        state.update_catalog(refreshed, Some(id.clone()));

        assert_eq!(
            state.reasoning_effort,
            Some(ReasoningEffort::Xhigh),
            "catalog refresh must not clobber a user-set per-session effort"
        );
    }

    #[test]
    fn update_catalog_rederives_effort_when_current_model_changes() {
        let id_a = acp::ModelId::new(Arc::from("model-a"));
        let mut state = ModelState::default();
        state.available.insert(
            id_a.clone(),
            model_with_effort("model-a", "Model A", "high"),
        );
        state.set_current(id_a.clone(), Some(ReasoningEffort::Xhigh));

        // Refresh drops model-a; fall back to model-b whose default is low.
        let id_b = acp::ModelId::new(Arc::from("model-b"));
        let mut refreshed = IndexMap::new();
        refreshed.insert(id_b.clone(), model_with_effort("model-b", "Model B", "low"));
        state.update_catalog(refreshed, Some(id_b.clone()));

        assert_eq!(state.current, Some(id_b));
        assert_eq!(state.reasoning_effort, Some(ReasoningEffort::Low));
    }

    fn state_with_meta(meta: Option<serde_json::Value>) -> ModelState {
        let id = acp::ModelId::new(Arc::from("m"));
        let mut state = ModelState::default();
        state.available.insert(
            id.clone(),
            acp::ModelInfo::new(id.clone(), "M".to_string())
                .meta(meta.and_then(|v| v.as_object().cloned())),
        );
        state.set_current(id, None);
        state
    }

    #[test]
    fn accepts_images_defaults_true_when_meta_absent() {
        // No current model, empty meta, and a meta without the key all default
        // permissive — correct today and a no-op until the server populates it.
        assert!(ModelState::default().current_model_accepts_images());
        assert!(state_with_meta(None).current_model_accepts_images());
        assert!(
            state_with_meta(Some(serde_json::json!({ "totalContextTokens": 256000 })))
                .current_model_accepts_images()
        );
    }

    #[test]
    fn reasoning_effort_options_renders_server_list() {
        let state = state_with_meta(Some(serde_json::json!({
            "supportsReasoningEffort": true,
            "reasoningEfforts": [
                { "id": "balanced", "value": "medium", "label": "Balanced" },
                { "id": "deep", "value": "xhigh", "label": "Deep", "description": "Max" },
            ],
        })));
        let opts = state.reasoning_effort_options();
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0].label, "Balanced");
        assert_eq!(opts[0].value, ReasoningEffort::Medium);
        assert_eq!(opts[1].id, "deep");
        assert_eq!(opts[1].description.as_deref(), Some("Max"));
    }

    #[test]
    fn reasoning_effort_options_gate_first_empty_when_unsupported() {
        // No current model → empty.
        assert!(ModelState::default().reasoning_effort_options().is_empty());
        // Current model that does not support effort → empty (even with a list).
        let state = state_with_meta(Some(serde_json::json!({
            "supportsReasoningEffort": false,
            "reasoningEffortSelection": "unsupported",
            "reasoningEfforts": [{ "value": "high" }],
        })));
        assert!(state.reasoning_effort_options().is_empty());
    }

    #[test]
    fn reasoning_effort_options_falls_back_to_builtin_menu() {
        // Supported but no server list → today's four-row built-in menu.
        let state = state_with_meta(Some(serde_json::json!({
            "supportsReasoningEffort": true,
        })));
        let ids: Vec<_> = state
            .reasoning_effort_options()
            .into_iter()
            .map(|o| o.id)
            .collect();
        assert_eq!(ids, ["xhigh", "high", "medium", "low"]);
    }

    #[test]
    fn reasoning_effort_options_falls_back_when_list_present_but_unusable() {
        // Matches the shell picker: an explicit empty list, and a list where every
        // entry skip-invalidated under version skew, both fall back to the built-in
        // menu rather than silently vanishing.
        for meta in [
            serde_json::json!({ "supportsReasoningEffort": true, "reasoningEfforts": [] }),
            serde_json::json!({
                "supportsReasoningEffort": true,
                "reasoningEfforts": [{ "value": "quantum" }],
            }),
        ] {
            let ids: Vec<_> = state_with_meta(Some(meta.clone()))
                .reasoning_effort_options()
                .into_iter()
                .map(|o| o.id)
                .collect();
            assert_eq!(ids, ["xhigh", "high", "medium", "low"], "for meta {meta}");
        }
    }

    #[test]
    fn resolve_effort_token_maps_remap_id_to_canonical_value() {
        let state = state_with_meta(Some(serde_json::json!({
            "supportsReasoningEffort": true,
            "reasoningEfforts": [
                { "id": "deep", "value": "xhigh", "label": "Deep" },
                { "id": "high", "value": "high", "label": "High" },
            ],
        })));
        // Design-2 remap: the typed id resolves to its canonical wire value.
        assert_eq!(
            state.resolve_effort_token("deep"),
            Some(ReasoningEffort::Xhigh)
        );
        assert_eq!(
            state.resolve_effort_token("DEEP"),
            Some(ReasoningEffort::Xhigh)
        );
        // Canonical level offered by the menu is accepted by value.
        assert_eq!(
            state.resolve_effort_token("high"),
            Some(ReasoningEffort::High)
        );
        // Levels the model does not offer (none/minimal on 4.5-style menus)
        // are rejected — better than a server-side 400.
        assert!(state.resolve_effort_token("minimal").is_none());
        assert!(state.resolve_effort_token("none").is_none());
        assert!(state.resolve_effort_token("bogus").is_none());
    }

    #[test]
    fn resolve_effort_token_accepts_none_only_when_menu_offers_it() {
        let with_none = state_with_meta(Some(serde_json::json!({
            "supportsReasoningEffort": true,
            "reasoningEfforts": [
                { "value": "none", "label": "None", "default": true },
                { "value": "high", "label": "High" },
            ],
        })));
        assert_eq!(
            with_none.resolve_effort_token("none"),
            Some(ReasoningEffort::None)
        );

        let without_none = state_with_meta(Some(serde_json::json!({
            "supportsReasoningEffort": true,
            "reasoningEfforts": [
                { "value": "high", "label": "High", "default": true },
                { "value": "low", "label": "Low" },
            ],
        })));
        assert!(without_none.resolve_effort_token("none").is_none());
        let err = without_none
            .resolve_effort_for_model(without_none.current.as_ref().unwrap(), "none")
            .unwrap_err();
        assert_eq!(
            err,
            EffortTokenError::UnknownToken {
                token: "none".to_string(),
                offered: vec!["high".to_string(), "low".to_string()],
            }
        );
        // Error copy must list only this model's options — never hardcode
        // none/minimal/… as offered values (the rejected token may still appear
        // quoted in "unknown effort level '…'").
        let msg = err.message();
        assert!(msg.contains("use one of: high, low"), "msg={msg}");
        let offered_half = msg
            .split_once("; ")
            .map(|(_, rest)| rest)
            .expect("message should have '; ' separator");
        assert!(
            !offered_half.contains("none"),
            "must not advertise blocked level: {msg}"
        );
        assert!(
            !offered_half.contains("minimal"),
            "must not advertise blocked level: {msg}"
        );
        assert!(
            !msg.contains("unset"),
            "unset is log-only, not a user token: {msg}"
        );
    }

    #[test]
    fn resolve_effort_token_legacy_menu_rejects_none() {
        // supportsReasoningEffort without a server list → built-in low..xhigh.
        let state = state_with_meta(Some(serde_json::json!({
            "supportsReasoningEffort": true,
        })));
        assert!(state.resolve_effort_token("none").is_none());
        assert!(state.resolve_effort_token("minimal").is_none());
        assert_eq!(
            state.resolve_effort_token("low"),
            Some(ReasoningEffort::Low)
        );
    }

    #[test]
    fn accepts_images_honors_explicit_meta() {
        assert!(
            !state_with_meta(Some(serde_json::json!({ "acceptsImages": false })))
                .current_model_accepts_images()
        );
        assert!(
            state_with_meta(Some(serde_json::json!({ "acceptsImages": true })))
                .current_model_accepts_images()
        );
        // inputModalities array form.
        assert!(
            state_with_meta(Some(
                serde_json::json!({ "inputModalities": ["text", "image"] })
            ))
            .current_model_accepts_images()
        );
        assert!(
            !state_with_meta(Some(serde_json::json!({ "inputModalities": ["text"] })))
                .current_model_accepts_images()
        );
    }

    // ── ReasoningEffortSelection state tests ─────────────────────────────────

    #[test]
    fn reasoning_effort_selection_unknown_accepts_canonical_token() {
        // Unknown: no menu, but explicit canonical tokens are wire-compatible
        let state = state_with_meta(Some(serde_json::json!({})));
        assert_eq!(
            state.reasoning_effort_selection,
            ReasoningEffortSelection::Unknown
        );
        // Unknown still accepts canonical tokens
        assert_eq!(
            state.resolve_effort_token("high"),
            Some(ReasoningEffort::High)
        );
        assert_eq!(
            state.resolve_effort_token("max"),
            Some(ReasoningEffort::Max)
        );
    }

    #[test]
    fn reasoning_effort_selection_unsupported_errors() {
        // Unsupported: model explicitly does not support
        let state = state_with_meta(Some(serde_json::json!({
            "supportsReasoningEffort": false,
        })));
        assert_eq!(
            state.reasoning_effort_selection,
            ReasoningEffortSelection::Unsupported
        );
        assert!(state.reasoning_effort_options().is_empty());
        // Unsupported should error when trying to resolve effort
        let err = state.resolve_effort_for_model(state.current.as_ref().unwrap(), "high");
        assert!(matches!(err, Err(EffortTokenError::Unsupported)));
    }

    #[test]
    fn reasoning_effort_selection_legacy_fallback_shows_builtin_menu() {
        // LegacyFallback: supports=true but no reasoningEfforts list
        let state = state_with_meta(Some(serde_json::json!({
            "supportsReasoningEffort": true,
        })));
        assert_eq!(
            state.reasoning_effort_selection,
            ReasoningEffortSelection::LegacyFallback
        );
        let ids: Vec<_> = state
            .reasoning_effort_options()
            .into_iter()
            .map(|o| o.id)
            .collect();
        // Legacy menu: xhigh, high, medium, low (no none/minimal)
        assert_eq!(ids, ["xhigh", "high", "medium", "low"]);
    }

    #[test]
    fn reasoning_effort_selection_exact_with_valid_menu() {
        // Exact: supports=true with explicit reasoningEfforts list
        let state = state_with_meta(Some(serde_json::json!({
            "supportsReasoningEffort": true,
            "reasoningEfforts": [
                { "id": "deep", "value": "xhigh", "label": "Deep" },
                { "id": "balanced", "value": "medium", "label": "Balanced" },
            ],
        })));
        assert_eq!(
            state.reasoning_effort_selection,
            ReasoningEffortSelection::Exact
        );
        let opts = state.reasoning_effort_options();
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0].id, "deep");
        assert_eq!(opts[0].value, ReasoningEffort::Xhigh);
        assert_eq!(opts[1].id, "balanced");
        assert_eq!(opts[1].value, ReasoningEffort::Medium);
    }

    #[test]
    fn reasoning_effort_selection_exact_present_but_unusable_fails_closed() {
        for meta in [
            serde_json::json!({
                "supportsReasoningEffort": true,
                "reasoningEffortSelection": "exact",
                "reasoningEfforts": []
            }),
            serde_json::json!({
                "supportsReasoningEffort": true,
                "reasoningEffortSelection": "exact",
                "reasoningEfforts": [{ "value": "quantum" }],
            }),
        ] {
            let state = state_with_meta(Some(meta.clone()));
            assert_eq!(
                state.reasoning_effort_selection,
                ReasoningEffortSelection::Exact,
                "for meta {meta}"
            );
            assert!(state.reasoning_effort_options().is_empty());
            assert!(state.resolve_effort_token("high").is_none());
        }
    }

    #[test]
    fn reasoning_effort_selection_unrestricted_shows_all_canonical() {
        // Unrestricted: all canonical values in strongest-first order
        // This requires the ACP meta to explicitly have reasoningEffortSelection: "unrestricted"
        let state = state_with_meta(Some(serde_json::json!({
            "supportsReasoningEffort": true,
            "reasoningEffortSelection": "unrestricted",
        })));
        assert_eq!(
            state.reasoning_effort_selection,
            ReasoningEffortSelection::Unrestricted
        );
        let opts = state.reasoning_effort_options();
        // Unrestricted: max, xhigh, high, medium, low, minimal, none
        assert_eq!(opts.len(), 7);
        assert_eq!(opts[0].value, ReasoningEffort::Max);
        assert_eq!(opts[1].value, ReasoningEffort::Xhigh);
        assert_eq!(opts[2].value, ReasoningEffort::High);
        assert_eq!(opts[3].value, ReasoningEffort::Medium);
        assert_eq!(opts[4].value, ReasoningEffort::Low);
        assert_eq!(opts[5].value, ReasoningEffort::Minimal);
        assert_eq!(opts[6].value, ReasoningEffort::None);
    }

    #[test]
    fn reasoning_effort_selection_unknown_explicit_canonical_no_menu() {
        // Unknown with explicit canonical token should work
        let state = state_with_meta(Some(serde_json::json!({})));
        // No menu shown
        assert!(state.reasoning_effort_options().is_empty());
        // But explicit canonical tokens are accepted
        assert_eq!(
            state.resolve_effort_token("max"),
            Some(ReasoningEffort::Max)
        );
        assert_eq!(
            state.resolve_effort_token("xhigh"),
            Some(ReasoningEffort::Xhigh)
        );
        assert_eq!(
            state.resolve_effort_token("high"),
            Some(ReasoningEffort::High)
        );
    }

    #[test]
    fn reasoning_effort_selection_unknown_invalid_token_errors() {
        // Unknown with invalid token should error
        let state = state_with_meta(Some(serde_json::json!({})));
        let err = state.resolve_effort_for_model(state.current.as_ref().unwrap(), "bogus");
        assert!(err.is_err());
        let err = err.unwrap_err();
        assert!(matches!(err, EffortTokenError::UnknownToken { .. }));
    }

    #[test]
    fn reasoning_effort_selection_exact_menu_id_only() {
        // Exact: only listed option IDs and canonical values are accepted
        let state = state_with_meta(Some(serde_json::json!({
            "supportsReasoningEffort": true,
            "reasoningEfforts": [
                { "id": "deep", "value": "xhigh", "label": "Deep" },
            ],
        })));
        // Menu id works
        assert_eq!(
            state.resolve_effort_token("deep"),
            Some(ReasoningEffort::Xhigh)
        );
        // Canonical value in menu works
        assert_eq!(
            state.resolve_effort_token("xhigh"),
            Some(ReasoningEffort::Xhigh)
        );
        // Canonical value NOT in menu fails
        assert!(state.resolve_effort_token("high").is_none());
        assert!(state.resolve_effort_token("medium").is_none());
        assert!(state.resolve_effort_token("low").is_none());
        assert!(state.resolve_effort_token("none").is_none());
        assert!(state.resolve_effort_token("minimal").is_none());
    }

    #[test]
    fn reasoning_effort_selection_fallback_consistency() {
        // LegacyFallback should consistently reject none/minimal
        let state = state_with_meta(Some(serde_json::json!({
            "supportsReasoningEffort": true,
        })));
        assert!(state.resolve_effort_token("none").is_none());
        assert!(state.resolve_effort_token("minimal").is_none());
        // But accepts the legacy ladder
        assert_eq!(
            state.resolve_effort_token("low"),
            Some(ReasoningEffort::Low)
        );
        assert_eq!(
            state.resolve_effort_token("medium"),
            Some(ReasoningEffort::Medium)
        );
        assert_eq!(
            state.resolve_effort_token("high"),
            Some(ReasoningEffort::High)
        );
        assert_eq!(
            state.resolve_effort_token("xhigh"),
            Some(ReasoningEffort::Xhigh)
        );
    }

    fn sibling_openai_state() -> ModelState {
        let mut state = ModelState::default();
        let home = acp::ModelId::new(Arc::from("openai:gpt-4o"));
        let work = acp::ModelId::new(Arc::from("openai_work:gpt-4o"));
        state.available.insert(
            home.clone(),
            acp::ModelInfo::new(home.clone(), "GPT-4o (openai)".to_string()).meta(
                serde_json::json!({
                    "providerInstanceId": "openai",
                    "providerKind": "openai",
                    "upstreamModelId": "gpt-4o",
                    "canonicalSelectionId": "openai:gpt-4o",
                })
                .as_object()
                .cloned(),
            ),
        );
        state.available.insert(
            work.clone(),
            acp::ModelInfo::new(work.clone(), "GPT-4o (openai_work)".to_string()).meta(
                serde_json::json!({
                    "providerInstanceId": "openai_work",
                    "providerKind": "openai",
                    "upstreamModelId": "gpt-4o",
                    "canonicalSelectionId": "openai_work:gpt-4o",
                })
                .as_object()
                .cloned(),
            ),
        );
        state.current = Some(home);
        state.catalog_generation = 3;
        state
    }

    #[test]
    fn resolve_rejects_ambiguous_upstream_sibling_labels() {
        let state = sibling_openai_state();
        // Exact canonical ids resolve.
        assert_eq!(
            state
                .resolve_by_name_or_id_detailed("openai:gpt-4o")
                .ok()
                .as_ref()
                .map(|id| id.0.as_ref()),
            Some("openai:gpt-4o")
        );
        assert_eq!(
            state
                .resolve_by_name_or_id_detailed("openai_work:gpt-4o")
                .ok()
                .as_ref()
                .map(|id| id.0.as_ref()),
            Some("openai_work:gpt-4o")
        );
        // Bare upstream / shared bare name is ambiguous — never silent pick.
        match state.resolve_by_name_or_id_detailed("gpt-4o") {
            ModelResolveResult::Ambiguous { candidates, .. } => {
                let ids: Vec<&str> = candidates.iter().map(|c| c.0.as_ref()).collect();
                assert!(ids.contains(&"openai:gpt-4o"));
                assert!(ids.contains(&"openai_work:gpt-4o"));
            }
            other => panic!("expected Ambiguous, got {other:?}"),
        }
        assert!(state.resolve_by_name_or_id("gpt-4o").is_none());
    }

    #[test]
    fn update_catalog_versioned_preserves_exact_selection_and_generation() {
        let mut state = sibling_openai_state();
        let work = acp::ModelId::new(Arc::from("openai_work:gpt-4o"));
        state.set_current(work.clone(), None);
        assert_eq!(state.catalog_generation, 3);

        // Refresh keeps work account by exact canonical id.
        let mut refreshed = IndexMap::new();
        refreshed.insert(
            work.clone(),
            acp::ModelInfo::new(work.clone(), "GPT-4o (openai_work)".to_string()),
        );
        let home = acp::ModelId::new(Arc::from("openai:gpt-4o"));
        refreshed.insert(
            home.clone(),
            acp::ModelInfo::new(home, "GPT-4o (openai)".to_string()),
        );
        assert!(state.update_catalog_versioned(refreshed.clone(), Some(work.clone()), Some(7)));
        assert_eq!(
            state.current.as_ref().map(|id| id.0.as_ref()),
            Some("openai_work:gpt-4o")
        );
        assert_eq!(state.catalog_generation, 7);

        // Stale lower generation rejects content entirely (no mix under gen 7).
        let mut stale = IndexMap::new();
        stale.insert(
            work.clone(),
            acp::ModelInfo::new(work.clone(), "STALE".to_string()),
        );
        assert!(!state.update_catalog_versioned(stale, Some(work.clone()), Some(2)));
        assert_eq!(state.catalog_generation, 7);
        assert_ne!(
            state.available.get(&work).map(|i| i.name.as_str()),
            Some("STALE")
        );

        // Equal generation is a no-op.
        assert!(!state.update_catalog_versioned(refreshed, Some(work), Some(7)));
        assert_eq!(state.catalog_generation, 7);
    }

    #[test]
    fn update_catalog_removed_incarnation_falls_back_without_sibling_steal_on_missing_key() {
        let mut state = sibling_openai_state();
        let work = acp::ModelId::new(Arc::from("openai_work:gpt-4o"));
        state.set_current(work.clone(), None);

        // Work account removed; fallback is explicit home, not silent first-key.
        let home = acp::ModelId::new(Arc::from("openai:gpt-4o"));
        let mut refreshed = IndexMap::new();
        refreshed.insert(
            home.clone(),
            acp::ModelInfo::new(home.clone(), "GPT-4o (openai)".to_string()),
        );
        state.update_catalog_versioned(refreshed, Some(home.clone()), Some(8));
        assert_eq!(state.current, Some(home));
        assert!(!state.available.contains_key(&work));
    }

    #[test]
    fn from_session_model_state_reads_catalog_generation() {
        let id = acp::ModelId::new(Arc::from("m"));
        let mut meta = acp::Meta::new();
        meta.insert(
            "catalogGeneration".into(),
            serde_json::Value::Number(42.into()),
        );
        let sms =
            acp::SessionModelState::new(id.clone(), vec![acp::ModelInfo::new(id, "M".to_string())])
                .meta(Some(meta));
        let state = ModelState::from(Some(sms));
        assert_eq!(state.catalog_generation, 42);
    }
}
