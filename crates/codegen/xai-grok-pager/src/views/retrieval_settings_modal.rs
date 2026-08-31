//! Retrieval graph settings modal (`/retrieval-settings`).
//!
//! Shell-authoritative: the pager never reads/writes raw TOML. All state is
//! generation-tagged; dirty editors enter conflict on multi-client update;
//! clean list/editor auto-reloads. Secret-free throughout.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::input::line_editor::LineEditor;
use crate::theme::Theme;
use crate::views::modal_window::{
    self, ModalSizing, ModalWindowConfig, ModalWindowState, Shortcut,
};

use xai_grok_shell::provider_registry::management::dto::RegistryGeneration;
use xai_grok_shell::retrieval_config::dto::{
    EmbeddingModelDto, MemoryReindexImpact, RerankerModelDto, RetrievalConflictInfo,
    RetrievalGraphSnapshot, RetrievalMutationResult, RetrievalPreviewResult, RetrievalProfileDto,
};
use xai_grok_shell::retrieval_config::{
    AgentPrimeConfig, EmbeddingEncoding, EmbeddingModelConfig, EmbeddingProtocol, PrimeConfig,
    RerankerModelConfig, RerankerProtocol, RetrievalFallbackStrategy, RetrievalProfileConfig,
    SkillPrimeConfig,
};

/// Typed pages covering every PR15-owned field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetrievalPage {
    EmbeddingModels,
    Rerankers,
    Profiles,
    Prime,
    Memory,
    Validate,
}

impl RetrievalPage {
    pub const ALL: [Self; 6] = [
        Self::EmbeddingModels,
        Self::Rerankers,
        Self::Profiles,
        Self::Prime,
        Self::Memory,
        Self::Validate,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::EmbeddingModels => "Embeddings",
            Self::Rerankers => "Rerankers",
            Self::Profiles => "Profiles",
            Self::Prime => "Prime",
            Self::Memory => "Memory",
            Self::Validate => "Validate",
        }
    }

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|p| *p == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let i = Self::ALL.iter().position(|p| *p == self).unwrap_or(0);
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// Safe reducer commands (no secrets).
///
/// Every mutation/preview carries the modal-loaded `expected_generation` and a
/// fresh `operation_id` minted at the UI boundary. Effects must not replace
/// generation with a live `current_generation()` read.
#[derive(Debug, Clone, PartialEq)]
pub enum RetrievalCommand {
    Reload,
    /// Browse validate/reload only (no durable write, no generation churn).
    ValidateAndReload,
    ValidatePreview {
        kind: String,
        id: String,
        expected_generation: RegistryGeneration,
        operation_id: String,
    },
    UpsertEmbedding {
        id: String,
        config: EmbeddingModelConfig,
        expected_generation: RegistryGeneration,
        confirm_memory_reindex: bool,
        operation_id: String,
    },
    UpsertReranker {
        id: String,
        config: RerankerModelConfig,
        expected_generation: RegistryGeneration,
        confirm_memory_reindex: bool,
        operation_id: String,
    },
    UpsertProfile {
        id: String,
        config: RetrievalProfileConfig,
        expected_generation: RegistryGeneration,
        confirm_memory_reindex: bool,
        operation_id: String,
    },
    CloneEntity {
        kind: String,
        source_id: String,
        new_id: String,
        expected_generation: RegistryGeneration,
        confirm_memory_reindex: bool,
        operation_id: String,
    },
    DeleteEntity {
        kind: String,
        id: String,
        expected_generation: RegistryGeneration,
        confirm_memory_reindex: bool,
        operation_id: String,
    },
    Reorder {
        kind: String,
        ordered_ids: Vec<String>,
        expected_generation: RegistryGeneration,
        confirm_memory_reindex: bool,
        operation_id: String,
    },
    SavePrime {
        prime: PrimeConfig,
        expected_generation: RegistryGeneration,
        confirm_memory_reindex: bool,
        operation_id: String,
    },
    SaveMemoryProfile {
        profile: Option<String>,
        expected_generation: RegistryGeneration,
        confirm_memory_reindex: bool,
        operation_id: String,
    },
    /// Retry the exact pending draft mutation with confirm_memory_reindex=true.
    ConfirmMemoryReindex,
    PrimeIndexBackfill {
        collection: String,
        confirm: bool,
    },
    PrimeIndexRebuild {
        collection: String,
        confirm: bool,
    },
    PrimeIndexCancel,
    DismissConflictReload,
    DismissConflictKeepDraft,
}

impl RetrievalCommand {
    /// Set confirm_memory_reindex on mutation variants (for reindex retry).
    pub fn with_reindex_confirm(self, confirm: bool) -> Self {
        match self {
            Self::UpsertEmbedding {
                id,
                config,
                expected_generation,
                operation_id,
                ..
            } => Self::UpsertEmbedding {
                id,
                config,
                expected_generation,
                confirm_memory_reindex: confirm,
                operation_id,
            },
            Self::UpsertReranker {
                id,
                config,
                expected_generation,
                operation_id,
                ..
            } => Self::UpsertReranker {
                id,
                config,
                expected_generation,
                confirm_memory_reindex: confirm,
                operation_id,
            },
            Self::UpsertProfile {
                id,
                config,
                expected_generation,
                operation_id,
                ..
            } => Self::UpsertProfile {
                id,
                config,
                expected_generation,
                confirm_memory_reindex: confirm,
                operation_id,
            },
            Self::CloneEntity {
                kind,
                source_id,
                new_id,
                expected_generation,
                operation_id,
                ..
            } => Self::CloneEntity {
                kind,
                source_id,
                new_id,
                expected_generation,
                confirm_memory_reindex: confirm,
                operation_id,
            },
            Self::DeleteEntity {
                kind,
                id,
                expected_generation,
                operation_id,
                ..
            } => Self::DeleteEntity {
                kind,
                id,
                expected_generation,
                confirm_memory_reindex: confirm,
                operation_id,
            },
            Self::Reorder {
                kind,
                ordered_ids,
                expected_generation,
                operation_id,
                ..
            } => Self::Reorder {
                kind,
                ordered_ids,
                expected_generation,
                confirm_memory_reindex: confirm,
                operation_id,
            },
            Self::SavePrime {
                prime,
                expected_generation,
                operation_id,
                ..
            } => Self::SavePrime {
                prime,
                expected_generation,
                confirm_memory_reindex: confirm,
                operation_id,
            },
            Self::SaveMemoryProfile {
                profile,
                expected_generation,
                operation_id,
                ..
            } => Self::SaveMemoryProfile {
                profile,
                expected_generation,
                confirm_memory_reindex: confirm,
                operation_id,
            },
            other => other,
        }
    }

    /// Replace operation_id only (preserve draft payload + expected_generation).
    pub fn with_operation_id(self, operation_id: String) -> Self {
        match self {
            Self::ValidatePreview {
                kind,
                id,
                expected_generation,
                ..
            } => Self::ValidatePreview {
                kind,
                id,
                expected_generation,
                operation_id,
            },
            Self::UpsertEmbedding {
                id,
                config,
                expected_generation,
                confirm_memory_reindex,
                ..
            } => Self::UpsertEmbedding {
                id,
                config,
                expected_generation,
                confirm_memory_reindex,
                operation_id,
            },
            Self::UpsertReranker {
                id,
                config,
                expected_generation,
                confirm_memory_reindex,
                ..
            } => Self::UpsertReranker {
                id,
                config,
                expected_generation,
                confirm_memory_reindex,
                operation_id,
            },
            Self::UpsertProfile {
                id,
                config,
                expected_generation,
                confirm_memory_reindex,
                ..
            } => Self::UpsertProfile {
                id,
                config,
                expected_generation,
                confirm_memory_reindex,
                operation_id,
            },
            Self::CloneEntity {
                kind,
                source_id,
                new_id,
                expected_generation,
                confirm_memory_reindex,
                ..
            } => Self::CloneEntity {
                kind,
                source_id,
                new_id,
                expected_generation,
                confirm_memory_reindex,
                operation_id,
            },
            Self::DeleteEntity {
                kind,
                id,
                expected_generation,
                confirm_memory_reindex,
                ..
            } => Self::DeleteEntity {
                kind,
                id,
                expected_generation,
                confirm_memory_reindex,
                operation_id,
            },
            Self::Reorder {
                kind,
                ordered_ids,
                expected_generation,
                confirm_memory_reindex,
                ..
            } => Self::Reorder {
                kind,
                ordered_ids,
                expected_generation,
                confirm_memory_reindex,
                operation_id,
            },
            Self::SavePrime {
                prime,
                expected_generation,
                confirm_memory_reindex,
                ..
            } => Self::SavePrime {
                prime,
                expected_generation,
                confirm_memory_reindex,
                operation_id,
            },
            Self::SaveMemoryProfile {
                profile,
                expected_generation,
                confirm_memory_reindex,
                ..
            } => Self::SaveMemoryProfile {
                profile,
                expected_generation,
                confirm_memory_reindex,
                operation_id,
            },
            other => other,
        }
    }

    pub fn operation_id(&self) -> Option<&str> {
        match self {
            Self::ValidatePreview { operation_id, .. }
            | Self::UpsertEmbedding { operation_id, .. }
            | Self::UpsertReranker { operation_id, .. }
            | Self::UpsertProfile { operation_id, .. }
            | Self::CloneEntity { operation_id, .. }
            | Self::DeleteEntity { operation_id, .. }
            | Self::Reorder { operation_id, .. }
            | Self::SavePrime { operation_id, .. }
            | Self::SaveMemoryProfile { operation_id, .. } => Some(operation_id.as_str()),
            _ => None,
        }
    }
}

/// Editor sub-mode for adding/editing an entity.
#[derive(Debug, Clone, PartialEq)]
pub enum RetrievalEditMode {
    Browse,
    EditFields {
        kind: String,
        id: String,
        is_new: bool,
        fields: Vec<(String, String)>,
        field_idx: usize,
        editing_value: bool,
    },
    ConfirmDelete {
        kind: String,
        id: String,
    },
    ConfirmMemoryReindex {
        impact: MemoryReindexImpact,
    },
    ConfirmPrimeRebuild {
        collection: String,
        route: String,
    },
    ConfirmPrimeBackfill {
        collection: String,
        route: String,
    },
    ClonePrompt {
        kind: String,
        source_id: String,
    },
}

/// Modal state for `/retrieval-settings`.
#[derive(Debug)]
pub struct RetrievalSettingsState {
    pub page: RetrievalPage,
    pub generation: RegistryGeneration,
    pub snapshot: Option<RetrievalGraphSnapshot>,
    pub loading: bool,
    pub error: Option<String>,
    pub status: Option<String>,
    pub selected: usize,
    pub edit: RetrievalEditMode,
    pub dirty: bool,
    pub conflict: Option<RetrievalConflictInfo>,
    pub draft_prime: PrimeConfig,
    pub draft_memory_profile: Option<String>,
    pub(crate) line_editor: LineEditor,
    pub window: ModalWindowState,
    pub last_preview: Option<RetrievalPreviewResult>,
    pub pending_operation_id: Option<String>,
    pub op_counter: u64,
    /// Exact draft mutation awaiting reindex confirmation (retry with confirm=true).
    /// Confirmation is derived solely via `with_reindex_confirm(true)` on this
    /// command — never a sticky global flag that could leak to later mutations.
    pub pending_reindex_command: Option<Box<RetrievalCommand>>,
    pub prime_index: Option<xai_grok_shell::session::prime::PrimeIndexStatus>,
}

impl Default for RetrievalSettingsState {
    fn default() -> Self {
        Self::new()
    }
}

impl RetrievalSettingsState {
    pub fn new() -> Self {
        Self {
            page: RetrievalPage::EmbeddingModels,
            generation: RegistryGeneration(0),
            snapshot: None,
            loading: true,
            error: None,
            status: None,
            selected: 0,
            edit: RetrievalEditMode::Browse,
            dirty: false,
            conflict: None,
            draft_prime: PrimeConfig::default(),
            draft_memory_profile: None,
            line_editor: LineEditor::default(),
            window: ModalWindowState::default(),
            last_preview: None,
            pending_operation_id: None,
            op_counter: 0,
            pending_reindex_command: None,
            prime_index: None,
        }
    }

    fn configured_route_for_confirm(&self) -> String {
        let raw = self
            .prime_index
            .as_ref()
            .and_then(|s| s.configured_route.clone())
            .or_else(|| {
                if self.selected == 0 {
                    self.draft_prime.skills.retrieval_profile.clone()
                } else {
                    self.draft_prime.agents.retrieval_profile.clone()
                }
            });
        display_configured_route(raw.as_deref())
    }

    fn set_prime_unavailable(&mut self) {
        let msg = PRIME_UNAVAILABLE_PROFILE.to_string();
        self.error = Some(msg.clone());
        self.status = Some(msg);
    }

