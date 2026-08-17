//! Provider management modal (`/providers`).
//!
//! This module deliberately owns only presentation state. API keys never leave
//! [`ProviderModalState`] through an [`crate::app::actions::Action`]: the
//! backend integration should call [`ProviderModalState::take_submitted_secret`]
//! immediately before starting its secret-store/connect effect. The resulting
//! status is applied with [`ProviderModalState::set_status`]. This keeps keys
//! out of the reducer's debug output, telemetry, config files, and task names.

#[cfg(test)]
use crossterm::event::KeyModifiers;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::input::line_editor::{LineEditOutcome, LineEditor};
use crate::theme::Theme;
use crate::views::modal_window::{
    self, ModalSizing, ModalWindowConfig, ModalWindowState, Shortcut,
};

/// Provider displayed by the management surface.
///
/// Built-ins remain first-class. Configured OpenAI-compatible instances are
/// owned rows identified by a validated slug (`Configured`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    Xai,
    OpenAi,
    OpenRouter,
    Anthropic,
    /// User-configured provider id (e.g. `local_vllm`, `zai-model-api`).
    Configured(String),
}

impl ProviderKind {
    /// Peer order: existing built-ins, then Anthropic, then configured rows.
    pub const BUILTINS: [Self; 4] = [Self::Xai, Self::OpenAi, Self::OpenRouter, Self::Anthropic];

    /// Default browse list before configured providers are loaded.
    pub const ALL: [Self; 4] = Self::BUILTINS;

    pub fn label(&self) -> String {
        match self {
            Self::Xai => "xAI".into(),
            Self::OpenAi => "OpenAI".into(),
            Self::OpenRouter => "OpenRouter".into(),
            Self::Anthropic => "Anthropic".into(),
            Self::Configured(id) => id.clone(),
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Self::Xai => "Grok/xAI account (OAuth) or xAI API key".into(),
            Self::OpenAi => "ChatGPT OAuth or API key · Responses".into(),
            Self::OpenRouter => "Chat Completions · key stored securely".into(),
            Self::Anthropic => "Native Grok agent loop · x-api-key stored securely".into(),
            Self::Configured(id) if id == "zai-model-api" || id == "zai" => {
                "Z.ai Model API · Chat Completions · key stored securely".into()
            }
            Self::Configured(_) => {
                "OpenAI-compatible · app/admin keys · catalog & capabilities".into()
            }
        }
    }

    pub fn needs_api_key(&self) -> bool {
        matches!(
            self,
            Self::OpenAi | Self::OpenRouter | Self::Anthropic | Self::Configured(_)
        )
    }

    pub fn id_str(&self) -> &str {
        match self {
            Self::Xai => "xai",
            Self::OpenAi => "openai",
            Self::OpenRouter => "openrouter",
            Self::Anthropic => "anthropic",
            Self::Configured(id) => id.as_str(),
        }
    }

    pub fn is_built_in(&self) -> bool {
        !matches!(self, Self::Configured(_))
    }
}

/// The externally observable state of a provider connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderStatus {
    Missing,
    Connecting,
    Connected { detail: Option<String> },
    Error(String),
}

impl ProviderStatus {
    fn label(&self) -> &str {
        match self {
            Self::Missing => "Key/connect missing",
            Self::Connecting => "Checking…",
            Self::Connected { .. } => "Connected",
            Self::Error(_) => "Connection error",
        }
    }
}

/// Experimental subscription-backed Claude CLI mode shown within the single
/// Anthropic provider card. This is deliberately separate from the Messages
/// API status because it never uses the Anthropic API key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeCliStatus {
    Checking,
    Ready {
        version: String,
        auth_summary: String,
    },
    AuthRequired {
        version: String,
        detail: String,
    },
    AuthUnknown {
        version: String,
        detail: String,
    },
    FeatureNotCompiled,
    OptInMissing,
    ProbeFailed(String),
}

impl ClaudeCliStatus {
    fn label(&self) -> &str {
        match self {
            Self::Checking => "Checking…",
            Self::Ready { .. } => "Ready",
            Self::AuthRequired { .. } => "Login required",
            Self::AuthUnknown { .. } => "Auth check failed",
            Self::FeatureNotCompiled => "Not compiled",
            Self::OptInMissing => "Opt-in required",
            Self::ProbeFailed(_) => "Unavailable",
        }
    }
}

/// Safe reducer intent. It contains no credentials; see the module docs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderCommand {
    Connect(ProviderKind),
    ReplaceKey(ProviderKind),
    Test(ProviderKind),
    Disconnect(ProviderKind),
    LoginCodex,
    LogoutCodex,
    LoginXai,
    LogoutXai,
    RefreshStatus(ProviderKind),
    /// Open the add-provider editor for a new OpenAI-compatible instance.
    AddConfigured,
    /// Refresh catalog for the selected configured provider.
    RefreshCatalog(ProviderKind),
    /// Refresh capability profile for the selected provider.
    RefreshCapabilities(ProviderKind),
    /// Load shell-authoritative list snapshot (generation-tagged).
    LoadListSnapshot,
    /// Open typed editor for selected provider (shell detail).
    OpenEditor {
        provider_id: String,
    },
    /// Enable selected configured provider.
    Enable {
        provider_id: String,
    },
    /// Disable selected configured provider.
    Disable {
        provider_id: String,
    },
    /// Clone selected provider (metadata only).
    Clone {
        source_id: String,
        new_id: String,
    },
    /// Editor-originated management ops (generation-tagged in effects).
    Editor(crate::views::provider_editor::EditorCommand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum XaiChoiceAction {
    Connect,
    Disconnect,
}

#[derive(Clone)]
pub(crate) enum ProviderModalMode {
    Browse,
    ChoosingXai {
        action: XaiChoiceAction,
        selected: usize,
    },
    ChoosingOpenAi {
        selected: usize,
    },
    EditingKey {
        provider: ProviderKind,
        editor: LineEditor,
    },
    /// Typed multi-page editor for one provider instance.
    Editor(Box<crate::views::provider_editor::ProviderEditorState>),
    /// Add-provider wizard (id + base URL + kind).
    Adding {
        step: AddStep,
        id_editor: LineEditor,
        url_editor: LineEditor,
        kind_index: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AddStep {
    Id,
    BaseUrl,
    Kind,
    Confirm,
}

/// Kinds offered when adding unlimited OpenAI/OpenRouter/custom instances.
pub(crate) const ADD_KINDS: [&str; 3] = ["openai_compatible", "openrouter", "openai"];

impl std::fmt::Debug for ProviderModalMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Browse => f.write_str("Browse"),
            Self::ChoosingXai { action, selected } => f
                .debug_struct("ChoosingXai")
                .field("action", action)
                .field("selected", selected)
                .finish(),
            Self::ChoosingOpenAi { selected } => f
                .debug_struct("ChoosingOpenAi")
                .field("selected", selected)
                .finish(),
            Self::EditingKey { provider, .. } => f
                .debug_struct("EditingKey")
                .field("provider", provider)
                .field("editor", &"[REDACTED]")
                .finish(),
            Self::Editor(state) => f.debug_tuple("Editor").field(state).finish(),
            Self::Adding {
                step, kind_index, ..
            } => f
                .debug_struct("Adding")
                .field("step", step)
                .field("kind_index", kind_index)
                .field("id_editor", &"[REDACTED_INPUT]")
                .field("url_editor", &"[REDACTED_INPUT]")
                .finish(),
        }
    }
}

