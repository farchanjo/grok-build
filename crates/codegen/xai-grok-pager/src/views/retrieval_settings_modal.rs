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
#[derive(Debug, Clone, PartialEq)]
pub enum RetrievalCommand {
    Reload,
    SaveGraph,
    ValidatePreview {
        kind: String,
        id: String,
    },
    UpsertEmbedding {
        id: String,
        config: EmbeddingModelConfig,
    },
    UpsertReranker {
        id: String,
        config: RerankerModelConfig,
    },
    UpsertProfile {
        id: String,
        config: RetrievalProfileConfig,
    },
    CloneEntity {
        kind: String,
        source_id: String,
        new_id: String,
    },
    DeleteEntity {
        kind: String,
        id: String,
    },
    Reorder {
        kind: String,
        ordered_ids: Vec<String>,
    },
    SavePrime {
        prime: PrimeConfig,
    },
    SaveMemoryProfile {
        profile: Option<String>,
        confirm_reindex: bool,
    },
    ConfirmMemoryReindex,
    DismissConflictReload,
    DismissConflictKeepDraft,
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
    pub pending_memory_confirm: bool,
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
            pending_memory_confirm: false,
        }
    }

    fn next_op_id(&mut self) -> String {
        self.op_counter = self.op_counter.saturating_add(1);
        format!("retrieval-op-{}", self.op_counter)
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

    /// Multi-client generation update: dirty → conflict; clean → auto-reload flag.
    pub fn on_remote_generation(&mut self, live: RegistryGeneration, changed: &[String]) {
        if live.get() <= self.generation.get() {
            return;
        }
        if self.dirty || !matches!(self.edit, RetrievalEditMode::Browse) {
            self.conflict = Some(RetrievalConflictInfo {
                client_generation: self.generation,
                live_generation: live,
                changed_fields: changed.to_vec(),
                guidance: "Another client updated the retrieval graph. Reload to discard local \
                           edits, or keep draft and re-save (may fail stale)."
                    .into(),
            });
            self.status = Some("Conflict: remote graph advanced".into());
        } else {
            self.loading = true;
            self.status = Some("Remote update — reloading".into());
        }
    }

    pub fn apply_mutation_result(&mut self, result: RetrievalMutationResult) {
        if let (Some(pending), Some(echo)) = (
            self.pending_operation_id.as_deref(),
            result.operation_id.as_deref(),
        ) && pending != echo
        {
            return;
        }
        self.pending_operation_id = None;
        if result.stale {
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
                return;
            }
            self.error = result.error;
            self.status = Some("Save failed".into());
            return;
        }
        if let Some(snap) = result.snapshot {
            self.apply_snapshot(snap);
        } else {
            self.generation = result.generation;
            self.dirty = false;
            self.loading = true;
        }
        self.edit = RetrievalEditMode::Browse;
        self.status = Some("Saved".into());
        self.pending_memory_confirm = false;
    }

    pub fn apply_preview(&mut self, preview: RetrievalPreviewResult) {
        if let (Some(pending), Some(echo)) = (
            self.pending_operation_id.as_deref(),
            preview.operation_id.as_deref(),
        ) && pending != echo
        {
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
                    Some(RetrievalCommand::DeleteEntity { kind, id })
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.edit = RetrievalEditMode::Browse;
                    None
                }
                _ => None,
            },
            RetrievalEditMode::ConfirmMemoryReindex { .. } => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.pending_memory_confirm = true;
                    self.edit = RetrievalEditMode::Browse;
                    Some(RetrievalCommand::ConfirmMemoryReindex)
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.edit = RetrievalEditMode::Browse;
                    self.pending_memory_confirm = false;
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
                        Some(RetrievalCommand::CloneEntity {
                            kind,
                            source_id,
                            new_id,
                        })
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
            KeyCode::Char('r') => Some(RetrievalCommand::Reload),
            KeyCode::Char('s') => Some(RetrievalCommand::SaveGraph),
            KeyCode::Char('v') => {
                let (kind, id) = self.selected_entity();
                let op = self.next_op_id();
                self.pending_operation_id = Some(op);
                Some(RetrievalCommand::ValidatePreview { kind, id })
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
        Some(RetrievalCommand::Reorder {
            kind: kind.into(),
            ordered_ids: ids,
        })
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
        match self.page {
            RetrievalPage::EmbeddingModels => {
                self.edit = RetrievalEditMode::EditFields {
                    kind: "embedding".into(),
                    id: String::new(),
                    is_new: true,
                    fields: default_embedding_fields(None),
                    field_idx: 0,
                    editing_value: false,
                };
            }
            RetrievalPage::Rerankers => {
                self.edit = RetrievalEditMode::EditFields {
                    kind: "reranker".into(),
                    id: String::new(),
                    is_new: true,
                    fields: default_reranker_fields(None),
                    field_idx: 0,
                    editing_value: false,
                };
            }
            RetrievalPage::Profiles => {
                self.edit = RetrievalEditMode::EditFields {
                    kind: "profile".into(),
                    id: String::new(),
                    is_new: true,
                    fields: default_profile_fields(None),
                    field_idx: 0,
                    editing_value: false,
                };
            }
            RetrievalPage::Prime => {
                self.edit = RetrievalEditMode::EditFields {
                    kind: "prime".into(),
                    id: if self.selected == 0 {
                        "skills".into()
                    } else {
                        "agents".into()
                    },
                    is_new: false,
                    fields: prime_fields(&self.draft_prime, self.selected == 0),
                    field_idx: 0,
                    editing_value: false,
                };
            }
            RetrievalPage::Memory => {
                self.edit = RetrievalEditMode::EditFields {
                    kind: "memory".into(),
                    id: "selection".into(),
                    is_new: false,
                    fields: vec![(
                        "retrieval_profile".into(),
                        self.draft_memory_profile.clone().unwrap_or_default(),
                    )],
                    field_idx: 0,
                    editing_value: false,
                };
            }
            RetrievalPage::Validate => {}
        }
    }

    fn begin_edit_selected(&mut self) {
        let Some(snap) = &self.snapshot else {
            return;
        };
        match self.page {
            RetrievalPage::EmbeddingModels => {
                if let Some(e) = snap.embedding_models.get(self.selected) {
                    self.edit = RetrievalEditMode::EditFields {
                        kind: "embedding".into(),
                        id: e.id.clone(),
                        is_new: false,
                        fields: default_embedding_fields(Some(e)),
                        field_idx: 0,
                        editing_value: false,
                    };
                }
            }
            RetrievalPage::Rerankers => {
                if let Some(e) = snap.reranker_models.get(self.selected) {
                    self.edit = RetrievalEditMode::EditFields {
                        kind: "reranker".into(),
                        id: e.id.clone(),
                        is_new: false,
                        fields: default_reranker_fields(Some(e)),
                        field_idx: 0,
                        editing_value: false,
                    };
                }
            }
            RetrievalPage::Profiles => {
                if let Some(e) = snap.retrieval_profiles.get(self.selected) {
                    self.edit = RetrievalEditMode::EditFields {
                        kind: "profile".into(),
                        id: e.id.clone(),
                        is_new: false,
                        fields: default_profile_fields(Some(e)),
                        field_idx: 0,
                        editing_value: false,
                    };
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
                        if let Some(f) = fields.get_mut(*field_idx) {
                            f.1 = self.line_editor.text().to_owned();
                        }
                        *editing_value = false;
                        self.line_editor.reset();
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
                if let RetrievalEditMode::EditFields { field_idx, .. } = &mut self.edit
                    && *field_idx > 0
                {
                    *field_idx -= 1;
                }
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let RetrievalEditMode::EditFields {
                    fields, field_idx, ..
                } = &mut self.edit
                    && *field_idx + 1 < fields.len()
                {
                    *field_idx += 1;
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
                    if let Some(f) = fields.get(*field_idx) {
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
                Some(RetrievalCommand::UpsertEmbedding { id: eid, config })
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
                Some(RetrievalCommand::UpsertReranker { id: rid, config })
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
                Some(RetrievalCommand::UpsertProfile { id: pid, config })
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
                Some(RetrievalCommand::SavePrime {
                    prime: self.draft_prime.clone(),
                })
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
                Some(RetrievalCommand::SaveMemoryProfile {
                    profile,
                    confirm_reindex: self.pending_memory_confirm,
                })
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
                    label: "v Validate",
                    clickable: false,
                    id: 7,
                },
                Shortcut {
                    label: "s Save",
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
                    label: "Enter Accept",
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
                let label = format!(" {}{} ", i + 1, p.label());
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
                format!("Error: {err}"),
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
                status.as_str(),
                Style::default().fg(theme.accent_user),
            )));
        }

        match &self.edit {
            RetrievalEditMode::Browse => self.render_browse(&mut lines, theme),
            RetrievalEditMode::EditFields {
                kind,
                id,
                is_new,
                fields,
                field_idx,
                editing_value,
            } => {
                lines.push(Line::from(format!(
                    "{} {} `{}`",
                    if *is_new { "Add" } else { "Edit" },
                    kind,
                    if id.is_empty() { "(new)" } else { id }
                )));
                for (i, (k, v)) in fields.iter().enumerate() {
                    let mark = if i == *field_idx { ">" } else { " " };
                    let val = if i == *field_idx && *editing_value {
                        format!("{}█", self.line_editor.text())
                    } else {
                        truncate_id(v, area.width.saturating_sub(24) as usize)
                    };
                    lines.push(Line::from(format!("{mark} {k}: {val}")));
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

    fn render_browse(&self, lines: &mut Vec<Line>, theme: &Theme) {
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
                let skills_mark = if self.selected == 0 { ">" } else { " " };
                let agents_mark = if self.selected == 1 { ">" } else { " " };
                lines.push(Line::from(format!(
                    "{skills_mark} skills  enabled={} profile={:?} results={} degrade={}",
                    snap.prime.skills.enabled,
                    snap.prime.skills.retrieval_profile,
                    snap.prime.skills.max_results,
                    snap.prime.skills.degrade_on_error
                )));
                lines.push(Line::from(format!(
                    "{agents_mark} agents  enabled={} profile={:?} results={} degrade={}",
                    snap.prime.agents.enabled,
                    snap.prime.agents.retrieval_profile,
                    snap.prime.agents.max_results,
                    snap.prime.agents.degrade_on_error
                )));
            }
            RetrievalPage::Memory => {
                lines.push(Line::from(format!(
                    "> retrieval_profile = {:?}",
                    snap.memory_retrieval_profile
                )));
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

fn default_embedding_fields(existing: Option<&EmbeddingModelDto>) -> Vec<(String, String)> {
    match existing {
        None => vec![
            ("id".into(), String::new()),
            ("provider".into(), String::new()),
            ("model".into(), String::new()),
            ("protocol".into(), "openai_compatible".into()),
            ("dimensions".into(), String::new()),
            ("encoding".into(), "float".into()),
            ("batch_size".into(), "32".into()),
            ("max_input_tokens".into(), "8192".into()),
        ],
        Some(e) => vec![
            ("id".into(), e.id.clone()),
            ("provider".into(), e.config.provider.clone()),
            ("model".into(), e.config.model.clone()),
            ("protocol".into(), e.config.protocol.as_str().into()),
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
            ("fallback_strategy".into(), "deterministic".into()),
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
                "fallback_strategy".into(),
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
    fn dirty_conflict_vs_clean_reload() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        s.dirty = true;
        s.on_remote_generation(RegistryGeneration(2), &["embedding_models".into()]);
        assert!(s.conflict.is_some());
        assert!(!s.loading);

        let mut s2 = RetrievalSettingsState::new();
        s2.apply_snapshot(snap_with_emb());
        s2.on_remote_generation(RegistryGeneration(2), &["prime".into()]);
        assert!(s2.loading);
        assert!(s2.conflict.is_none());
    }

    #[test]
    fn stale_op_id_discarded() {
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
    }

    #[test]
    fn add_edit_flow_emits_upsert() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        s.handle_key(key(KeyCode::Char('a')));
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
        assert!(matches!(
            cmd,
            Some(RetrievalCommand::UpsertEmbedding { id, .. }) if id == "e2"
        ));
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
    fn confirm_delete_flow() {
        let mut s = RetrievalSettingsState::new();
        s.apply_snapshot(snap_with_emb());
        s.handle_key(key(KeyCode::Char('d')));
        assert!(matches!(s.edit, RetrievalEditMode::ConfirmDelete { .. }));
        let cmd = s.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(
            cmd,
            Some(RetrievalCommand::DeleteEntity { id, .. }) if id == "e1"
        ));
    }
}