    pub fn apply_prime_index_update(
        &mut self,
        update: &xai_grok_shell::session::prime::PrimeIndexUpdate,
    ) {
        if let Some(ref mut status) = self.prime_index {
            if update.generation_is_stale_vs(status.generation) {
                return;
            }
            status.generation = update.generation;
            if !update.fingerprint_short.is_empty() {
                status.fingerprint_short = update.fingerprint_short.clone();
            }
            if let Some(job) = update.sanitized_job() {
                status.job = Some(job);
            }
            status.sanitize_secrets();
        }
    }

    fn next_op_id(&mut self) -> String {
        self.op_counter = self.op_counter.saturating_add(1);
        format!("retrieval-op-{}", self.op_counter)
    }

    /// Stamp generation + op-id, set pending correlation, stash for reindex retry.
    fn stamp_mutation(&mut self, cmd: RetrievalCommand) -> RetrievalCommand {
        if let Some(op) = cmd.operation_id() {
            self.pending_operation_id = Some(op.to_owned());
        }
        // Stash for confirm-reindex retry (exact draft).
        match &cmd {
            RetrievalCommand::Reload
            | RetrievalCommand::ValidateAndReload
            | RetrievalCommand::ValidatePreview { .. }
            | RetrievalCommand::ConfirmMemoryReindex
            | RetrievalCommand::DismissConflictReload
            | RetrievalCommand::DismissConflictKeepDraft => {}
            _ => {
                self.pending_reindex_command = Some(Box::new(cmd.clone()));
            }
        }
        cmd
    }

    pub fn apply_snapshot(&mut self, snap: RetrievalGraphSnapshot) {
        self.generation = snap.generation;
        self.draft_prime = PrimeConfig {
            skills: snap.prime.skills.clone(),
            agents: snap.prime.agents.clone(),
        };
        self.draft_memory_profile = snap.memory_retrieval_profile.clone();
        self.snapshot = Some(snap);
        self.loading = false;
        self.error = None;
        self.dirty = false;
        self.conflict = None;
        self.clamp_selection();
    }

    /// Multi-client generation update.
    ///
    /// Returns `true` when the clean path should enqueue `LoadSnapshot`.
    /// Dirty/non-browse enters conflict and preserves draft (returns false).
    pub fn on_remote_generation(&mut self, live: RegistryGeneration, changed: &[String]) -> bool {
        if live.get() <= self.generation.get() {
            return false;
        }
        if self.dirty || !matches!(self.edit, RetrievalEditMode::Browse) {
            self.conflict = Some(RetrievalConflictInfo {
                client_generation: self.generation,
                live_generation: live,
                changed_fields: changed.to_vec(),
                guidance: "Another client updated the retrieval graph. Reload to discard local \
                           edits, or keep draft and re-save (stale CAS will reject)."
                    .into(),
            });
            self.status = Some("Conflict: remote graph advanced".into());
            false
        } else {
            self.loading = true;
            self.status = Some("Remote update — reloading".into());
            true
        }
    }

    /// Strict op-id match (provider Gate E): mutation completes require Some/Some equal.
    pub fn mutation_op_matches(pending: Option<&str>, echo: Option<&str>) -> bool {
        match (pending, echo) {
            (Some(p), Some(e)) if p == e => true,
            _ => false,
        }
    }

    pub fn apply_mutation_result(&mut self, result: RetrievalMutationResult) {
        // Strict: None or mismatch ⇒ discard late/async results.
        if !Self::mutation_op_matches(
            self.pending_operation_id.as_deref(),
            result.operation_id.as_deref(),
        ) {
            return;
        }
        self.pending_operation_id = None;
        if result.stale {
            // Terminal for this attempt — do not keep a half-confirmed draft.
            self.pending_reindex_command = None;
            if let Some(c) = result.conflict {
                self.conflict = Some(c);
            }
            self.status = Some(result.error.unwrap_or_else(|| "Stale generation".into()));
            return;
        }
        if !result.ok {
            if let Some(impact) = result.memory_reindex
                && impact.requires_confirmation
            {
                self.edit = RetrievalEditMode::ConfirmMemoryReindex { impact };
                // Keep pending_reindex_command for exact draft retry only.
                return;
            }
            // Validation / I/O / other non-reindex failure: clear stash so the
            // next mutation is never pre-confirmed.
            self.pending_reindex_command = None;
            self.error = result.error;
            self.status = Some("Save failed".into());
            return;
        }
        self.pending_reindex_command = None;
        if let Some(snap) = result.snapshot {
            self.apply_snapshot(snap);
        } else {
            self.generation = result.generation;
            self.dirty = false;
            self.loading = true;
        }
        self.edit = RetrievalEditMode::Browse;
        self.status = Some("Saved".into());
    }

    pub fn apply_preview(&mut self, preview: RetrievalPreviewResult) {
        if !Self::mutation_op_matches(
            self.pending_operation_id.as_deref(),
            preview.operation_id.as_deref(),
        ) {
            return;
        }
        self.pending_operation_id = None;
        self.last_preview = Some(preview);
        self.page = RetrievalPage::Validate;
    }

    fn clamp_selection(&mut self) {
        let n = self.list_len();
        if n == 0 {
            self.selected = 0;
        } else if self.selected >= n {
            self.selected = n - 1;
        }
    }

    fn list_len(&self) -> usize {
        let Some(snap) = &self.snapshot else {
            return 0;
        };
        match self.page {
            RetrievalPage::EmbeddingModels => snap.embedding_models.len(),
            RetrievalPage::Rerankers => snap.reranker_models.len(),
            RetrievalPage::Profiles => snap.retrieval_profiles.len(),
            RetrievalPage::Prime => 2,
            RetrievalPage::Memory => 1,
            RetrievalPage::Validate => {
                snap.validation_errors.len() + snap.validation_warnings.len() + 1
            }
        }
    }