/// State of the provider cards and API-key entry surface.
#[derive(Clone)]
pub struct ProviderModalState {
    pub window: ModalWindowState,
    pub selected: usize,
    /// Dynamic row list: built-ins first, then configured providers.
    pub rows: Vec<ProviderKind>,
    pub statuses: Vec<ProviderStatus>,
    pub claude_cli_status: ClaudeCliStatus,
    pub(crate) mode: ProviderModalMode,
    /// When set, browse focus jumps to this provider id (provider-scoped 401).
    pub focus_provider_id: Option<String>,
    /// A key submitted by the UI but not yet picked up by the integration.
    /// It is cleared on close and never rendered or logged.
    submitted_secret: Option<(ProviderKind, String)>,
    /// Shell-authoritative list generation (never from raw config.toml).
    pub list_generation: u64,
    /// Banner / status from management mutations (secret-free).
    pub management_message: Option<String>,
    pub management_error: Option<String>,
    /// Typed add draft (Issue 13) — not encoded into banner text.
    pub pending_add: Option<PendingProviderAdd>,
}

/// Secret-free add draft carried until the dispatch effect runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingProviderAdd {
    pub id: String,
    pub kind: String,
    pub base_url: String,
    pub display_name: Option<String>,
}

// Deliberately omit the line editor and submitted secret. App state is often
// included in diagnostics; deriving Debug here would expose a key while it is
// being typed or between Enter and the provider effect consuming it.
impl std::fmt::Debug for ProviderModalState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderModalState")
            .field("selected", &self.selected)
            .field(
                "rows",
                &self
                    .rows
                    .iter()
                    .map(ProviderKind::id_str)
                    .collect::<Vec<_>>(),
            )
            .field("statuses", &self.statuses)
            .field("claude_cli_status", &self.claude_cli_status)
            .field(
                "mode",
                &match &self.mode {
                    ProviderModalMode::Browse => "browse",
                    ProviderModalMode::ChoosingXai { .. } => "choosing_xai_auth",
                    ProviderModalMode::ChoosingOpenAi { .. } => "choosing_openai_auth",
                    ProviderModalMode::EditingKey { .. } => "editing_key_redacted",
                    ProviderModalMode::Editor(_) => "editor",
                    ProviderModalMode::Adding { .. } => "adding",
                },
            )
            .field("focus_provider_id", &self.focus_provider_id)
            .field("has_submitted_secret", &self.submitted_secret.is_some())
            .field("list_generation", &self.list_generation)
            .field("management_message", &self.management_message)
            .field("management_error", &self.management_error)
            .finish_non_exhaustive()
    }
}

impl Default for ProviderModalState {
    fn default() -> Self {
        Self::new()
    }
}

fn initial_claude_cli_status() -> ClaudeCliStatus {
    use xai_grok_shell::agent::external_runtime::gates;

    if !gates::claude_cli_feature_compiled() {
        ClaudeCliStatus::FeatureNotCompiled
    } else if !gates::claude_cli_runtime_opt_in() {
        ClaudeCliStatus::OptInMissing
    } else {
        ClaudeCliStatus::Checking
    }
}

impl ProviderModalState {
    pub fn new() -> Self {
        let rows: Vec<ProviderKind> = ProviderKind::BUILTINS.to_vec();
        let n = rows.len();
        Self {
            window: ModalWindowState::new(),
            selected: 0,
            rows,
            statuses: vec![ProviderStatus::Missing; n],
            claude_cli_status: initial_claude_cli_status(),
            mode: ProviderModalMode::Browse,
            focus_provider_id: None,
            submitted_secret: None,
            list_generation: 0,
            management_message: None,
            management_error: None,
            pending_add: None,
        }
    }

    /// Apply a shell-authored list snapshot (never raw config.toml).
    pub fn apply_list_snapshot(
        &mut self,
        snapshot: &xai_grok_shell::provider_registry::management::dto::ProviderListSnapshot,
    ) {
        self.list_generation = snapshot.generation.get();
        let configured: Vec<String> = snapshot
            .rows
            .iter()
            .filter(|r| r.is_configured)
            .map(|r| r.id.clone())
            .collect();
        self.set_configured_providers(configured);
        // Overlay shell status labels when present.
        for row in &snapshot.rows {
            let kind = if row.is_built_in {
                match row.id.as_str() {
                    "xai" => ProviderKind::Xai,
                    "openai" => ProviderKind::OpenAi,
                    "openrouter" => ProviderKind::OpenRouter,
                    "anthropic" => ProviderKind::Anthropic,
                    _ => ProviderKind::Configured(row.id.clone()),
                }
            } else {
                ProviderKind::Configured(row.id.clone())
            };
            let status = if row.credentials.has_application_key || row.credentials.has_oauth {
                ProviderStatus::Connected {
                    detail: row.status_detail.clone(),
                }
            } else if row.status_label.to_ascii_lowercase().contains("error") {
                ProviderStatus::Error(row.status_label.clone())
            } else {
                ProviderStatus::Missing
            };
            self.set_status(&kind, status);
        }
        if let Some(w) = snapshot.warnings.first() {
            self.management_message = Some(w.clone());
        }
    }