    /// Conflicts and non-Browse sub-modes own Esc so chrome cannot close
    /// `/retrieval-settings` while a draft, confirm, or conflict is active
    /// (mirrors `/providers` `owns_escape`). Browse with no conflict still
    /// closes the modal via chrome `CloseRequested`.
    pub(crate) fn owns_escape(&self) -> bool {
        self.conflict.is_some() || !matches!(self.edit, RetrievalEditMode::Browse)
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<RetrievalCommand> {
        if self.conflict.is_some() {
            return self.handle_conflict_key(key);
        }
        match &self.edit {
            RetrievalEditMode::Browse => self.handle_browse_key(key),
            RetrievalEditMode::EditFields { .. } => self.handle_edit_key(key),
            RetrievalEditMode::ConfirmDelete { kind, id } => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    let kind = kind.clone();
                    let id = id.clone();
                    self.edit = RetrievalEditMode::Browse;
                    let op = self.next_op_id();
                    Some(self.stamp_mutation(RetrievalCommand::DeleteEntity {
                        kind,
                        id,
                        expected_generation: self.generation,
                        confirm_memory_reindex: false,
                        operation_id: op,
                    }))
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.edit = RetrievalEditMode::Browse;
                    None
                }
                _ => None,
            },
            RetrievalEditMode::ConfirmMemoryReindex { .. } => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.edit = RetrievalEditMode::Browse;
                    // Retry exact pending draft only — confirmation is on that
                    // command, never a sticky modal-wide flag.
                    if let Some(cmd) = self.pending_reindex_command.take() {
                        let op = self.next_op_id();
                        let confirmed = cmd.with_reindex_confirm(true).with_operation_id(op);
                        return Some(self.stamp_mutation(confirmed));
                    }
                    Some(RetrievalCommand::ConfirmMemoryReindex)
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.edit = RetrievalEditMode::Browse;
                    self.pending_reindex_command = None;
                    None
                }
                _ => None,
            },
            RetrievalEditMode::ConfirmPrimeRebuild { collection, route } => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    let collection = collection.clone();
                    let route = display_configured_route(Some(route.as_str()));
                    self.edit = RetrievalEditMode::Browse;
                    if route.is_empty() {
                        self.set_prime_unavailable();
                        None
                    } else {
                        Some(RetrievalCommand::PrimeIndexRebuild {
                            collection,
                            confirm: true,
                        })
                    }
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    let _ = route;
                    self.edit = RetrievalEditMode::Browse;
                    None
                }
                _ => None,
            },
            RetrievalEditMode::ConfirmPrimeBackfill { collection, route } => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    let collection = collection.clone();
                    let route = display_configured_route(Some(route.as_str()));
                    self.edit = RetrievalEditMode::Browse;
                    if route.is_empty() {
                        self.set_prime_unavailable();
                        None
                    } else {
                        Some(RetrievalCommand::PrimeIndexBackfill {
                            collection,
                            confirm: true,
                        })
                    }
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    let _ = route;
                    self.edit = RetrievalEditMode::Browse;
                    None
                }
                _ => None,
            },
            RetrievalEditMode::ClonePrompt { kind, source_id } => match key.code {
                KeyCode::Enter => {
                    let new_id = self.line_editor.text().trim().to_owned();
                    let kind = kind.clone();
                    let source_id = source_id.clone();
                    self.edit = RetrievalEditMode::Browse;
                    self.line_editor.reset();
                    if new_id.is_empty() {
                        self.status = Some("Clone id required".into());
                        None
                    } else {
                        let op = self.next_op_id();
                        Some(self.stamp_mutation(RetrievalCommand::CloneEntity {
                            kind,
                            source_id,
                            new_id,
                            expected_generation: self.generation,
                            confirm_memory_reindex: false,
                            operation_id: op,
                        }))
                    }
                }
                KeyCode::Esc => {
                    self.edit = RetrievalEditMode::Browse;
                    self.line_editor.reset();
                    None
                }
                _ => {
                    let _ = self.line_editor.handle_key(&key);
                    None
                }
            },
        }
    }

    fn handle_conflict_key(&mut self, key: KeyEvent) -> Option<RetrievalCommand> {
        match key.code {
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.conflict = None;
                self.dirty = false;
                self.edit = RetrievalEditMode::Browse;
                Some(RetrievalCommand::DismissConflictReload)
            }
            KeyCode::Char('k') | KeyCode::Char('K') | KeyCode::Esc => {
                self.conflict = None;
                Some(RetrievalCommand::DismissConflictKeepDraft)
            }
            _ => None,
        }
    }

    fn handle_browse_key(&mut self, key: KeyEvent) -> Option<RetrievalCommand> {
        match key.code {
            KeyCode::Tab | KeyCode::Right => {
                self.page = self.page.next();
                self.selected = 0;
                None
            }
            KeyCode::BackTab | KeyCode::Left => {
                self.page = self.page.prev();
                self.selected = 0;
                None
            }
            KeyCode::Char(c @ '1'..='6') => {
                let idx = (c as u8 - b'1') as usize;
                if let Some(p) = RetrievalPage::ALL.get(idx) {
                    self.page = *p;
                    self.selected = 0;
                }
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let n = self.list_len();
                if n > 0 && self.selected + 1 < n {
                    self.selected += 1;
                }
                None
            }
            KeyCode::Char('b') if self.page == RetrievalPage::Prime => {
                let collection = if self.selected == 0 {
                    "skills"
                } else {
                    "agents"
                };
                let route = self.configured_route_for_confirm();
                if route.is_empty() {
                    self.set_prime_unavailable();
                    None
                } else {
                    self.edit = RetrievalEditMode::ConfirmPrimeBackfill {
                        collection: collection.into(),
                        route,
                    };
                    None
                }
            }
            KeyCode::Char('u') if self.page == RetrievalPage::Prime => {
                let collection = if self.selected == 0 {
                    "skills"
                } else {
                    "agents"
                };
                let route = self.configured_route_for_confirm();
                if route.is_empty() {
                    self.set_prime_unavailable();
                    None
                } else {
                    self.edit = RetrievalEditMode::ConfirmPrimeRebuild {
                        collection: collection.into(),
                        route,
                    };
                    None
                }
            }
            KeyCode::Char('x') if self.page == RetrievalPage::Prime => {
                Some(RetrievalCommand::PrimeIndexCancel)
            }
            KeyCode::Char('r') => Some(RetrievalCommand::Reload),
            // Browse "s" validates/reloads only — no durable rewrite or gen churn.
            KeyCode::Char('s') => Some(RetrievalCommand::ValidateAndReload),
            KeyCode::Char('v') => {
                let (kind, id) = self.selected_entity();
                let op = self.next_op_id();
                let cmd = RetrievalCommand::ValidatePreview {
                    kind,
                    id,
                    expected_generation: self.generation,
                    operation_id: op,
                };
                Some(self.stamp_mutation(cmd))
            }
            KeyCode::Char('a') => {
                self.begin_add();
                None
            }
            KeyCode::Char('e') | KeyCode::Enter => {
                self.begin_edit_selected();
                None
            }
            KeyCode::Char('y') => {
                let (kind, id) = self.selected_entity();
                if !id.is_empty() && matches!(kind.as_str(), "embedding" | "reranker" | "profile") {
                    self.edit = RetrievalEditMode::ClonePrompt {
                        kind,
                        source_id: id,
                    };
                    self.line_editor.reset();
                }
                None
            }
            KeyCode::Char('d') | KeyCode::Char('x') => {
                let (kind, id) = self.selected_entity();
                if !id.is_empty() && matches!(kind.as_str(), "embedding" | "reranker" | "profile") {
                    self.edit = RetrievalEditMode::ConfirmDelete { kind, id };
                }
                None
            }
            // delete confirm handled above in ConfirmDelete arm
            KeyCode::Char('K') => self.reorder_selected(-1),
            KeyCode::Char('J') => self.reorder_selected(1),
            _ => None,
        }
    }

    fn reorder_selected(&mut self, delta: i32) -> Option<RetrievalCommand> {
        let kind = match self.page {
            RetrievalPage::EmbeddingModels => "embedding",
            RetrievalPage::Rerankers => "reranker",
            RetrievalPage::Profiles => "profile",
            _ => return None,
        };
        let Some(snap) = &self.snapshot else {
            return None;
        };
        let mut ids: Vec<String> = match self.page {
            RetrievalPage::EmbeddingModels => {
                snap.embedding_models.iter().map(|e| e.id.clone()).collect()
            }
            RetrievalPage::Rerankers => snap.reranker_models.iter().map(|e| e.id.clone()).collect(),
            RetrievalPage::Profiles => snap
                .retrieval_profiles
                .iter()
                .map(|e| e.id.clone())
                .collect(),
            _ => return None,
        };
        if ids.is_empty() {
            return None;
        }
        let i = self.selected.min(ids.len() - 1);
        let j = if delta < 0 {
            i.saturating_sub(1)
        } else {
            (i + 1).min(ids.len() - 1)
        };
        if i == j {
            return None;
        }
        ids.swap(i, j);
        self.selected = j;
        self.dirty = true;
        let op = self.next_op_id();
        Some(self.stamp_mutation(RetrievalCommand::Reorder {
            kind: kind.into(),
            ordered_ids: ids,
            expected_generation: self.generation,
            confirm_memory_reindex: false,
            operation_id: op,
        }))
    }

    fn selected_entity(&self) -> (String, String) {
        let Some(snap) = &self.snapshot else {
            return ("validate".into(), String::new());
        };
        match self.page {
            RetrievalPage::EmbeddingModels => snap
                .embedding_models
                .get(self.selected)
                .map(|e| ("embedding".into(), e.id.clone()))
                .unwrap_or_else(|| ("embedding".into(), String::new())),
            RetrievalPage::Rerankers => snap
                .reranker_models
                .get(self.selected)
                .map(|e| ("reranker".into(), e.id.clone()))
                .unwrap_or_else(|| ("reranker".into(), String::new())),
            RetrievalPage::Profiles => snap
                .retrieval_profiles
                .get(self.selected)
                .map(|e| ("profile".into(), e.id.clone()))
                .unwrap_or_else(|| ("profile".into(), String::new())),
            RetrievalPage::Prime => (
                "prime".into(),
                if self.selected == 0 {
                    "skills"
                } else {
                    "agents"
                }
                .into(),
            ),
            RetrievalPage::Memory => ("memory".into(), "selection".into()),
            RetrievalPage::Validate => ("validate".into(), String::new()),
        }
    }

    fn begin_add(&mut self) {
        let (kind, id, is_new, fields) = match self.page {
            RetrievalPage::EmbeddingModels => (
                "embedding".to_string(),
                String::new(),
                true,
                default_embedding_fields(None),
            ),
            RetrievalPage::Rerankers => (
                "reranker".to_string(),
                String::new(),
                true,
                default_reranker_fields(None),
            ),
            RetrievalPage::Profiles => (
                "profile".to_string(),
                String::new(),
                true,
                default_profile_fields(None),
            ),
            RetrievalPage::Prime => (
                "prime".to_string(),
                if self.selected == 0 {
                    "skills".to_string()
                } else {
                    "agents".to_string()
                },
                false,
                prime_fields(&self.draft_prime, self.selected == 0),
            ),
            RetrievalPage::Memory => (
                "memory".to_string(),
                "selection".to_string(),
                false,
                vec![(
                    "retrieval_profile".to_string(),
                    self.draft_memory_profile.clone().unwrap_or_default(),
                )],
            ),
            RetrievalPage::Validate => return,
        };
        self.start_field_edit(kind, id, is_new, fields);
    }

    /// Open the field editor with value editing armed on the first editable
    /// row, so typing works immediately after `a`/Enter without a hidden
    /// extra `e` step (the old flow read as "nothing happens").
    fn start_field_edit(
        &mut self,
        kind: String,
        id: String,
        is_new: bool,
        fields: Vec<(String, String)>,
    ) {
        let idx = first_editable_field_idx(&fields);
        self.line_editor.set_text(&fields[idx].1);
        self.edit = RetrievalEditMode::EditFields {
            kind,
            id,
            is_new,
            fields,
            field_idx: idx,
            editing_value: true,
        };
    }

    fn begin_edit_selected(&mut self) {
        let Some(snap) = &self.snapshot else {
            return;
        };
        match self.page {
            RetrievalPage::EmbeddingModels => {
                let e = snap.embedding_models.get(self.selected).cloned();
                if let Some(e) = e {
                    self.start_field_edit(
                        "embedding".into(),
                        e.id.clone(),
                        false,
                        default_embedding_fields(Some(&e)),
                    );
                }
            }
            RetrievalPage::Rerankers => {
                let e = snap.reranker_models.get(self.selected).cloned();
                if let Some(e) = e {
                    self.start_field_edit(
                        "reranker".into(),
                        e.id.clone(),
                        false,
                        default_reranker_fields(Some(&e)),
                    );
                }
            }
            RetrievalPage::Profiles => {
                let e = snap.retrieval_profiles.get(self.selected).cloned();
                if let Some(e) = e {
                    self.start_field_edit(
                        "profile".into(),
                        e.id.clone(),
                        false,
                        default_profile_fields(Some(&e)),
                    );
                }
            }
            RetrievalPage::Prime | RetrievalPage::Memory => self.begin_add(),
            RetrievalPage::Validate => {}
        }
    }

    fn handle_edit_key(&mut self, key: KeyEvent) -> Option<RetrievalCommand> {
        let editing = matches!(
            self.edit,
            RetrievalEditMode::EditFields {
                editing_value: true,
                ..
            }
        );
        if editing {
            match key.code {
                KeyCode::Enter => {
                    if let RetrievalEditMode::EditFields {
                        fields,
                        field_idx,
                        editing_value,
                        ..
                    } = &mut self.edit
                    {
                        // Defensive: fixed rows are display-only, never written to.
                        let on_fixed = fields
                            .get(*field_idx)
                            .is_some_and(|(k, _)| is_fixed_field_label(k));
                        if on_fixed {
                            *editing_value = false;
                            self.line_editor.reset();
                            return None;
                        }
                        if let Some(f) = fields.get_mut(*field_idx) {
                            f.1 = self.line_editor.text().to_owned();
                        }
                        // Wizard flow: advance to the next editable field and
                        // keep typing. On the last field this ends editing so
                        // `s` can commit.
                        let next = next_editable_field_idx(fields, *field_idx);
                        if next != *field_idx {
                            *field_idx = next;
                            self.line_editor.set_text(&fields[next].1);
                        } else {
                            *editing_value = false;
                            self.line_editor.reset();
                        }
                        self.dirty = true;
                    }
                    return None;
                }
                KeyCode::Esc => {
                    if let RetrievalEditMode::EditFields { editing_value, .. } = &mut self.edit {
                        *editing_value = false;
                    }
                    self.line_editor.reset();
                    return None;
                }
                _ => {
                    let _ = self.line_editor.handle_key(&key);
                    return None;
                }
            }
        }

        match key.code {
            KeyCode::Esc => {
                self.edit = RetrievalEditMode::Browse;
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let RetrievalEditMode::EditFields {
                    fields, field_idx, ..
                } = &mut self.edit
                {
                    *field_idx = prev_editable_field_idx(fields, *field_idx);
                }
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let RetrievalEditMode::EditFields {
                    fields, field_idx, ..
                } = &mut self.edit
                {
                    *field_idx = next_editable_field_idx(fields, *field_idx);
                }
                None
            }
            KeyCode::Enter | KeyCode::Char('e') => {
                if let RetrievalEditMode::EditFields {
                    fields,
                    field_idx,
                    editing_value,
                    ..
                } = &mut self.edit
                {
                    // Fixed enum rows are display-only — never free-form edit.
                    if let Some(f) = fields.get(*field_idx)
                        && !is_fixed_field_label(&f.0)
                    {
                        self.line_editor.set_text(&f.1);
                        *editing_value = true;
                    }
                }
                None
            }
            KeyCode::Char('s') => {
                let (kind, id, is_new, fields) = match &self.edit {
                    RetrievalEditMode::EditFields {
                        kind,
                        id,
                        is_new,
                        fields,
                        ..
                    } => (kind.clone(), id.clone(), *is_new, fields.clone()),
                    _ => return None,
                };
                self.commit_fields(&kind, &id, is_new, &fields)
            }
            _ => None,
        }
    }

    fn commit_fields(
        &mut self,
        kind: &str,
        id: &str,
        is_new: bool,
        fields: &[(String, String)],
    ) -> Option<RetrievalCommand> {
        let get = |name: &str| {
            fields
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        match kind {
            "embedding" => {
                let mut eid = if is_new { get("id") } else { id.to_owned() };
                eid = eid.trim().to_ascii_lowercase();
                if eid.is_empty() {
                    self.status = Some("id required".into());
                    return None;
                }
                let config = EmbeddingModelConfig {
                    provider: get("provider").trim().to_owned(),
                    model: get("model").trim().to_owned(),
                    // v1: only openai_compatible (read-only in UI).
                    protocol: EmbeddingProtocol::OpenaiCompatible,
                    dimensions: get("dimensions").parse().ok(),
                    encoding: match get("encoding").as_str() {
                        "base64" => EmbeddingEncoding::Base64,
                        _ => EmbeddingEncoding::Float,
                    },
                    batch_size: get("batch_size").parse().unwrap_or(32),
                    max_input_tokens: get("max_input_tokens").parse().unwrap_or(8192),
                };
                self.edit = RetrievalEditMode::Browse;
                let op = self.next_op_id();
                Some(self.stamp_mutation(RetrievalCommand::UpsertEmbedding {
                    id: eid,
                    config,
                    expected_generation: self.generation,
                    confirm_memory_reindex: false,
                    operation_id: op,
                }))
            }
            "reranker" => {
                let mut rid = if is_new { get("id") } else { id.to_owned() };
                rid = rid.trim().to_ascii_lowercase();
                if rid.is_empty() {
                    self.status = Some("id required".into());
                    return None;
                }
                let ep = get("endpoint");
                let config = RerankerModelConfig {
                    provider: get("provider").trim().to_owned(),
                    model: get("model").trim().to_owned(),
                    protocol: match get("protocol").as_str() {
                        "cohere_compatible" => RerankerProtocol::CohereCompatible,
                        _ => RerankerProtocol::OpenaiCompatible,
                    },
                    endpoint: if ep.trim().is_empty() {
                        None
                    } else {
                        Some(ep.trim().to_owned())
                    },
                    batch_size: get("batch_size").parse().unwrap_or(32),
                    max_input_tokens: get("max_input_tokens").parse().unwrap_or(8192),
                };
                self.edit = RetrievalEditMode::Browse;
                let op = self.next_op_id();
                Some(self.stamp_mutation(RetrievalCommand::UpsertReranker {
                    id: rid,
                    config,
                    expected_generation: self.generation,
                    confirm_memory_reindex: false,
                    operation_id: op,
                }))
            }
            "profile" => {
                let mut pid = if is_new { get("id") } else { id.to_owned() };
                pid = pid.trim().to_ascii_lowercase();
                if pid.is_empty() {
                    self.status = Some("id required".into());
                    return None;
                }
                let emb: Vec<String> = get("embedding_models")
                    .split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect();
                let rr: Vec<String> = get("reranker_models")
                    .split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect();
                let config = RetrievalProfileConfig {
                    embedding_models: emb,
                    reranker_models: rr,
                    // v1: only deterministic (read-only in UI).
                    fallback_strategy: RetrievalFallbackStrategy::Deterministic,
                    max_candidates: get("max_candidates").parse().unwrap_or(50),
                    max_results: get("max_results").parse().unwrap_or(10),
                    min_score: get("min_score").parse().unwrap_or(0.0),
                    deadline_ms: get("deadline_ms").parse().unwrap_or(10_000),
                    max_attempts: get("max_attempts").parse().unwrap_or(2),
                    max_input_tokens: get("max_input_tokens").parse().unwrap_or(8192),
                    max_output_tokens: get("max_output_tokens").parse().unwrap_or(4096),
                };
                self.edit = RetrievalEditMode::Browse;
                let op = self.next_op_id();
                Some(self.stamp_mutation(RetrievalCommand::UpsertProfile {
                    id: pid,
                    config,
                    expected_generation: self.generation,
                    confirm_memory_reindex: false,
                    operation_id: op,
                }))
            }
            "prime" => {
                let enabled = get("enabled") == "true" || get("enabled") == "1";
                let profile = {
                    let p = get("retrieval_profile");
                    if p.trim().is_empty() {
                        None
                    } else {
                        Some(p.trim().to_owned())
                    }
                };
                if id == "skills" {
                    self.draft_prime.skills = SkillPrimeConfig {
                        enabled,
                        retrieval_profile: profile,
                        max_results: get("max_results").parse().unwrap_or(3),
                        max_body_chars: get("max_body_chars").parse().unwrap_or(2000),
                        max_total_chars: get("max_total_chars").parse().unwrap_or(6000),
                        max_tokens: get("max_tokens").parse().unwrap_or(1500),
                        max_context_fraction: get("max_context_fraction").parse().unwrap_or(0.05),
                        deadline_ms: get("deadline_ms").parse().unwrap_or(3000),
                        degrade_on_error: get("degrade_on_error") != "false",
                        min_score: get("min_score").parse().unwrap_or(0.0),
                    };
                } else {
                    self.draft_prime.agents = AgentPrimeConfig {
                        enabled,
                        retrieval_profile: profile,
                        max_results: get("max_results").parse().unwrap_or(3),
                        max_body_chars: get("max_body_chars").parse().unwrap_or(2000),
                        max_total_chars: get("max_total_chars").parse().unwrap_or(6000),
                        max_tokens: get("max_tokens").parse().unwrap_or(1500),
                        max_context_fraction: get("max_context_fraction").parse().unwrap_or(0.05),
                        deadline_ms: get("deadline_ms").parse().unwrap_or(3000),
                        degrade_on_error: get("degrade_on_error") != "false",
                    };
                }
                self.dirty = true;
                self.edit = RetrievalEditMode::Browse;
                let op = self.next_op_id();
                Some(self.stamp_mutation(RetrievalCommand::SavePrime {
                    prime: self.draft_prime.clone(),
                    expected_generation: self.generation,
                    confirm_memory_reindex: false,
                    operation_id: op,
                }))
            }
            "memory" => {
                let p = get("retrieval_profile");
                let profile = if p.trim().is_empty() {
                    None
                } else {
                    Some(p.trim().to_owned())
                };
                self.draft_memory_profile = profile.clone();
                self.dirty = true;
                self.edit = RetrievalEditMode::Browse;
                let op = self.next_op_id();
                Some(self.stamp_mutation(RetrievalCommand::SaveMemoryProfile {
                    profile,
                    expected_generation: self.generation,
                    confirm_memory_reindex: false,
                    operation_id: op,
                }))
            }
            _ => None,
        }
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let shortcuts = self.footer_shortcuts();
        let config = ModalWindowConfig {
            title: "Retrieval settings",
            tabs: None,
            shortcuts: &shortcuts,
            sizing: ModalSizing::large(),
            fold_info: None,
        };
        let Some(content) =
            modal_window::render_modal_window(buf, area, &mut self.window, &config, theme)
        else {
            return;
        };
        self.render_body(content.content, buf, theme);
    }

    fn footer_shortcuts(&self) -> Vec<Shortcut<'static>> {
        if self.conflict.is_some() {
            return vec![
                Shortcut {
                    label: "r Reload",
                    clickable: false,
                    id: 1,
                },
                Shortcut {
                    label: "k Keep draft",
                    clickable: false,
                    id: 2,
                },
            ];
        }
        match &self.edit {
            RetrievalEditMode::ConfirmPrimeRebuild { route, .. } => vec![
                Shortcut {
                    label: "y Confirm rebuild",
                    clickable: false,
                    id: 1,
                },
                Shortcut {
                    label: "n Cancel",
                    clickable: false,
                    id: 2,
                },
                Shortcut {
                    label: if route.is_empty() {
                        "route (none)"
                    } else {
                        "route shown"
                    },
                    clickable: false,
                    id: 3,
                },
            ],
            RetrievalEditMode::ConfirmPrimeBackfill { route, .. } => vec![
                Shortcut {
                    label: "y Confirm backfill",
                    clickable: false,
                    id: 1,
                },
                Shortcut {
                    label: "n Cancel",
                    clickable: false,
                    id: 2,
                },
                Shortcut {
                    label: if route.is_empty() {
                        "route (none)"
                    } else {
                        "route shown"
                    },
                    clickable: false,
                    id: 3,
                },
            ],
            RetrievalEditMode::Browse => vec![
                Shortcut {
                    label: "Tab Page",
                    clickable: false,
                    id: 1,
                },
                Shortcut {
                    label: "a Add",
                    clickable: false,
                    id: 2,
                },
                Shortcut {
                    label: "e Edit",
                    clickable: false,
                    id: 3,
                },
                Shortcut {
                    label: "y Clone",
                    clickable: false,
                    id: 4,
                },
                Shortcut {
                    label: "d Delete",
                    clickable: false,
                    id: 5,
                },
                Shortcut {
                    label: "J/K Reorder",
                    clickable: false,
                    id: 6,
                },
                Shortcut {
                    // Synthetic network-free validation preview (not a disk reload).
                    label: "v Preview",
                    clickable: false,
                    id: 7,
                },
                Shortcut {
                    // Browse `s` → ValidateAndReload / LoadSnapshot (no gen churn).
                    label: "s Refresh",
                    clickable: false,
                    id: 8,
                },
                Shortcut {
                    label: "r Reload",
                    clickable: false,
                    id: 9,
                },
                Shortcut {
                    label: "Esc Close",
                    clickable: false,
                    id: 10,
                },
            ],
            RetrievalEditMode::EditFields {
                editing_value: true,
                ..
            } => vec![
                Shortcut {
                    label: "Enter Accept + next field",
                    clickable: false,
                    id: 1,
                },
                Shortcut {
                    label: "Esc Cancel field",
                    clickable: false,
                    id: 2,
                },
            ],
            RetrievalEditMode::EditFields { .. } => vec![
                Shortcut {
                    label: "j/k Field",
                    clickable: false,
                    id: 1,
                },
                Shortcut {
                    label: "e Edit value",
                    clickable: false,
                    id: 2,
                },
                Shortcut {
                    label: "s Commit",
                    clickable: false,
                    id: 3,
                },
                Shortcut {
                    label: "Esc Back",
                    clickable: false,
                    id: 4,
                },
            ],
            RetrievalEditMode::ConfirmDelete { .. }
            | RetrievalEditMode::ConfirmMemoryReindex { .. } => vec![
                Shortcut {
                    label: "y Confirm",
                    clickable: false,
                    id: 1,
                },
                Shortcut {
                    label: "n Cancel",
                    clickable: false,
                    id: 2,
                },
            ],
            RetrievalEditMode::ClonePrompt { .. } => vec![
                Shortcut {
                    label: "Enter Clone",
                    clickable: false,
                    id: 1,
                },
                Shortcut {
                    label: "Esc Cancel",
                    clickable: false,
                    id: 2,
                },
            ],
        }
    }

    fn render_body(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let mut lines: Vec<Line> = Vec::new();
        let tabs: Vec<Span> = RetrievalPage::ALL
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let label = format!(" {} {} ", i + 1, p.label());
                if *p == self.page {
                    Span::styled(
                        label,
                        Style::default()
                            .fg(theme.accent_user)
                            .add_modifier(Modifier::BOLD | Modifier::REVERSED),
                    )
                } else {
                    Span::styled(label, Style::default().fg(theme.gray))
                }
            })
            .collect();
        lines.push(Line::from(tabs));
        lines.push(Line::from(""));

        if self.loading {
            lines.push(Line::from(Span::styled(
                "Loading…",
                Style::default().fg(theme.gray),
            )));
        } else if let Some(err) = &self.error {
            lines.push(Line::from(Span::styled(
                bound_prime_status_line(&format!("Error: {err}"), area.width),
                Style::default().fg(theme.accent_error),
            )));
        }

        if let Some(c) = &self.conflict {
            lines.push(Line::from(Span::styled(
                format!(
                    "CONFLICT client={} live={} fields={:?}",
                    c.client_generation.get(),
                    c.live_generation.get(),
                    c.changed_fields
                ),
                Style::default()
                    .fg(theme.accent_error)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(c.guidance.as_str()));
            lines.push(Line::from(""));
        }

        if let Some(status) = &self.status {
            lines.push(Line::from(Span::styled(
                bound_prime_status_line(status, area.width),
                Style::default().fg(theme.accent_user),
            )));
        }

        match &self.edit {
            RetrievalEditMode::Browse => self.render_browse(&mut lines, theme, area.width),
            RetrievalEditMode::EditFields {
                kind,
                id,
                is_new,
                fields,
                field_idx,
                editing_value,
            } => {
                lines.push(Line::from(Span::styled(
                    format!(
                        "{} {} `{}`",
                        if *is_new { "Add" } else { "Edit" },
                        kind,
                        if id.is_empty() { "(new)" } else { id }
                    ),
                    Style::default().add_modifier(Modifier::BOLD),
                )));
                let hint = if *editing_value {
                    "type the value · Enter saves the field and advances · Esc leaves editing"
                } else {
                    "j/k select a field · e edit its value · s save · Esc back"
                };
                lines.push(Line::from(Span::styled(
                    hint,
                    Style::default().fg(theme.gray),
                )));
                for (i, (k, v)) in fields.iter().enumerate() {
                    let fixed = is_fixed_field_label(k);
                    let focused = i == *field_idx;
                    let mark = if fixed {
                        "  "
                    } else if focused {
                        "> "
                    } else {
                        "  "
                    };
                    let editing_here = focused && *editing_value && !fixed;
                    let (val, val_is_hint) = if editing_here {
                        (format!("{}█", self.line_editor.text()), false)
                    } else if v.is_empty() && !fixed {
                        ("(empty)".to_string(), true)
                    } else {
                        (
                            truncate_id(v, area.width.saturating_sub(24) as usize),
                            false,
                        )
                    };
                    let key_style = if focused {
                        Style::default()
                            .fg(theme.accent_user)
                            .add_modifier(Modifier::BOLD)
                    } else if fixed {
                        Style::default().fg(theme.gray)
                    } else {
                        Style::default()
                    };
                    let val_style = if editing_here {
                        key_style
                    } else if val_is_hint || fixed {
                        Style::default().fg(theme.gray)
                    } else {
                        Style::default()
                    };
                    let suffix = if fixed { "  [read-only]" } else { "" };
                    lines.push(Line::from(vec![
                        Span::styled(format!("{mark}{k}: "), key_style),
                        Span::styled(format!("{val}{suffix}"), val_style),
                    ]));
                }
            }
            RetrievalEditMode::ConfirmDelete { kind, id } => {
                lines.push(Line::from(format!(
                    "Delete {kind} `{id}`? [y] confirm  [n] cancel"
                )));
            }
            RetrievalEditMode::ConfirmMemoryReindex { impact } => {
                lines.push(Line::from(Span::styled(
                    "Memory reindex impact (config only — reindex is NOT executed)",
                    Style::default()
                        .fg(theme.warning)
                        .add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(impact.reason.as_str()));
                if let Some(p) = &impact.previous_fingerprint {
                    lines.push(Line::from(format!("previous: {p}")));
                }
                if let Some(n) = &impact.next_fingerprint {
                    lines.push(Line::from(format!("next:     {n}")));
                }
                lines.push(Line::from("[y] confirm save  [n] cancel"));
            }
            RetrievalEditMode::ClonePrompt { kind, source_id } => {
                lines.push(Line::from(format!(
                    "Clone {kind} `{source_id}` → new id: {}_",
                    self.line_editor.text()
                )));
            }
            RetrievalEditMode::ConfirmPrimeRebuild { collection, route } => {
                lines.push(Line::from(Span::styled(
                    format!("Rebuild {collection} index?"),
                    Style::default()
                        .fg(theme.warning)
                        .add_modifier(Modifier::BOLD),
                )));
                let shown = display_configured_route(Some(route.as_str()));
                if shown.is_empty() {
                    lines.push(Line::from(PRIME_UNAVAILABLE_PROFILE));
                } else {
                    lines.push(Line::from(format!("Configured route: {shown}")));
                }
                lines.push(Line::from(
                    "This rebuilds vectors. Saving configuration does not rebuild.",
                ));
                lines.push(Line::from("[y] confirm  [n] cancel"));
            }
            RetrievalEditMode::ConfirmPrimeBackfill { collection, route } => {
                lines.push(Line::from(Span::styled(
                    format!("Backfill {collection} index?"),
                    Style::default()
                        .fg(theme.warning)
                        .add_modifier(Modifier::BOLD),
                )));
                let shown = display_configured_route(Some(route.as_str()));
                if shown.is_empty() {
                    lines.push(Line::from(PRIME_UNAVAILABLE_PROFILE));
                } else {
                    lines.push(Line::from(format!("Configured route: {shown}")));
                }
                lines.push(Line::from(
                    "This contacts the embedding profile. Saving configuration does not backfill.",
                ));
                lines.push(Line::from("[y] confirm  [n] cancel"));
            }
        }

        if self.page == RetrievalPage::Validate {
            if let Some(p) = &self.last_preview {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Preview (network-free)",
                    Style::default().add_modifier(Modifier::BOLD),
                )));
                for m in &p.messages {
                    lines.push(Line::from(m.as_str()));
                }
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(
                "gen={} dirty={} {}",
                self.generation.get(),
                self.dirty,
                if self.snapshot.as_ref().map(|s| s.is_valid).unwrap_or(false) {
                    "valid"
                } else {
                    "invalid"
                }
            ),
            Style::default().fg(theme.gray_dim),
        )));

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn render_browse(&self, lines: &mut Vec<Line>, theme: &Theme, width: u16) {
        let Some(snap) = &self.snapshot else {
            lines.push(Line::from("No snapshot loaded."));
            return;
        };
        match self.page {
            RetrievalPage::EmbeddingModels => {
                if snap.embedding_models.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "(no embedding models — press a to add)",
                        Style::default().fg(theme.gray),
                    )));
                }
                for (i, e) in snap.embedding_models.iter().enumerate() {
                    let mark = if i == self.selected { ">" } else { " " };
                    lines.push(Line::from(format!(
                        "{mark} {}  provider={} model={} dims={:?}",
                        truncate_id(&e.id, 24),
                        e.config.provider,
                        e.config.model,
                        e.config.dimensions
                    )));
                }
            }
            RetrievalPage::Rerankers => {
                if snap.reranker_models.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "(no rerankers — press a to add)",
                        Style::default().fg(theme.gray),
                    )));
                }
                for (i, e) in snap.reranker_models.iter().enumerate() {
                    let mark = if i == self.selected { ">" } else { " " };
                    lines.push(Line::from(format!(
                        "{mark} {}  provider={} model={} endpoint={:?}",
                        truncate_id(&e.id, 24),
                        e.config.provider,
                        e.config.model,
                        e.config.endpoint
                    )));
                }
            }
            RetrievalPage::Profiles => {
                if snap.retrieval_profiles.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "(no profiles — press a to add)",
                        Style::default().fg(theme.gray),
                    )));
                }
                for (i, e) in snap.retrieval_profiles.iter().enumerate() {
                    let mark = if i == self.selected { ">" } else { " " };
                    lines.push(Line::from(format!(
                        "{mark} {}  emb={:?} rr={:?} cand={} res={} score={}",
                        truncate_id(&e.id, 20),
                        e.config.embedding_models,
                        e.config.reranker_models,
                        e.config.max_candidates,
                        e.config.max_results,
                        e.config.min_score
                    )));
                }
            }
            RetrievalPage::Prime => {
                // Prefer draft while dirty so pending edits are visible.
                let prime = if self.dirty {
                    &self.draft_prime
                } else {
                    // snapshot prime is PrimeDto-compatible via fields
                    // rebuild view from draft when empty snap
                    &self.draft_prime
                };
                let skills_mark = if self.selected == 0 { ">" } else { " " };
                let agents_mark = if self.selected == 1 { ">" } else { " " };
                lines.push(Line::from(format!(
                    "{skills_mark} skills  enabled={} profile={:?} results={} degrade={}",
                    prime.skills.enabled,
                    prime.skills.retrieval_profile,
                    prime.skills.max_results,
                    prime.skills.degrade_on_error
                )));
                lines.push(Line::from(format!(
                    "{agents_mark} agents  enabled={} profile={:?} results={} degrade={}",
                    prime.agents.enabled,
                    prime.agents.retrieval_profile,
                    prime.agents.max_results,
                    prime.agents.degrade_on_error
                )));
                if self.dirty {
                    lines.push(Line::from(Span::styled(
                        "(draft — not yet saved)",
                        Style::default().fg(theme.warning),
                    )));
                }
                lines.push(Line::from(Span::styled(
                    "Saving configuration does not rebuild the index.",
                    Style::default().fg(theme.gray),
                )));
                if let Some(idx) = &self.prime_index {
                    let coll = if self.selected == 0 {
                        &idx.skills
                    } else {
                        &idx.agents
                    };
                    lines.push(Line::from(format!(
                        "  index {}/{} {} · gen {}",
                        coll.vector_count, coll.item_count, coll.readiness, coll.generation
                    )));
                    let shown = display_configured_route(
                        idx.configured_route.as_deref().or(coll.route_id.as_deref()),
                    );
                    if !shown.is_empty() {
                        lines.push(Line::from(format!("  route {shown}")));
                    }
                    if let Some(job) = &idx.job {
                        let line = format!(
                            "  job {} {}/{}",
                            compact_prime_job_label(job),
                            job.done,
                            job.total
                        );
                        let cap = width as usize;
                        lines.push(Line::from(if cap == 0 {
                            line
                        } else {
                            crate::render::line_utils::truncate_str(&line, cap)
                        }));
                    }
                }
                lines.push(Line::from(Span::styled(
                    "b backfill (confirm route)  ·  u rebuild (confirm route)  ·  s save config",
                    Style::default().fg(theme.gray),
                )));
            }
            RetrievalPage::Memory => {
                let shown = if self.dirty {
                    self.draft_memory_profile.as_ref()
                } else {
                    snap.memory_retrieval_profile.as_ref()
                };
                lines.push(Line::from(format!("> retrieval_profile = {:?}", shown)));
                if self.dirty {
                    lines.push(Line::from(Span::styled(
                        "(draft — not yet saved)",
                        Style::default().fg(theme.warning),
                    )));
                }
                lines.push(Line::from(Span::styled(
                    "Legacy [memory.embedding]/[memory.search] remain readable and unchanged.",
                    Style::default().fg(theme.gray),
                )));
                lines.push(Line::from(Span::styled(
                    "Changing the profile may require reindex confirmation (not executed here).",
                    Style::default().fg(theme.gray),
                )));
            }
            RetrievalPage::Validate => {
                if snap.is_valid {
                    lines.push(Line::from(Span::styled(
                        "Graph is valid (no hard errors).",
                        Style::default().fg(theme.accent_success),
                    )));
                } else {
                    lines.push(Line::from(Span::styled(
                        "Graph has validation errors:",
                        Style::default().fg(theme.accent_error),
                    )));
                }
                for e in &snap.validation_errors {
                    lines.push(Line::from(Span::styled(
                        format!("  ! {e}"),
                        Style::default().fg(theme.accent_error),
                    )));
                }
                for w in &snap.validation_warnings {
                    lines.push(Line::from(Span::styled(
                        format!("  ~ {w}"),
                        Style::default().fg(theme.warning),
                    )));
                }
                for w in &snap.warnings {
                    lines.push(Line::from(Span::styled(
                        format!("  parse: {w}"),
                        Style::default().fg(theme.gray),
                    )));
                }
            }
        }
    }
}