    /// Open typed editor with shell detail DTO.
    pub fn open_editor(
        &mut self,
        detail: xai_grok_shell::provider_registry::management::dto::ProviderDetailDto,
    ) {
        self.mode = ProviderModalMode::Editor(Box::new(
            crate::views::provider_editor::ProviderEditorState::new(detail),
        ));
    }

    pub fn editor_mut(
        &mut self,
    ) -> Option<&mut crate::views::provider_editor::ProviderEditorState> {
        match &mut self.mode {
            ProviderModalMode::Editor(state) => Some(state),
            _ => None,
        }
    }

    /// Replace the configured tail of the row list (built-ins stay first).
    pub fn set_configured_providers(&mut self, configured: Vec<String>) {
        let mut rows = ProviderKind::BUILTINS.to_vec();
        for id in configured {
            if !id.is_empty() {
                rows.push(ProviderKind::Configured(id));
            }
        }
        let old_selected_id = self.selected_provider().id_str().to_owned();
        self.rows = rows;
        self.statuses
            .resize(self.rows.len(), ProviderStatus::Missing);
        if let Some(idx) = self.rows.iter().position(|r| r.id_str() == old_selected_id) {
            self.selected = idx;
        } else {
            self.selected = 0;
        }
        if let Some(focus) = self.focus_provider_id.clone() {
            self.focus_provider(&focus);
        }
    }

    /// Focus a provider by id (used for provider-scoped 401 recovery).
    pub fn focus_provider(&mut self, id: &str) {
        self.focus_provider_id = Some(id.to_owned());
        if let Some(idx) = self.rows.iter().position(|r| r.id_str() == id) {
            self.selected = idx;
        }
    }

    pub fn selected_provider(&self) -> ProviderKind {
        self.rows
            .get(self.selected.min(self.rows.len().saturating_sub(1)))
            .cloned()
            .unwrap_or(ProviderKind::Xai)
    }

    pub fn status(&self, provider: &ProviderKind) -> &ProviderStatus {
        static MISSING: ProviderStatus = ProviderStatus::Missing;
        self.provider_index(provider)
            .and_then(|i| self.statuses.get(i))
            .unwrap_or(&MISSING)
    }

    /// Called by the provider backend after a status probe or connection task.
    pub fn set_status(&mut self, provider: &ProviderKind, status: ProviderStatus) {
        if let Some(idx) = self.provider_index(provider) {
            if idx >= self.statuses.len() {
                self.statuses.resize(idx + 1, ProviderStatus::Missing);
            }
            self.statuses[idx] = status;
        }
    }

    pub fn set_claude_cli_status(&mut self, status: ClaudeCliStatus) {
        self.claude_cli_status = status;
    }

    /// Returns the newly submitted key exactly once.
    ///
    /// Backend integration must move it into the platform secret store and
    /// should immediately overwrite/drop the returned `String` after use.
    pub fn take_submitted_secret(&mut self, provider: &ProviderKind) -> Option<String> {
        let matches_provider = self
            .submitted_secret
            .as_ref()
            .is_some_and(|(pending, _)| pending == provider);
        matches_provider
            .then(|| self.submitted_secret.take().map(|(_, key)| key))
            .flatten()
    }

    pub fn clear_sensitive_input(&mut self) {
        self.submitted_secret = None;
        if let ProviderModalMode::Editor(ed) = &mut self.mode {
            let _ = ed.take_app_secret();
            let _ = ed.take_admin_secret();
        }
        if !matches!(&self.mode, ProviderModalMode::Browse) {
            self.mode = ProviderModalMode::Browse;
        }
    }

    fn provider_index(&self, provider: &ProviderKind) -> Option<usize> {
        self.rows.iter().position(|r| r == provider)
    }