fn truncate_id(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

/// v1 single-value enum rows labeled `(fixed)` — not free-form editable.
fn is_fixed_field_label(label: &str) -> bool {
    label.contains("(fixed)")
}

fn next_editable_field_idx(fields: &[(String, String)], from: usize) -> usize {
    let mut i = from.saturating_add(1);
    while i < fields.len() {
        if !is_fixed_field_label(&fields[i].0) {
            return i;
        }
        i += 1;
    }
    from
}

fn prev_editable_field_idx(fields: &[(String, String)], from: usize) -> usize {
    if from == 0 {
        return from;
    }
    let mut i = from;
    while i > 0 {
        i -= 1;
        if !is_fixed_field_label(&fields[i].0) {
            return i;
        }
    }
    from
}

fn first_editable_field_idx(fields: &[(String, String)]) -> usize {
    fields
        .iter()
        .position(|(k, _)| !is_fixed_field_label(k))
        .unwrap_or(0)
}

fn default_embedding_fields(existing: Option<&EmbeddingModelDto>) -> Vec<(String, String)> {
    match existing {
        None => vec![
            ("id".into(), String::new()),
            ("provider".into(), String::new()),
            ("model".into(), String::new()),
            // protocol is v1 single-value (read-only label; not free-form).
            ("protocol (fixed)".into(), "openai_compatible".into()),
            ("dimensions".into(), String::new()),
            ("encoding".into(), "float".into()),
            ("batch_size".into(), "32".into()),
            ("max_input_tokens".into(), "8192".into()),
        ],
        Some(e) => vec![
            ("id".into(), e.id.clone()),
            ("provider".into(), e.config.provider.clone()),
            ("model".into(), e.config.model.clone()),
            ("protocol (fixed)".into(), e.config.protocol.as_str().into()),
            (
                "dimensions".into(),
                e.config
                    .dimensions
                    .map(|d| d.to_string())
                    .unwrap_or_default(),
            ),
            ("encoding".into(), e.config.encoding.as_str().into()),
            ("batch_size".into(), e.config.batch_size.to_string()),
            (
                "max_input_tokens".into(),
                e.config.max_input_tokens.to_string(),
            ),
        ],
    }
}

fn default_reranker_fields(existing: Option<&RerankerModelDto>) -> Vec<(String, String)> {
    match existing {
        None => vec![
            ("id".into(), String::new()),
            ("provider".into(), String::new()),
            ("model".into(), String::new()),
            ("protocol".into(), "openai_compatible".into()),
            ("endpoint".into(), String::new()),
            ("batch_size".into(), "32".into()),
            ("max_input_tokens".into(), "8192".into()),
        ],
        Some(e) => vec![
            ("id".into(), e.id.clone()),
            ("provider".into(), e.config.provider.clone()),
            ("model".into(), e.config.model.clone()),
            ("protocol".into(), e.config.protocol.as_str().into()),
            (
                "endpoint".into(),
                e.config.endpoint.clone().unwrap_or_default(),
            ),
            ("batch_size".into(), e.config.batch_size.to_string()),
            (
                "max_input_tokens".into(),
                e.config.max_input_tokens.to_string(),
            ),
        ],
    }
}

fn default_profile_fields(existing: Option<&RetrievalProfileDto>) -> Vec<(String, String)> {
    match existing {
        None => vec![
            ("id".into(), String::new()),
            ("embedding_models".into(), String::new()),
            ("reranker_models".into(), String::new()),
            ("fallback_strategy (fixed)".into(), "deterministic".into()),
            ("max_candidates".into(), "50".into()),
            ("max_results".into(), "10".into()),
            ("min_score".into(), "0.0".into()),
            ("deadline_ms".into(), "10000".into()),
            ("max_attempts".into(), "2".into()),
            ("max_input_tokens".into(), "8192".into()),
            ("max_output_tokens".into(), "4096".into()),
        ],
        Some(e) => vec![
            ("id".into(), e.id.clone()),
            (
                "embedding_models".into(),
                e.config.embedding_models.join(","),
            ),
            ("reranker_models".into(), e.config.reranker_models.join(",")),
            (
                "fallback_strategy (fixed)".into(),
                e.config.fallback_strategy.as_str().into(),
            ),
            ("max_candidates".into(), e.config.max_candidates.to_string()),
            ("max_results".into(), e.config.max_results.to_string()),
            ("min_score".into(), e.config.min_score.to_string()),
            ("deadline_ms".into(), e.config.deadline_ms.to_string()),
            ("max_attempts".into(), e.config.max_attempts.to_string()),
            (
                "max_input_tokens".into(),
                e.config.max_input_tokens.to_string(),
            ),
            (
                "max_output_tokens".into(),
                e.config.max_output_tokens.to_string(),
            ),
        ],
    }
}

fn prime_fields(prime: &PrimeConfig, skills: bool) -> Vec<(String, String)> {
    if skills {
        let s = &prime.skills;
        vec![
            ("enabled".into(), s.enabled.to_string()),
            (
                "retrieval_profile".into(),
                s.retrieval_profile.clone().unwrap_or_default(),
            ),
            ("max_results".into(), s.max_results.to_string()),
            ("max_body_chars".into(), s.max_body_chars.to_string()),
            ("max_total_chars".into(), s.max_total_chars.to_string()),
            ("max_tokens".into(), s.max_tokens.to_string()),
            (
                "max_context_fraction".into(),
                s.max_context_fraction.to_string(),
            ),
            ("deadline_ms".into(), s.deadline_ms.to_string()),
            ("degrade_on_error".into(), s.degrade_on_error.to_string()),
            ("min_score".into(), s.min_score.to_string()),
        ]
    } else {
        let a = &prime.agents;
        vec![
            ("enabled".into(), a.enabled.to_string()),
            (
                "retrieval_profile".into(),
                a.retrieval_profile.clone().unwrap_or_default(),
            ),
            ("max_results".into(), a.max_results.to_string()),
            ("max_body_chars".into(), a.max_body_chars.to_string()),
            ("max_total_chars".into(), a.max_total_chars.to_string()),
            ("max_tokens".into(), a.max_tokens.to_string()),
            (
                "max_context_fraction".into(),
                a.max_context_fraction.to_string(),
            ),
            ("deadline_ms".into(), a.deadline_ms.to_string()),
            ("degrade_on_error".into(), a.degrade_on_error.to_string()),
        ]
    }
}

/// Compact status when Prime rebuild/backfill has no saved profile route.
pub(crate) const PRIME_UNAVAILABLE_PROFILE: &str = "No retrieval profile saved";
/// Skills-list hint under [`PRIME_UNAVAILABLE_PROFILE`]. Retrieval settings
/// already *is* the editor, so this line is overlay-only.
pub(crate) const PRIME_UNAVAILABLE_PROFILE_HINT: &str =
    "Press Enter to open Retrieval settings and create one.";

/// Profile id only — never show an endpoint, path, or credential.
pub(crate) fn display_configured_route(raw: Option<&str>) -> String {
    xai_grok_shell::session::prime::displayable_configured_route(raw)
        .unwrap_or("")
        .to_owned()
}

fn bound_prime_status_line(text: &str, width: u16) -> String {
    let cap = width as usize;
    if cap == 0 {
        text.to_owned()
    } else {
        crate::render::line_utils::truncate_str(text, cap)
    }
}

/// Shared compact job label for Skills footer and Retrieval Prime browse.
/// Maps confirm-required to `confirm` / `unavailable`; never returns raw
/// failure text.
pub(crate) fn compact_prime_job_label(
    job: &xai_grok_shell::session::prime::PrimeIndexJobStatus,
) -> &'static str {
    let failure = job
        .failure
        .as_deref()
        .map(xai_grok_shell::session::prime::sanitize_prime_job_failure)
        .unwrap_or_default();
    if xai_grok_shell::session::prime::prime_failure_is_confirm_required(Some(&failure)) {
        let suffix = failure.split_once(':').map(|(_, rest)| rest.trim());
        if display_configured_route(job.configured_route.as_deref()).is_empty()
            && display_configured_route(suffix).is_empty()
        {
            "unavailable"
        } else {
            "confirm"
        }
    } else if failure == "unavailable" {
        "unavailable"
    } else {
        match job.state.as_str() {
            "running" => "running",
            "cancelling" => "cancelling",
            "completed" => "completed",
            "failed" => "failed",
            "cancelled" => "cancelled",
            _ => "failed",
        }
    }
}