    fn row_count(&self) -> usize {
        self.rows.len().max(1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderModalOutcome {
    Close,
    Changed,
    Unchanged,
    Command(ProviderCommand),
}

fn handle_xai_choice(
    state: &mut ProviderModalState,
    key: &KeyEvent,
) -> Option<ProviderModalOutcome> {
    let ProviderModalMode::ChoosingXai { action, selected } = &mut state.mode else {
        return None;
    };
    match key.code {
        KeyCode::Esc => {
            state.mode = ProviderModalMode::Browse;
            Some(ProviderModalOutcome::Changed)
        }
        KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => {
            *selected = selected.saturating_sub(1);
            Some(ProviderModalOutcome::Changed)
        }
        KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => {
            *selected = (*selected + 1).min(1);
            Some(ProviderModalOutcome::Changed)
        }
        KeyCode::Enter => {
            let action = *action;
            let selected = *selected;
            if selected == 1 && action == XaiChoiceAction::Connect {
                state.mode = ProviderModalMode::EditingKey {
                    provider: ProviderKind::Xai,
                    editor: LineEditor::default(),
                };
                return Some(ProviderModalOutcome::Changed);
            }
            state.mode = ProviderModalMode::Browse;
            Some(ProviderModalOutcome::Command(match (action, selected) {
                (XaiChoiceAction::Connect, 0) => ProviderCommand::LoginXai,
                (XaiChoiceAction::Disconnect, 0) => ProviderCommand::LogoutXai,
                (XaiChoiceAction::Disconnect, 1) => ProviderCommand::Disconnect(ProviderKind::Xai),
                _ => unreachable!("xAI chooser has exactly two options"),
            }))
        }
        _ => Some(ProviderModalOutcome::Unchanged),
    }
}

fn handle_openai_choice(
    state: &mut ProviderModalState,
    key: &KeyEvent,
) -> Option<ProviderModalOutcome> {
    let ProviderModalMode::ChoosingOpenAi { selected } = &mut state.mode else {
        return None;
    };
    match key.code {
        KeyCode::Esc => {
            state.mode = ProviderModalMode::Browse;
            Some(ProviderModalOutcome::Changed)
        }
        KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => {
            *selected = selected.saturating_sub(1);
            Some(ProviderModalOutcome::Changed)
        }
        KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => {
            *selected = (*selected + 1).min(1);
            Some(ProviderModalOutcome::Changed)
        }
        KeyCode::Enter => {
            let selected = *selected;
            if selected == 1 {
                // API key
                state.mode = ProviderModalMode::EditingKey {
                    provider: ProviderKind::OpenAi,
                    editor: LineEditor::default(),
                };
                return Some(ProviderModalOutcome::Changed);
            }
            // ChatGPT OAuth (browser / device)
            state.mode = ProviderModalMode::Browse;
            state.set_status(&ProviderKind::OpenAi, ProviderStatus::Connecting);
            Some(ProviderModalOutcome::Command(ProviderCommand::LoginCodex))
        }
        _ => Some(ProviderModalOutcome::Unchanged),
    }
}

pub fn handle_key(state: &mut ProviderModalState, key: &KeyEvent) -> ProviderModalOutcome {
    if let ProviderModalMode::Editor(_) = &state.mode {
        return handle_editor_mode(state, key);
    }
    if let ProviderModalMode::Adding { .. } = &state.mode {
        return handle_adding_mode(state, key);
    }
    if let Some(outcome) = handle_xai_choice(state, key) {
        return outcome;
    }
    if let Some(outcome) = handle_openai_choice(state, key) {
        return outcome;
    }
    if matches!(&state.mode, ProviderModalMode::EditingKey { .. }) {
        match key.code {
            KeyCode::Esc => {
                state.clear_sensitive_input();
                return ProviderModalOutcome::Changed;
            }
            KeyCode::Enter => {
                let (provider, secret) = {
                    let ProviderModalMode::EditingKey { provider, editor } = &mut state.mode else {
                        unreachable!("editing mode was checked above");
                    };
                    if editor.text().trim().is_empty() {
                        return ProviderModalOutcome::Unchanged;
                    }
                    let provider = provider.clone();
                    let secret = editor.text().to_owned();
                    editor.reset();
                    (provider, secret)
                };
                state.submitted_secret = Some((provider.clone(), secret));
                let command = match state.status(&provider) {
                    ProviderStatus::Connected { .. } => {
                        ProviderCommand::ReplaceKey(provider.clone())
                    }
                    _ => ProviderCommand::Connect(provider.clone()),
                };
                state.set_status(&provider, ProviderStatus::Connecting);
                state.mode = ProviderModalMode::Browse;
                return ProviderModalOutcome::Command(command);
            }
            _ => {
                let ProviderModalMode::EditingKey { editor, .. } = &mut state.mode else {
                    unreachable!("editing mode was checked above");
                };
                return line_edit_outcome(editor.handle_key(key));
            }
        }
    }

    match key.code {
        KeyCode::Esc => ProviderModalOutcome::Close,
        KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => {
            state.selected = state.selected.saturating_sub(1);
            ProviderModalOutcome::Changed
        }
        KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => {
            state.selected = (state.selected + 1).min(state.row_count() - 1);
            ProviderModalOutcome::Changed
        }
        KeyCode::Enter | KeyCode::Char('c') if key.modifiers.is_empty() => start_connect(state),
        KeyCode::Char('t') if key.modifiers.is_empty() => {
            let provider = state.selected_provider();
            state.set_status(&provider, ProviderStatus::Connecting);
            ProviderModalOutcome::Command(ProviderCommand::Test(provider))
        }
        KeyCode::Char('d') if key.modifiers.is_empty() => {
            let provider = state.selected_provider();
            if provider == ProviderKind::Xai {
                state.mode = ProviderModalMode::ChoosingXai {
                    action: XaiChoiceAction::Disconnect,
                    selected: 0,
                };
                return ProviderModalOutcome::Changed;
            }
            state.set_status(&provider, ProviderStatus::Missing);
            // OpenAI currently disconnects both methods together; connecting
            // or replacing either method preserves the other.
            ProviderModalOutcome::Command(if provider == ProviderKind::OpenAi {
                ProviderCommand::LogoutCodex
            } else {
                ProviderCommand::Disconnect(provider)
            })
        }
        KeyCode::Char('r') if key.modifiers.is_empty() => {
            ProviderModalOutcome::Command(ProviderCommand::RefreshStatus(state.selected_provider()))
        }
        KeyCode::Char('a') if key.modifiers.is_empty() => {
            state.mode = ProviderModalMode::Adding {
                step: AddStep::Id,
                id_editor: LineEditor::default(),
                url_editor: LineEditor::default(),
                kind_index: 0,
            };
            ProviderModalOutcome::Changed
        }
        KeyCode::Char('e') if key.modifiers.is_empty() => {
            let id = state.selected_provider().id_str().to_owned();
            ProviderModalOutcome::Command(ProviderCommand::OpenEditor { provider_id: id })
        }
        KeyCode::Char('y') if key.modifiers.is_empty() => {
            // Enable selected configured provider.
            let id = state.selected_provider().id_str().to_owned();
            ProviderModalOutcome::Command(ProviderCommand::Enable { provider_id: id })
        }
        KeyCode::Char('n') if key.modifiers.is_empty() => {
            let id = state.selected_provider().id_str().to_owned();
            ProviderModalOutcome::Command(ProviderCommand::Disable { provider_id: id })
        }
        KeyCode::Char('l') if key.modifiers.is_empty() => {
            ProviderModalOutcome::Command(ProviderCommand::LoadListSnapshot)
        }
        // Browse clone: open editor clone flow with a suggested id suffix.
        KeyCode::Char('o') if key.modifiers.is_empty() => {
            let id = state.selected_provider().id_str().to_owned();
            ProviderModalOutcome::Command(ProviderCommand::OpenEditor { provider_id: id })
        }
        _ => ProviderModalOutcome::Unchanged,
    }
}

fn handle_editor_mode(state: &mut ProviderModalState, key: &KeyEvent) -> ProviderModalOutcome {
    use crate::views::provider_editor::EditorOutcome;
    let ProviderModalMode::Editor(editor) = &mut state.mode else {
        return ProviderModalOutcome::Unchanged;
    };
    match crate::views::provider_editor::handle_key(editor, key) {
        EditorOutcome::Back => {
            state.mode = ProviderModalMode::Browse;
            ProviderModalOutcome::Command(ProviderCommand::LoadListSnapshot)
        }
        EditorOutcome::Changed => ProviderModalOutcome::Changed,
        EditorOutcome::Unchanged => ProviderModalOutcome::Unchanged,
        EditorOutcome::Command(cmd) => ProviderModalOutcome::Command(ProviderCommand::Editor(cmd)),
    }
}

fn handle_adding_mode(state: &mut ProviderModalState, key: &KeyEvent) -> ProviderModalOutcome {
    let ProviderModalMode::Adding {
        step,
        id_editor,
        url_editor,
        kind_index,
    } = &mut state.mode
    else {
        return ProviderModalOutcome::Unchanged;
    };
    match key.code {
        KeyCode::Esc => {
            state.mode = ProviderModalMode::Browse;
            ProviderModalOutcome::Changed
        }
        KeyCode::Up | KeyCode::Char('k') if *step == AddStep::Kind && key.modifiers.is_empty() => {
            *kind_index = kind_index.saturating_sub(1);
            ProviderModalOutcome::Changed
        }
        KeyCode::Down | KeyCode::Char('j')
            if *step == AddStep::Kind && key.modifiers.is_empty() =>
        {
            *kind_index = (*kind_index + 1).min(ADD_KINDS.len() - 1);
            ProviderModalOutcome::Changed
        }
        KeyCode::Enter => match *step {
            AddStep::Id => {
                if id_editor.text().trim().is_empty() {
                    return ProviderModalOutcome::Unchanged;
                }
                *step = AddStep::BaseUrl;
                ProviderModalOutcome::Changed
            }
            AddStep::BaseUrl => {
                if url_editor.text().trim().is_empty() {
                    return ProviderModalOutcome::Unchanged;
                }
                *step = AddStep::Kind;
                ProviderModalOutcome::Changed
            }
            AddStep::Kind => {
                *step = AddStep::Confirm;
                ProviderModalOutcome::Changed
            }
            AddStep::Confirm => {
                let id = id_editor.text().trim().to_owned();
                let base_url = url_editor.text().trim().to_owned();
                let kind = ADD_KINDS[*kind_index].to_owned();
                state.pending_add = Some(PendingProviderAdd {
                    id,
                    kind,
                    base_url,
                    display_name: None,
                });
                state.mode = ProviderModalMode::Browse;
                ProviderModalOutcome::Command(ProviderCommand::AddConfigured)
            }
        },
        _ => {
            let editor = match *step {
                AddStep::Id => id_editor,
                AddStep::BaseUrl => url_editor,
                AddStep::Kind | AddStep::Confirm => return ProviderModalOutcome::Unchanged,
            };
            match editor.handle_key(key) {
                LineEditOutcome::TextChanged
                | LineEditOutcome::CursorChanged
                | LineEditOutcome::HandledNoChange => ProviderModalOutcome::Changed,
                LineEditOutcome::Unhandled => ProviderModalOutcome::Unchanged,
            }
        }
    }
}

fn start_connect(state: &mut ProviderModalState) -> ProviderModalOutcome {
    let provider = state.selected_provider();
    if provider == ProviderKind::Xai {
        state.mode = ProviderModalMode::ChoosingXai {
            action: XaiChoiceAction::Connect,
            selected: 0,
        };
        ProviderModalOutcome::Changed
    } else if provider == ProviderKind::OpenAi {
        // ChatGPT OAuth and an API key may coexist; the selected route chooses one.
        state.mode = ProviderModalMode::ChoosingOpenAi { selected: 0 };
        ProviderModalOutcome::Changed
    } else if provider.needs_api_key() {
        state.mode = ProviderModalMode::EditingKey {
            provider,
            editor: LineEditor::default(),
        };
        ProviderModalOutcome::Changed
    } else if matches!(state.status(&provider), ProviderStatus::Connected { .. }) {
        ProviderModalOutcome::Command(ProviderCommand::RefreshStatus(provider))
    } else {
        state.set_status(&provider, ProviderStatus::Connecting);
        ProviderModalOutcome::Command(ProviderCommand::LoginCodex)
    }
}

fn line_edit_outcome(outcome: LineEditOutcome) -> ProviderModalOutcome {
    match outcome {
        LineEditOutcome::TextChanged
        | LineEditOutcome::CursorChanged
        | LineEditOutcome::HandledNoChange => ProviderModalOutcome::Changed,
        LineEditOutcome::Unhandled => ProviderModalOutcome::Unchanged,
    }
}

pub fn handle_paste(state: &mut ProviderModalState, text: &str) -> ProviderModalOutcome {
    if let ProviderModalMode::Editor(editor) = &mut state.mode {
        return match crate::views::provider_editor::handle_paste(editor, text) {
            crate::views::provider_editor::EditorOutcome::Changed => ProviderModalOutcome::Changed,
            _ => ProviderModalOutcome::Unchanged,
        };
    }
    if let ProviderModalMode::Adding {
        step,
        id_editor,
        url_editor,
        ..
    } = &mut state.mode
    {
        let editor = match *step {
            AddStep::Id => id_editor,
            AddStep::BaseUrl => url_editor,
            AddStep::Kind | AddStep::Confirm => return ProviderModalOutcome::Unchanged,
        };
        return line_edit_outcome(editor.insert_paste_with_byte_limit(text, 16_384));
    }
    let ProviderModalMode::EditingKey { editor, .. } = &mut state.mode else {
        return ProviderModalOutcome::Unchanged;
    };
    line_edit_outcome(editor.insert_paste_with_byte_limit(text, 16_384))
}

pub fn render_modal(buf: &mut Buffer, area: Rect, state: &mut ProviderModalState, compact: bool) {
    let theme = Theme::current();
    let footer = [
        Shortcut {
            label: "↵ connect",
            clickable: false,
            id: 0,
        },
        Shortcut {
            label: "c replace key",
            clickable: false,
            id: 0,
        },
        Shortcut {
            label: "t test",
            clickable: false,
            id: 0,
        },
        Shortcut {
            label: "d disconnect",
            clickable: false,
            id: 0,
        },
        Shortcut {
            label: "r status",
            clickable: false,
            id: 0,
        },
        Shortcut {
            label: "e edit",
            clickable: false,
            id: 0,
        },
        Shortcut {
            label: "a add",
            clickable: false,
            id: 0,
        },
        Shortcut {
            label: "y/n enable",
            clickable: false,
            id: 0,
        },
        Shortcut {
            label: "Esc close",
            clickable: false,
            id: 0,
        },
    ];
    let config = ModalWindowConfig {
        title: "Providers",
        tabs: None,
        shortcuts: &footer,
        sizing: ModalSizing {
            width_pct: if compact { 0.96 } else { 0.72 },
            max_width: 94,
            min_width: 58,
            v_margin: 5,
            h_pad: 2,
            v_pad: 1,
            footer_lines: 2,
        },
        fold_info: None,
    };
    let Some(content) =
        modal_window::render_modal_window(buf, area, &mut state.window, &config, &theme)
    else {
        return;
    };
    let mut y = content.content.y;
    if let ProviderModalMode::Editor(editor) = &state.mode {
        crate::views::provider_editor::render_editor(buf, content.content, editor, &mut y);
        return;
    }
    if let ProviderModalMode::Adding {
        step,
        id_editor,
        url_editor,
        kind_index,
    } = &state.mode
    {
        put_line(
            buf,
            content.content,
            &mut y,
            "Add OpenAI / OpenRouter / custom provider instance",
            Style::default()
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD),
        );
        put_line(
            buf,
            content.content,
            &mut y,
            &format!("Step: {:?} · generation {}", step, state.list_generation),
            Style::default().fg(theme.gray),
        );
        put_line(
            buf,
            content.content,
            &mut y,
            &format!(
                "{} Id: {}",
                if *step == AddStep::Id { "›" } else { " " },
                id_editor.text()
            ),
            Style::default().fg(theme.gray),
        );
        put_line(
            buf,
            content.content,
            &mut y,
            &format!(
                "{} Base URL: {}",
                if *step == AddStep::BaseUrl {
                    "›"
                } else {
                    " "
                },
                url_editor.text()
            ),
            Style::default().fg(theme.gray),
        );
        for (i, kind) in ADD_KINDS.iter().enumerate() {
            let mark = if *step == AddStep::Kind && *kind_index == i {
                "›"
            } else {
                " "
            };
            put_line(
                buf,
                content.content,
                &mut y,
                &format!("{mark} kind: {kind}"),
                Style::default().fg(if *kind_index == i {
                    theme.accent_user
                } else {
                    theme.gray
                }),
            );
        }
        if *step == AddStep::Confirm {
            put_line(
                buf,
                content.content,
                &mut y,
                "Enter confirms add · Esc cancels",
                Style::default().fg(theme.accent_user),
            );
        }
        return;
    }
    put_line(
        buf,
        content.content,
        &mut y,
        &format!(
            "Configure model providers (gen {}). Keys are masked and never written to config.toml.",
            state.list_generation
        ),
        Style::default().fg(theme.gray),
    );
    if let Some(msg) = &state.management_message {
        put_line(
            buf,
            content.content,
            &mut y,
            msg,
            Style::default().fg(theme.accent_success),
        );
    }
    if let Some(err) = &state.management_error {
        put_line(
            buf,
            content.content,
            &mut y,
            &format!("Error: {err}"),
            Style::default().fg(theme.accent_error),
        );
    }
    for (idx, provider) in state.rows.iter().enumerate() {
        let show_claude_cli = provider == &ProviderKind::Anthropic
            && !matches!(
                &state.claude_cli_status,
                ClaudeCliStatus::FeatureNotCompiled
            );
        let row_height = if show_claude_cli { 4 } else { 2 };
        if y.saturating_add(row_height) >= content.content.y.saturating_add(content.content.height)
        {
            break;
        }
        let selected = state.selected == idx && matches!(&state.mode, ProviderModalMode::Browse);
        let bg = selected.then_some(theme.bg_highlight);
        let mut title = Style::default()
            .fg(theme.text_primary)
            .add_modifier(Modifier::BOLD);
        if let Some(bg) = bg {
            title = title.bg(bg);
        }
        let prefix = if selected { "› " } else { "  " };
        let status = state.status(provider);
        let status_color = match status {
            ProviderStatus::Connected { .. } => theme.accent_success,
            ProviderStatus::Connecting => theme.accent_running,
            ProviderStatus::Error(_) => theme.accent_error,
            ProviderStatus::Missing => theme.gray_dim,
        };
        let mut status_style = Style::default().fg(status_color);
        if let Some(bg) = bg {
            status_style = status_style.bg(bg);
        }
        let status_text = format!(" [{:>18}]", status.label());
        let x = content.content.x;
        buf.set_string(x, y, prefix, title);
        buf.set_string(x + 2, y, &provider.label(), title);
        let sx =
            (content.content.x + content.content.width).saturating_sub(status_text.len() as u16);
        buf.set_string(sx, y, &status_text, status_style);
        y = y.saturating_add(1);
        let mut detail_style = Style::default().fg(theme.gray);
        if let Some(bg) = bg {
            detail_style = detail_style.bg(bg);
        }
        let owned_detail = provider.detail();
        let detail = match status {
            ProviderStatus::Connected {
                detail: Some(detail),
            } => detail.as_str(),
            ProviderStatus::Error(error) => error.as_str(),
            _ => owned_detail.as_str(),
        };
        let detail = if provider == &ProviderKind::Anthropic {
            format!("    Messages API: {detail}")
        } else {
            format!("    {detail}")
        };
        put_line(buf, content.content, &mut y, &detail, detail_style);

        if show_claude_cli {
            let cli_status = &state.claude_cli_status;
            let cli_color = match cli_status {
                ClaudeCliStatus::Ready { .. } => theme.accent_success,
                ClaudeCliStatus::Checking => theme.accent_running,
                ClaudeCliStatus::AuthRequired { .. } => theme.warning,
                ClaudeCliStatus::AuthUnknown { .. } | ClaudeCliStatus::ProbeFailed(_) => {
                    theme.accent_error
                }
                ClaudeCliStatus::FeatureNotCompiled | ClaudeCliStatus::OptInMissing => {
                    theme.gray_dim
                }
            };
            let cli_detail = match cli_status {
                ClaudeCliStatus::Checking => {
                    "probing official `claude` binary and subscription".to_owned()
                }
                ClaudeCliStatus::Ready {
                    version,
                    auth_summary,
                } => format!("v{version} · {auth_summary}"),
                ClaudeCliStatus::AuthRequired { version, detail }
                | ClaudeCliStatus::AuthUnknown { version, detail } => {
                    format!("v{version} · {detail}")
                }
                ClaudeCliStatus::FeatureNotCompiled => {
                    "build without `claude-cli-runtime`; hidden from /model".to_owned()
                }
                ClaudeCliStatus::OptInMissing => {
                    "set GROK_CLAUDE_CLI_RUNTIME=1; hidden from /model".to_owned()
                }
                ClaudeCliStatus::ProbeFailed(error) => {
                    format!("{error} · hidden from /model")
                }
            };
            put_line(
                buf,
                content.content,
                &mut y,
                &format!(
                    "    Claude Agent CLI [{}]: {cli_detail}",
                    cli_status.label()
                ),
                Style::default().fg(cli_color),
            );
            put_line(
                buf,
                content.content,
                &mut y,
                "      Experimental subscription mode · separate from the API key",
                Style::default().fg(theme.gray_dim),
            );
        }
    }
    if let ProviderModalMode::EditingKey { provider, editor } = &state.mode {
        y = y.saturating_add(1);
        let masked = "•".repeat(editor.text().chars().count());
        put_line(
            buf,
            content.content,
            &mut y,
            &format!("{} API key (masked): {}", provider.label(), masked),
            Style::default().fg(theme.accent_user),
        );
        put_line(
            buf,
            content.content,
            &mut y,
            "Enter saves securely and connects · Esc cancels",
            Style::default().fg(theme.gray_dim),
        );
    } else if let ProviderModalMode::ChoosingXai { action, selected } = &state.mode {
        y = y.saturating_add(1);
        let verb = if *action == XaiChoiceAction::Connect {
            "Connect"
        } else {
            "Disconnect"
        };
        put_line(
            buf,
            content.content,
            &mut y,
            &format!("{verb} xAI credential:"),
            Style::default().fg(theme.text_primary),
        );
        for (idx, label) in ["Grok/xAI account (OAuth)", "xAI API key"]
            .iter()
            .enumerate()
        {
            let prefix = if *selected == idx { "› " } else { "  " };
            put_line(
                buf,
                content.content,
                &mut y,
                &format!("{prefix}{label}"),
                Style::default().fg(if *selected == idx {
                    theme.accent_user
                } else {
                    theme.gray
                }),
            );
        }
    } else if let ProviderModalMode::ChoosingOpenAi { selected } = &state.mode {
        y = y.saturating_add(1);
        put_line(
            buf,
            content.content,
            &mut y,
            "Add an OpenAI credential (both may be stored):",
            Style::default().fg(theme.text_primary),
        );
        for (idx, label) in ["ChatGPT Pro/Plus (browser OAuth)", "OpenAI API key"]
            .iter()
            .enumerate()
        {
            let prefix = if *selected == idx { "› " } else { "  " };
            put_line(
                buf,
                content.content,
                &mut y,
                &format!("{prefix}{label}"),
                Style::default().fg(if *selected == idx {
                    theme.accent_user
                } else {
                    theme.gray
                }),
            );
        }
        put_line(
            buf,
            content.content,
            &mut y,
            "The selected model route chooses OAuth or the API key.",
            Style::default().fg(theme.gray_dim),
        );
    } else {
        put_line(
            buf,
            content.content,
            &mut y,
            "xAI/OpenAI: routes select OAuth or API key. Anthropic: Messages API key or separate Claude CLI subscription.",
            Style::default().fg(theme.gray_dim),
        );
    }
}

fn put_line(buf: &mut Buffer, area: Rect, y: &mut u16, text: &str, style: Style) {
    if *y < area.y.saturating_add(area.height) {
        let max = area.width as usize;
        let clipped: String = text.chars().take(max).collect();
        buf.set_string(area.x, *y, clipped, style);
        *y = y.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn api_key_is_masked_and_submit_intent_has_no_secret() {
        let mut state = ProviderModalState::new();
        state.selected = 1; // OpenAI
        // OpenAI: choose API key path from dual-auth chooser
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Enter)),
            ProviderModalOutcome::Changed
        );
        assert!(matches!(
            &state.mode,
            ProviderModalMode::ChoosingOpenAi { .. }
        ));
        handle_key(&mut state, &key(KeyCode::Down));
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Enter)),
            ProviderModalOutcome::Changed
        );
        assert!(matches!(&state.mode, ProviderModalMode::EditingKey { .. }));
        assert_eq!(
            handle_paste(&mut state, "sk-secret"),
            ProviderModalOutcome::Changed
        );
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Enter)),
            ProviderModalOutcome::Command(ProviderCommand::Connect(ProviderKind::OpenAi))
        );
        assert_eq!(
            state.status(&ProviderKind::OpenAi),
            &ProviderStatus::Connecting
        );
        assert_eq!(
            state
                .take_submitted_secret(&ProviderKind::OpenAi)
                .as_deref(),
            Some("sk-secret")
        );
        assert!(state.take_submitted_secret(&ProviderKind::OpenAi).is_none());
    }

    #[test]
    fn diagnostics_never_render_typed_or_submitted_api_key() {
        let mut state = ProviderModalState::new();
        state.selected = 1; // OpenAI
        handle_key(&mut state, &key(KeyCode::Enter)); // chooser
        handle_key(&mut state, &key(KeyCode::Down)); // API key
        handle_key(&mut state, &key(KeyCode::Enter)); // editing
        handle_paste(&mut state, "sk-super-secret");
        let editing_debug = format!("{state:?}");
        assert!(!editing_debug.contains("sk-super-secret"));
        assert!(editing_debug.contains("editing_key_redacted"));
        assert!(!format!("{:?}", state.mode).contains("sk-super-secret"));

        handle_key(&mut state, &key(KeyCode::Enter));
        let submitted_debug = format!("{state:?}");
        assert!(!submitted_debug.contains("sk-super-secret"));
        assert!(submitted_debug.contains("has_submitted_secret: true"));
    }

    #[test]
    fn openai_chatgpt_oauth_is_default_connect_choice() {
        let mut state = ProviderModalState::new();
        state.selected = 1; // OpenAI
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Enter)),
            ProviderModalOutcome::Changed
        );
        assert!(matches!(
            state.mode,
            ProviderModalMode::ChoosingOpenAi { selected: 0 }
        ));
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Enter)),
            ProviderModalOutcome::Command(ProviderCommand::LoginCodex)
        );
        assert!(matches!(&state.mode, ProviderModalMode::Browse));
    }

    #[test]
    fn disconnect_and_status_transitions_are_explicit() {
        let mut state = ProviderModalState::new();
        state.set_status(
            &ProviderKind::OpenRouter,
            ProviderStatus::Connected {
                detail: Some("ok".into()),
            },
        );
        state.selected = 2;
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Char('d'))),
            ProviderModalOutcome::Command(ProviderCommand::Disconnect(ProviderKind::OpenRouter))
        );
        assert_eq!(
            state.status(&ProviderKind::OpenRouter),
            &ProviderStatus::Missing
        );
        state.set_status(
            &ProviderKind::OpenRouter,
            ProviderStatus::Error("invalid key".into()),
        );
        assert_eq!(
            state.status(&ProviderKind::OpenRouter).label(),
            "Connection error"
        );
    }

    #[test]
    fn anthropic_is_a_builtin_card_after_openrouter() {
        let state = ProviderModalState::new();
        assert_eq!(ProviderKind::BUILTINS.len(), 4);
        assert_eq!(
            ProviderKind::BUILTINS[3],
            ProviderKind::Anthropic,
            "peer order: Anthropic after OpenRouter"
        );
        assert_eq!(state.rows[3], ProviderKind::Anthropic);
        assert!(ProviderKind::Anthropic.needs_api_key());
        assert_eq!(ProviderKind::Anthropic.id_str(), "anthropic");
        assert!(
            ProviderKind::Anthropic
                .detail()
                .contains("Native Grok agent loop")
        );
    }

    #[test]
    fn anthropic_card_renders_api_and_ready_cli_modes_separately() {
        let mut state = ProviderModalState::new();
        state.set_status(
            &ProviderKind::Anthropic,
            ProviderStatus::Connected {
                detail: Some("Configured by environment variable".into()),
            },
        );
        state.set_claude_cli_status(ClaudeCliStatus::Ready {
            version: "2.1.219".into(),
            auth_summary: "Claude subscription logged in".into(),
        });

        let area = Rect::new(0, 0, 110, 32);
        let mut buf = Buffer::empty(area);
        render_modal(&mut buf, area, &mut state, false);
        let mut rendered = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                rendered.push_str(buf[(x, y)].symbol());
            }
            rendered.push('\n');
        }

        assert!(rendered.contains("Messages API: Configured by environment variable"));
        assert!(rendered.contains("Claude Agent CLI [Ready]: v2.1.219"));
        assert!(rendered.contains("subscription logged in"));
        assert!(rendered.contains("separate from the API key"));
    }

    #[test]
    fn release_build_hides_cli_mode_when_feature_is_not_compiled() {
        let mut state = ProviderModalState::new();
        state.set_claude_cli_status(ClaudeCliStatus::FeatureNotCompiled);
        let area = Rect::new(0, 0, 110, 32);
        let mut buf = Buffer::empty(area);
        render_modal(&mut buf, area, &mut state, false);
        let mut rendered = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                rendered.push_str(buf[(x, y)].symbol());
            }
            rendered.push('\n');
        }

        assert!(!rendered.contains("Claude Agent CLI"));
    }

    #[test]
    fn anthropic_card_renders_actionable_cli_gate_status() {
        let mut state = ProviderModalState::new();
        state.set_claude_cli_status(ClaudeCliStatus::OptInMissing);
        let area = Rect::new(0, 0, 110, 32);
        let mut buf = Buffer::empty(area);
        render_modal(&mut buf, area, &mut state, false);
        let mut rendered = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                rendered.push_str(buf[(x, y)].symbol());
            }
            rendered.push('\n');
        }

        assert!(rendered.contains("Claude Agent CLI [Opt-in required]"));
        assert!(rendered.contains("GROK_CLAUDE_CLI_RUNTIME=1"));
    }

    #[test]
    fn anthropic_connect_replace_test_disconnect_never_put_secret_in_command() {
        let mut state = ProviderModalState::new();
        state.selected = 3; // Anthropic
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Enter)),
            ProviderModalOutcome::Changed
        );
        assert!(matches!(
            &state.mode,
            ProviderModalMode::EditingKey {
                provider: ProviderKind::Anthropic,
                ..
            }
        ));
        assert_eq!(
            handle_paste(&mut state, "sk-ant-test-secret"),
            ProviderModalOutcome::Changed
        );
        let cmd = handle_key(&mut state, &key(KeyCode::Enter));
        assert_eq!(
            cmd,
            ProviderModalOutcome::Command(ProviderCommand::Connect(ProviderKind::Anthropic))
        );
        assert!(!format!("{cmd:?}").contains("sk-ant-test-secret"));
        assert!(!format!("{state:?}").contains("sk-ant-test-secret"));
        assert_eq!(
            state
                .take_submitted_secret(&ProviderKind::Anthropic)
                .as_deref(),
            Some("sk-ant-test-secret")
        );

        state.set_status(
            &ProviderKind::Anthropic,
            ProviderStatus::Connected {
                detail: Some("ok".into()),
            },
        );
        state.selected = 3;
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Char('t'))),
            ProviderModalOutcome::Command(ProviderCommand::Test(ProviderKind::Anthropic))
        );
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Char('d'))),
            ProviderModalOutcome::Command(ProviderCommand::Disconnect(ProviderKind::Anthropic))
        );
    }

    #[test]
    fn configured_providers_appear_in_dynamic_rows() {
        let mut state = ProviderModalState::new();
        state.set_configured_providers(vec!["local_vllm".into(), "zai-model-api".into()]);
        assert!(state.rows.iter().any(|r| r.id_str() == "local_vllm"));
        assert!(state.rows.iter().any(|r| r.id_str() == "zai-model-api"));
        state.focus_provider("zai-model-api");
        assert_eq!(state.selected_provider().id_str(), "zai-model-api");
    }

    #[test]
    fn secret_debug_never_leaks_configured_key() {
        let mut state = ProviderModalState::new();
        state.set_configured_providers(vec!["local_vllm".into()]);
        state.focus_provider("local_vllm");
        handle_key(&mut state, &key(KeyCode::Enter));
        handle_paste(&mut state, "sk-configured-secret");
        let dbg = format!("{state:?}");
        assert!(!dbg.contains("sk-configured-secret"));
    }

    #[test]
    fn xai_chooses_oauth_or_api_key_without_exposing_a_secret() {
        let mut state = ProviderModalState::new();
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Enter)),
            ProviderModalOutcome::Changed
        );
        assert!(matches!(
            state.mode,
            ProviderModalMode::ChoosingXai {
                action: XaiChoiceAction::Connect,
                selected: 0
            }
        ));
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Enter)),
            ProviderModalOutcome::Command(ProviderCommand::LoginXai)
        );

        handle_key(&mut state, &key(KeyCode::Enter));
        handle_key(&mut state, &key(KeyCode::Down));
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Enter)),
            ProviderModalOutcome::Changed
        );
        assert!(matches!(
            state.mode,
            ProviderModalMode::EditingKey {
                provider: ProviderKind::Xai,
                ..
            }
        ));
    }
}