/// Bounded, secret-free error for Retrieval/Skills overlays.
pub(crate) fn compact_prime_job_error(message: &str) -> String {
    let code = xai_grok_shell::session::prime::sanitize_prime_job_failure(message);
    if code.is_empty() {
        return "failed".into();
    }
    if xai_grok_shell::session::prime::prime_failure_is_confirm_required(Some(&code)) {
        if xai_grok_shell::session::prime::confirm_required_display_route(&code).is_some() {
            "confirm required".into()
        } else {
            PRIME_UNAVAILABLE_PROFILE.into()
        }
    } else if code == "unavailable" {
        PRIME_UNAVAILABLE_PROFILE.into()
    } else {
        code
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use xai_grok_shell::retrieval_config::dto::RetrievalGraphSnapshot;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn snap_with_emb() -> RetrievalGraphSnapshot {
        RetrievalGraphSnapshot {
            generation: RegistryGeneration(1),
            embedding_models: vec![EmbeddingModelDto {
                id: "e1".into(),
                config: EmbeddingModelConfig {
                    provider: "lab".into(),
                    model: "m".into(),
                    ..Default::default()
                },
            }],
            is_valid: true,
            ..Default::default()
        }
    }

    #[test]
    fn prime_page_backfill_and_rebuild_are_not_save() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        s.page = RetrievalPage::Prime;
        s.selected = 0;
        assert!(
            s.handle_key(key(KeyCode::Char('b'))).is_none(),
            "no saved profile must not spawn a backfill job"
        );
        assert_eq!(s.status.as_deref(), Some(PRIME_UNAVAILABLE_PROFILE));
        assert_eq!(s.error.as_deref(), Some(PRIME_UNAVAILABLE_PROFILE));
        s.error = None;
        s.status = None;
        s.prime_index = Some(xai_grok_shell::session::prime::PrimeIndexStatus {
            api_version: 1,
            generation: 1,
            fingerprint_short: "abc123def456".into(),
            skills: xai_grok_shell::session::prime::PrimeIndexCollectionStatus {
                collection: "skills".into(),
                generation: 1,
                fingerprint_short: "abc123def456".into(),
                item_count: 1,
                vector_count: 0,
                missing_vectors: 1,
                readiness: "pending".into(),
                route_id: Some("main".into()),
                dimensions: None,
            },
            agents: xai_grok_shell::session::prime::PrimeIndexCollectionStatus {
                collection: "agents".into(),
                generation: 0,
                fingerprint_short: String::new(),
                item_count: 0,
                vector_count: 0,
                missing_vectors: 0,
                readiness: "ready".into(),
                route_id: None,
                dimensions: None,
            },
            job: None,
            configured_route: Some("main".into()),
            capabilities: xai_grok_shell::session::prime::PrimeIndexCapabilities::SUPPORTED,
            unchanged: false,
        });
        s.handle_key(key(KeyCode::Char('u')));
        match &s.edit {
            RetrievalEditMode::ConfirmPrimeRebuild { collection, route } => {
                assert_eq!(collection, "skills");
                assert_eq!(route, "main", "confirm must display the configured route");
            }
            other => panic!("expected confirm rebuild, got {other:?}"),
        }
        match s.handle_key(key(KeyCode::Char('y'))) {
            Some(RetrievalCommand::PrimeIndexRebuild { confirm, .. }) => {
                assert!(confirm, "rebuild must confirm the configured route");
            }
            other => panic!("expected rebuild confirm, got {other:?}"),
        }
        s.edit = RetrievalEditMode::Browse;
        s.dirty = true;
        let cmd = s.handle_key(key(KeyCode::Char('s')));
        assert!(
            matches!(cmd, Some(RetrievalCommand::ValidateAndReload)),
            "browse s still refreshes config and does not rebuild, got {cmd:?}"
        );
    }

    #[test]
    fn prime_page_backfill_with_configured_route_requires_confirm() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        s.page = RetrievalPage::Prime;
        s.selected = 0;
        s.prime_index = Some(xai_grok_shell::session::prime::PrimeIndexStatus {
            api_version: 1,
            generation: 1,
            fingerprint_short: "abc123def456".into(),
            skills: xai_grok_shell::session::prime::PrimeIndexCollectionStatus {
                collection: "skills".into(),
                generation: 1,
                fingerprint_short: "abc123def456".into(),
                item_count: 1,
                vector_count: 0,
                missing_vectors: 1,
                readiness: "pending".into(),
                route_id: Some("main".into()),
                dimensions: None,
            },
            agents: xai_grok_shell::session::prime::PrimeIndexCollectionStatus {
                collection: "agents".into(),
                generation: 0,
                fingerprint_short: String::new(),
                item_count: 0,
                vector_count: 0,
                missing_vectors: 0,
                readiness: "ready".into(),
                route_id: None,
                dimensions: None,
            },
            job: None,
            configured_route: Some("main".into()),
            capabilities: xai_grok_shell::session::prime::PrimeIndexCapabilities::SUPPORTED,
            unchanged: false,
        });
        assert!(s.handle_key(key(KeyCode::Char('b'))).is_none());
        match &s.edit {
            RetrievalEditMode::ConfirmPrimeBackfill { collection, route } => {
                assert_eq!(collection, "skills");
                assert_eq!(route, "main");
            }
            other => panic!("expected confirm backfill, got {other:?}"),
        }
        match s.handle_key(key(KeyCode::Char('y'))) {
            Some(RetrievalCommand::PrimeIndexBackfill {
                confirm,
                collection,
            }) => {
                assert_eq!(collection, "skills");
                assert!(confirm, "backfill must confirm the configured route");
            }
            other => panic!("expected backfill confirm, got {other:?}"),
        }
        s.edit = RetrievalEditMode::Browse;
        s.error = None;
        s.status = None;
        s.prime_index.as_mut().unwrap().configured_route = Some("sk-live-secret".into());
        assert!(
            s.handle_key(key(KeyCode::Char('b'))).is_none(),
            "credential-like routes must not spawn a job"
        );
        assert_eq!(s.status.as_deref(), Some(PRIME_UNAVAILABLE_PROFILE));
        assert!(matches!(s.edit, RetrievalEditMode::Browse));
        assert_eq!(display_configured_route(Some("http://127.0.0.1/v1")), "");
        assert_eq!(display_configured_route(Some("main")), "main");
        assert_eq!(display_configured_route(Some("sk-live-secret")), "");
        assert_eq!(display_configured_route(Some("file:///tmp/secret")), "");
        assert_eq!(display_configured_route(Some("main\nsk-live-secret")), "");
        assert_eq!(display_configured_route(Some(&"x".repeat(200))), "");
    }

    #[test]
    fn prime_page_without_route_u_y_and_b_leave_visible_error() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        s.page = RetrievalPage::Prime;
        s.selected = 0;
        assert!(s.handle_key(key(KeyCode::Char('u'))).is_none());
        assert_eq!(s.status.as_deref(), Some(PRIME_UNAVAILABLE_PROFILE));
        assert_eq!(s.error.as_deref(), Some(PRIME_UNAVAILABLE_PROFILE));
        assert!(matches!(s.edit, RetrievalEditMode::Browse));
        assert!(
            s.handle_key(key(KeyCode::Char('y'))).is_none(),
            "y must not start a rebuild without a saved profile"
        );
        s.error = None;
        s.status = None;
        assert!(s.handle_key(key(KeyCode::Char('b'))).is_none());
        assert_eq!(s.status.as_deref(), Some(PRIME_UNAVAILABLE_PROFILE));
        assert!(matches!(s.edit, RetrievalEditMode::Browse));
    }

    fn buffer_text(buf: &Buffer) -> String {
        let area = *buf.area();
        let mut out = String::new();
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn prime_status_with_failure(
        route: Option<&str>,
        failure: &str,
    ) -> xai_grok_shell::session::prime::PrimeIndexStatus {
        xai_grok_shell::session::prime::PrimeIndexStatus {
            api_version: 1,
            generation: 4,
            fingerprint_short: "abc123def456".into(),
            skills: xai_grok_shell::session::prime::PrimeIndexCollectionStatus {
                collection: "skills".into(),
                generation: 4,
                fingerprint_short: "abc123def456".into(),
                item_count: 3,
                vector_count: 1,
                missing_vectors: 2,
                readiness: "pending".into(),
                route_id: route.map(str::to_owned),
                dimensions: None,
            },
            agents: xai_grok_shell::session::prime::PrimeIndexCollectionStatus {
                collection: "agents".into(),
                generation: 0,
                fingerprint_short: String::new(),
                item_count: 0,
                vector_count: 0,
                missing_vectors: 0,
                readiness: "ready".into(),
                route_id: None,
                dimensions: None,
            },
            job: Some(xai_grok_shell::session::prime::PrimeIndexJobStatus {
                api_version: 1,
                job_id: "j1".into(),
                kind: "backfill".into(),
                collection: "skills".into(),
                state: "failed".into(),
                generation: 4,
                fingerprint_short: "abc123def456".into(),
                done: 0,
                total: 3,
                confirm_configured_profile: false,
                configured_route: route.map(str::to_owned),
                failure: Some(failure.into()),
            }),
            configured_route: route.map(str::to_owned),
            capabilities: xai_grok_shell::session::prime::PrimeIndexCapabilities::SUPPORTED,
            unchanged: false,
        }
    }

    #[test]
    fn prime_browse_job_line_omits_raw_failure_payloads_at_narrow_width() {
        let long = "x".repeat(200);
        let cases: &[(&str, &str)] = &[
            (
                "http://127.0.0.1/v1",
                "confirm_required:http://127.0.0.1/v1",
            ),
            ("sk-live-secret", "confirm_required:sk-live-secret"),
            ("file:///tmp/secret", "confirm_required:file:///tmp/secret"),
            (
                "main\nsk-live-secret",
                "confirm_required:main\nsk-live-secret",
            ),
            (long.as_str(), long.as_str()),
        ];
        for (raw, failure) in cases {
            let mut s = RetrievalSettingsState::new();
            s.apply_snapshot(snap_with_emb());
            s.page = RetrievalPage::Prime;
            s.selected = 0;
            s.prime_index = Some(prime_status_with_failure(Some(raw), failure));
            let job = s
                .prime_index
                .as_ref()
                .and_then(|st| st.job.as_ref())
                .unwrap();
            let label = compact_prime_job_label(job);
            assert!(
                matches!(label, "confirm" | "unavailable" | "failed"),
                "compact label for {raw:?}: {label}"
            );
            assert!(!label.contains(raw.trim()), "{label}");
            // Retrieval Settings uses ModalSizing::large (min 60 cols, 7-row
            // vertical margin). 80x40 is still a narrow content width.
            let area = Rect::new(0, 0, 80, 40);
            let mut buf = Buffer::empty(area);
            s.render(area, &mut buf, &Theme::current());
            let text = buffer_text(&buf);
            assert!(
                !text.contains(raw.trim()),
                "browse leaked {raw:?} in:\n{text}"
            );
            assert!(
                !text.contains("127.0.0.1") || !raw.contains("127"),
                "{text}"
            );
            assert!(
                !text.contains("sk-live-secret") || !raw.contains("sk-"),
                "{text}"
            );
            assert!(!text.contains("file://"), "{text}");
            assert!(
                !text.contains('\n') || !raw.contains('\n') || !text.contains("sk-live"),
                "{text}"
            );
            let understandable =
                text.contains("confirm") || text.contains("unavailable") || text.contains("failed");
            assert!(
                understandable,
                "browse must still show a compact job state:\n{text}"
            );
            for row in text.lines() {
                let cols: usize = row.chars().count();
                assert!(
                    cols <= 80,
                    "job line must clamp to terminal width, got {cols}: {row:?}"
                );
            }
        }
    }

    #[test]
    fn prime_confirm_cancel_then_retry_dispatches_once() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        s.page = RetrievalPage::Prime;
        s.selected = 0;
        s.prime_index = Some(prime_status_with_failure(Some("main"), ""));
        s.prime_index.as_mut().unwrap().job = None;
        assert!(s.handle_key(key(KeyCode::Char('b'))).is_none());
        assert!(matches!(
            s.edit,
            RetrievalEditMode::ConfirmPrimeBackfill { .. }
        ));
        assert!(s.handle_key(key(KeyCode::Char('n'))).is_none());
        assert!(matches!(s.edit, RetrievalEditMode::Browse));
        assert!(s.handle_key(key(KeyCode::Char('b'))).is_none());
        match s.handle_key(key(KeyCode::Char('y'))) {
            Some(RetrievalCommand::PrimeIndexBackfill { confirm, .. }) => {
                assert!(confirm);
            }
            other => panic!("retry y must dispatch confirmed backfill, got {other:?}"),
        }
    }

    #[test]
    fn prime_confirm_render_omits_unsanitary_route_at_narrow_width() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        s.page = RetrievalPage::Prime;
        s.edit = RetrievalEditMode::ConfirmPrimeBackfill {
            collection: "skills".into(),
            route: "http://127.0.0.1/v1".into(),
        };
        let area = Rect::new(0, 0, 80, 40);
        let mut buf = Buffer::empty(area);
        s.render(area, &mut buf, &Theme::current());
        let text = buffer_text(&buf);
        assert!(!text.contains("127.0.0.1"), "{text}");
        assert!(!text.contains("http://"), "{text}");
        assert!(text.contains(PRIME_UNAVAILABLE_PROFILE), "{text}");
        assert!(
            s.handle_key(key(KeyCode::Char('y'))).is_none(),
            "y on an unsanitary confirm must not start a job"
        );
        assert_eq!(s.status.as_deref(), Some(PRIME_UNAVAILABLE_PROFILE));
    }

    #[test]
    fn compact_prime_job_error_never_returns_raw_payload() {
        assert_eq!(
            compact_prime_job_error("couldn't run prime index: confirm_required:main"),
            "confirm required"
        );
        assert_eq!(
            compact_prime_job_error("confirm_required:http://127.0.0.1/v1"),
            PRIME_UNAVAILABLE_PROFILE
        );
        assert_eq!(compact_prime_job_error("sk-live-secret"), "failed");
        assert_eq!(compact_prime_job_error(&"x".repeat(200)), "failed");
        assert_eq!(
            compact_prime_job_error("already_running"),
            "already_running"
        );
        let job = prime_status_with_failure(
            Some("http://127.0.0.1/v1"),
            "confirm_required:http://127.0.0.1/v1",
        )
        .job
        .unwrap();
        assert_eq!(compact_prime_job_label(&job), "unavailable");
    }

    #[test]
    fn keyboard_page_navigation() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        assert_eq!(s.page, RetrievalPage::EmbeddingModels);
        s.handle_key(key(KeyCode::Tab));
        assert_eq!(s.page, RetrievalPage::Rerankers);
        s.handle_key(key(KeyCode::Char('3')));
        assert_eq!(s.page, RetrievalPage::Profiles);
    }

    #[test]
    fn unit_dirty_conflict_vs_clean_reload_signal() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        s.dirty = true;
        assert!(!s.on_remote_generation(RegistryGeneration(2), &["embedding_models".into()]));
        assert!(s.conflict.is_some());
        assert!(!s.loading);

        let mut s2 = RetrievalSettingsState::new();
        s2.apply_snapshot(snap_with_emb());
        assert!(s2.on_remote_generation(RegistryGeneration(2), &["prime".into()]));
        assert!(s2.loading);
        assert!(s2.conflict.is_none());
    }

    #[test]
    fn unit_stale_op_id_and_none_discarded() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        s.pending_operation_id = Some("op-a".into());
        s.apply_mutation_result(RetrievalMutationResult {
            ok: true,
            generation: RegistryGeneration(2),
            error: None,
            stale: false,
            guidance: None,
            conflict: None,
            changed_fields: vec![],
            operation_id: Some("op-b".into()),
            memory_reindex: None,
            snapshot: None,
        });
        assert_eq!(s.generation.get(), 1);
        assert_eq!(s.pending_operation_id.as_deref(), Some("op-a"));

        // None echo discarded under strict Gate E matching.
        s.apply_mutation_result(RetrievalMutationResult {
            ok: true,
            generation: RegistryGeneration(9),
            error: None,
            stale: false,
            guidance: None,
            conflict: None,
            changed_fields: vec![],
            operation_id: None,
            memory_reindex: None,
            snapshot: None,
        });
        assert_eq!(s.generation.get(), 1);
        assert_eq!(s.pending_operation_id.as_deref(), Some("op-a"));
    }

    #[test]
    fn unit_add_edit_stamps_generation_and_op_id() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        s.handle_key(key(KeyCode::Char('a')));
        s.handle_key(key(KeyCode::Esc)); // leave value editing, stay in form
        assert!(matches!(
            s.edit,
            RetrievalEditMode::EditFields { is_new: true, .. }
        ));
        if let RetrievalEditMode::EditFields { fields, .. } = &mut s.edit {
            for (k, v) in fields.iter_mut() {
                match k.as_str() {
                    "id" => *v = "e2".into(),
                    "provider" => *v = "lab".into(),
                    "model" => *v = "m2".into(),
                    _ => {}
                }
            }
        }
        let cmd = s.handle_key(key(KeyCode::Char('s')));
        match cmd {
            Some(RetrievalCommand::UpsertEmbedding {
                id,
                expected_generation,
                operation_id,
                confirm_memory_reindex,
                ..
            }) => {
                assert_eq!(id, "e2");
                assert_eq!(expected_generation.get(), 1);
                assert!(!operation_id.is_empty());
                assert!(!confirm_memory_reindex);
                assert_eq!(
                    s.pending_operation_id.as_deref(),
                    Some(operation_id.as_str())
                );
                assert!(s.pending_reindex_command.is_some());
            }
            other => panic!("expected stamped upsert, got {other:?}"),
        }
    }

    #[test]
    fn unit_confirm_reindex_retries_exact_draft() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        let draft = RetrievalCommand::SaveMemoryProfile {
            profile: Some("p1".into()),
            expected_generation: RegistryGeneration(1),
            confirm_memory_reindex: false,
            operation_id: "op-mem".into(),
        };
        s.pending_reindex_command = Some(Box::new(draft));
        s.pending_operation_id = Some("op-mem".into());
        s.edit = RetrievalEditMode::ConfirmMemoryReindex {
            impact: MemoryReindexImpact {
                requires_confirmation: true,
                reason: "test".into(),
                previous_fingerprint: None,
                next_fingerprint: Some("x".into()),
            },
        };
        let cmd = s.handle_key(key(KeyCode::Char('y')));
        match cmd {
            Some(RetrievalCommand::SaveMemoryProfile {
                profile,
                expected_generation,
                confirm_memory_reindex,
                operation_id,
            }) => {
                assert_eq!(profile.as_deref(), Some("p1"));
                assert_eq!(expected_generation.get(), 1);
                assert!(confirm_memory_reindex);
                // New op-id stamped for the retry.
                assert!(!operation_id.is_empty());
            }
            other => panic!("expected confirmed memory save, got {other:?}"),
        }
    }

    #[test]
    fn empty_list_render_does_not_panic() {
        let mut s = RetrievalSettingsState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        let theme = Theme::current();
        s.render(Rect::new(0, 0, 80, 24), &mut buf, &theme);
    }

    #[test]
    fn long_id_truncated_in_list() {
        let long = "a".repeat(80);
        let mut snap = snap_with_emb();
        snap.embedding_models[0].id = long;
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 12));
        s.render(Rect::new(0, 0, 40, 12), &mut buf, &Theme::current());
    }

    #[test]
    fn unit_confirm_delete_stamps_cas() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        s.handle_key(key(KeyCode::Char('d')));
        assert!(matches!(s.edit, RetrievalEditMode::ConfirmDelete { .. }));
        let cmd = s.handle_key(key(KeyCode::Char('y')));
        match cmd {
            Some(RetrievalCommand::DeleteEntity {
                id,
                expected_generation,
                operation_id,
                ..
            }) => {
                assert_eq!(id, "e1");
                assert_eq!(expected_generation.get(), 1);
                assert!(!operation_id.is_empty());
            }
            other => panic!("expected delete, got {other:?}"),
        }
    }

    #[test]
    fn unit_browse_s_is_validate_not_save_graph() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        let cmd = s.handle_key(key(KeyCode::Char('s')));
        assert!(matches!(cmd, Some(RetrievalCommand::ValidateAndReload)));
    }

    /// Confirmed reindex retry that fails/stales must not leave a sticky
    /// pre-confirm on the next unrelated mutation (Round 2 Issue 13).
    #[test]
    fn unit_confirmed_retry_failure_does_not_preconfirm_next_mutation() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        let draft = RetrievalCommand::SaveMemoryProfile {
            profile: Some("p1".into()),
            expected_generation: RegistryGeneration(1),
            confirm_memory_reindex: false,
            operation_id: "op-mem".into(),
        };
        s.pending_reindex_command = Some(Box::new(draft));
        s.pending_operation_id = Some("op-mem".into());
        s.edit = RetrievalEditMode::ConfirmMemoryReindex {
            impact: MemoryReindexImpact {
                requires_confirmation: true,
                reason: "test".into(),
                previous_fingerprint: None,
                next_fingerprint: Some("x".into()),
            },
        };
        let confirmed = s.handle_key(key(KeyCode::Char('y')));
        let confirmed_op = match confirmed {
            Some(RetrievalCommand::SaveMemoryProfile {
                confirm_memory_reindex: true,
                operation_id,
                profile,
                expected_generation,
            }) => {
                assert_eq!(profile.as_deref(), Some("p1"));
                assert_eq!(expected_generation.get(), 1);
                operation_id
            }
            other => panic!("expected confirmed memory save, got {other:?}"),
        };
        // Simulate confirmed retry terminal failure (validation / I/O).
        s.apply_mutation_result(RetrievalMutationResult {
            ok: false,
            generation: RegistryGeneration(1),
            error: Some("validation hard-error".into()),
            stale: false,
            guidance: None,
            conflict: None,
            changed_fields: vec![],
            operation_id: Some(confirmed_op),
            memory_reindex: None,
            snapshot: None,
        });
        assert!(s.pending_reindex_command.is_none());

        // Next unrelated mutation must not be pre-confirmed.
        s.handle_key(key(KeyCode::Char('a')));
        s.handle_key(key(KeyCode::Esc)); // leave value editing, stay in form
        if let RetrievalEditMode::EditFields { fields, .. } = &mut s.edit {
            for (k, v) in fields.iter_mut() {
                match k.as_str() {
                    "id" => *v = "e-next".into(),
                    "provider" => *v = "lab".into(),
                    "model" => *v = "m-next".into(),
                    _ => {}
                }
            }
        }
        let next = s.handle_key(key(KeyCode::Char('s')));
        match next {
            Some(RetrievalCommand::UpsertEmbedding {
                confirm_memory_reindex,
                id,
                ..
            }) => {
                assert_eq!(id, "e-next");
                assert!(
                    !confirm_memory_reindex,
                    "next mutation must still hit the reindex barrier"
                );
            }
            other => panic!("expected unconfirmed upsert, got {other:?}"),
        }

        // Stale path also clears stash and must not pre-confirm.
        let mut s2 = RetrievalSettingsState::new();
        s2.apply_snapshot(snap_with_emb());
        let draft2 = RetrievalCommand::UpsertEmbedding {
            id: "e1".into(),
            config: EmbeddingModelConfig {
                provider: "lab".into(),
                model: "m".into(),
                dimensions: Some(64),
                ..Default::default()
            },
            expected_generation: RegistryGeneration(1),
            confirm_memory_reindex: true,
            operation_id: "op-stale".into(),
        };
        s2.pending_reindex_command = Some(Box::new(draft2));
        s2.pending_operation_id = Some("op-stale".into());
        s2.apply_mutation_result(RetrievalMutationResult {
            ok: false,
            generation: RegistryGeneration(2),
            error: Some("stale".into()),
            stale: true,
            guidance: None,
            conflict: None,
            changed_fields: vec![],
            operation_id: Some("op-stale".into()),
            memory_reindex: None,
            snapshot: None,
        });
        assert!(s2.pending_reindex_command.is_none());
        s2.handle_key(key(KeyCode::Char('a')));
        s2.handle_key(key(KeyCode::Esc)); // leave value editing, stay in form
        if let RetrievalEditMode::EditFields { fields, .. } = &mut s2.edit {
            for (k, v) in fields.iter_mut() {
                match k.as_str() {
                    "id" => *v = "e-stale".into(),
                    "provider" => *v = "lab".into(),
                    "model" => *v = "m2".into(),
                    _ => {}
                }
            }
        }
        match s2.handle_key(key(KeyCode::Char('s'))) {
            Some(RetrievalCommand::UpsertEmbedding {
                confirm_memory_reindex: false,
                ..
            }) => {}
            other => panic!("stale terminal must not pre-confirm next: {other:?}"),
        }
    }

    /// Fixed enum rows are not free-form editable (Round 2 Issue 12).
    #[test]
    fn unit_fixed_enum_rows_not_editable() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        s.handle_key(key(KeyCode::Char('a')));
        s.handle_key(key(KeyCode::Esc)); // leave value editing, stay in form
        let (proto_idx, dims_idx) = match &s.edit {
            RetrievalEditMode::EditFields { fields, .. } => {
                let p = fields
                    .iter()
                    .position(|(k, _)| k == "protocol (fixed)")
                    .expect("protocol fixed row");
                let d = fields
                    .iter()
                    .position(|(k, _)| k == "dimensions")
                    .expect("dimensions row");
                assert!(is_fixed_field_label("protocol (fixed)"));
                assert!(!is_fixed_field_label("dimensions"));
                (p, d)
            }
            other => panic!("expected edit fields, got {other:?}"),
        };
        // Land on protocol (fixed) then Enter must not open free-form edit.
        if let RetrievalEditMode::EditFields { field_idx, .. } = &mut s.edit {
            *field_idx = proto_idx;
        }
        s.handle_key(key(KeyCode::Enter));
        assert!(
            matches!(
                s.edit,
                RetrievalEditMode::EditFields {
                    editing_value: false,
                    ..
                }
            ),
            "fixed row must not enter free-form edit"
        );
        // j from the field before fixed should skip over it.
        if let RetrievalEditMode::EditFields { field_idx, .. } = &mut s.edit {
            *field_idx = proto_idx.saturating_sub(1);
        }
        s.handle_key(key(KeyCode::Char('j')));
        match &s.edit {
            RetrievalEditMode::EditFields {
                field_idx, fields, ..
            } => {
                assert_ne!(*field_idx, proto_idx, "j must skip fixed protocol row");
                assert_eq!(*field_idx, dims_idx);
                assert!(!is_fixed_field_label(&fields[*field_idx].0));
            }
            other => panic!("expected edit fields, got {other:?}"),
        }
        // Profile fallback_strategy (fixed) same contract.
        s.edit = RetrievalEditMode::Browse;
        s.page = RetrievalPage::Profiles;
        s.handle_key(key(KeyCode::Char('a')));
        s.handle_key(key(KeyCode::Esc)); // leave value editing, stay in form
        if let RetrievalEditMode::EditFields {
            fields, field_idx, ..
        } = &mut s.edit
        {
            let fb = fields
                .iter()
                .position(|(k, _)| k == "fallback_strategy (fixed)")
                .expect("fallback fixed row");
            *field_idx = fb;
        }
        s.handle_key(key(KeyCode::Char('e')));
        assert!(matches!(
            s.edit,
            RetrievalEditMode::EditFields {
                editing_value: false,
                ..
            }
        ));
    }

    /// `a` must arm value editing on the first field so typing works
    /// immediately; Enter confirms the value and advances the wizard.
    #[test]
    fn add_starts_typing_immediately_and_enter_advances() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        s.page = RetrievalPage::EmbeddingModels;
        s.handle_key(key(KeyCode::Char('a')));
        let (editing, idx0) = match &s.edit {
            RetrievalEditMode::EditFields {
                editing_value,
                field_idx,
                ..
            } => (*editing_value, *field_idx),
            other => panic!("expected edit fields, got {other:?}"),
        };
        assert!(editing, "a must arm value editing");
        assert_eq!(idx0, 0, "must start on the id field");
        s.handle_key(key(KeyCode::Char('x')));
        assert_eq!(s.line_editor.text(), "x", "typing must land in the editor");

        s.handle_key(key(KeyCode::Enter));
        match &s.edit {
            RetrievalEditMode::EditFields {
                fields,
                field_idx,
                editing_value,
                ..
            } => {
                assert_eq!(fields[0].1, "x", "Enter must commit the typed value");
                assert_eq!(*field_idx, 1, "Enter must advance to provider");
                assert!(*editing_value, "wizard keeps editing the next field");
            }
            other => panic!("expected edit fields, got {other:?}"),
        }
    }

    /// Enter on the last editable field leaves value editing so `s` commits.
    #[test]
    fn enter_on_last_field_ends_value_editing() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        s.page = RetrievalPage::EmbeddingModels;
        s.handle_key(key(KeyCode::Char('a')));
        s.handle_key(key(KeyCode::Esc));
        let last_idx = match &s.edit {
            RetrievalEditMode::EditFields { fields, .. } => fields
                .iter()
                .rposition(|(k, _)| !is_fixed_field_label(k))
                .expect("editable field"),
            other => panic!("expected edit fields, got {other:?}"),
        };
        if let RetrievalEditMode::EditFields { field_idx, .. } = &mut s.edit {
            *field_idx = last_idx;
        }
        let before = match &s.edit {
            RetrievalEditMode::EditFields { fields, .. } => fields[last_idx].1.clone(),
            other => panic!("expected edit fields, got {other:?}"),
        };
        s.handle_key(key(KeyCode::Char('e')));
        s.handle_key(key(KeyCode::Enter));
        match &s.edit {
            RetrievalEditMode::EditFields {
                fields,
                editing_value,
                ..
            } => {
                assert!(!editing_value, "last field must end value editing");
                assert_eq!(
                    fields[last_idx].1, before,
                    "value must be preserved through commit"
                );
            }
            other => panic!("expected edit fields, got {other:?}"),
        }
    }

    /// The add form shows visible placeholders and per-mode footer hints.
    #[test]
    fn add_form_renders_placeholders_and_edit_hints() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        s.page = RetrievalPage::EmbeddingModels;
        s.handle_key(key(KeyCode::Char('a')));
        let labels: Vec<&str> = s.footer_shortcuts().iter().map(|sc| sc.label).collect();
        assert!(
            labels.contains(&"Enter Accept + next field"),
            "value-editing footer must advertise Enter confirm: {labels:?}"
        );
        s.handle_key(key(KeyCode::Esc));
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
        s.render(Rect::new(0, 0, 100, 30), &mut buf, &Theme::current());
        let text = buffer_text(&buf);
        assert!(text.contains("Add embedding"), "{text}");
        assert!(
            text.contains("(empty)"),
            "empty editable fields must show a placeholder, got:\n{text}"
        );
        assert!(
            text.contains("e edit its value"),
            "form body must show the edit hint, got:\n{text}"
        );
    }

    #[test]
    fn unit_footer_disambiguates_preview_and_refresh() {
        let s = RetrievalSettingsState::new();
        let labels: Vec<&str> = s.footer_shortcuts().iter().map(|sc| sc.label).collect();
        assert!(
            labels.contains(&"v Preview"),
            "footer must label synthetic validate as Preview: {labels:?}"
        );
        assert!(
            labels.contains(&"s Refresh"),
            "footer must label LoadSnapshot path as Refresh: {labels:?}"
        );
        assert!(
            !labels
                .iter()
                .any(|l| *l == "v Validate" || *l == "s Validate"),
            "duplicate Validate labels removed: {labels:?}"
        );
    }

    #[test]
    fn esc_in_edit_fields_returns_to_browse_not_a_command() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        s.handle_key(key(KeyCode::Char('e')));
        assert!(
            matches!(s.edit, RetrievalEditMode::EditFields { .. }),
            "e must open EditFields, got {:?}",
            s.edit
        );
        assert!(s.owns_escape());
        // `e` arms value editing: first Esc leaves typing (stays in the form),
        // second Esc returns to Browse. Neither dispatches a command.
        assert!(s.handle_key(key(KeyCode::Esc)).is_none());
        assert!(matches!(
            s.edit,
            RetrievalEditMode::EditFields {
                editing_value: false,
                ..
            }
        ));
        assert!(s.handle_key(key(KeyCode::Esc)).is_none());
        assert!(
            matches!(s.edit, RetrievalEditMode::Browse),
            "Esc in EditFields must return to Browse, got {:?}",
            s.edit
        );
        assert!(!s.owns_escape());
    }

    #[test]
    fn esc_in_confirm_prime_rebuild_returns_to_browse() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        s.page = RetrievalPage::Prime;
        s.prime_index = Some(xai_grok_shell::session::prime::PrimeIndexStatus {
            api_version: 1,
            generation: 1,
            fingerprint_short: "abc123def456".into(),
            skills: xai_grok_shell::session::prime::PrimeIndexCollectionStatus {
                collection: "skills".into(),
                generation: 1,
                fingerprint_short: "abc123def456".into(),
                item_count: 1,
                vector_count: 0,
                missing_vectors: 1,
                readiness: "pending".into(),
                route_id: Some("main".into()),
                dimensions: None,
            },
            agents: xai_grok_shell::session::prime::PrimeIndexCollectionStatus {
                collection: "agents".into(),
                generation: 0,
                fingerprint_short: String::new(),
                item_count: 0,
                vector_count: 0,
                missing_vectors: 0,
                readiness: "ready".into(),
                route_id: None,
                dimensions: None,
            },
            job: None,
            configured_route: Some("main".into()),
            capabilities: xai_grok_shell::session::prime::PrimeIndexCapabilities::SUPPORTED,
            unchanged: false,
        });
        s.handle_key(key(KeyCode::Char('u')));
        assert!(
            matches!(s.edit, RetrievalEditMode::ConfirmPrimeRebuild { .. }),
            "u must open ConfirmPrimeRebuild, got {:?}",
            s.edit
        );
        assert!(s.owns_escape());
        assert!(s.handle_key(key(KeyCode::Esc)).is_none());
        assert!(
            matches!(s.edit, RetrievalEditMode::Browse),
            "Esc in ConfirmPrimeRebuild must return to Browse, got {:?}",
            s.edit
        );
    }

    #[test]
    fn esc_in_conflict_keeps_draft() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        s.dirty = true;
        s.conflict = Some(RetrievalConflictInfo {
            client_generation: RegistryGeneration(1),
            live_generation: RegistryGeneration(2),
            changed_fields: vec!["embedding_models".into()],
            guidance: "reload or keep".into(),
        });
        assert!(s.owns_escape());
        match s.handle_key(key(KeyCode::Esc)) {
            Some(RetrievalCommand::DismissConflictKeepDraft) => {}
            other => panic!("conflict Esc must keep draft, got {other:?}"),
        }
        assert!(s.conflict.is_none());
        assert!(s.dirty);
    }
}
