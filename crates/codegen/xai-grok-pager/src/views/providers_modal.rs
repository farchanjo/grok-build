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

/// A ChatGPT subscription model available for context-window configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatgptModel {
    pub id: String,
    pub label: String,
    pub context_window: Option<u64>,
    /// Catalog/live default before a user override. Restored when the override
    /// is cleared so the list does not fall back to `None`.
    pub catalog_context_window: Option<u64>,
}

impl ChatgptModel {
    pub(crate) fn from_catalog(id: String, label: String, context_window: Option<u64>) -> Self {
        Self {
            id,
            label,
            context_window,
            catalog_context_window: context_window,
        }
    }

    fn apply_override(&mut self, tokens: Option<u64>) {
        self.context_window = tokens.or(self.catalog_context_window);
    }
}

/// A display-only ChatGPT account email that is redacted from diagnostics.
#[derive(Clone, PartialEq, Eq)]
pub struct ChatgptAccountEmail(String);

impl ChatgptAccountEmail {
    /// Returns an email that is safe to render in the terminal UI.
    pub fn new(email: &str) -> Option<Self> {
        let email = email.trim();
        let mut parts = email.split('@');
        let local = parts.next()?;
        let domain = parts.next()?;
        if parts.next().is_some()
            || local.is_empty()
            || domain.is_empty()
            || email.len() > 320
            || !email.is_ascii()
            || !email.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-' | b'@')
            })
        {
            return None;
        }
        Some(Self(email.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ChatgptAccountEmail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

/// The externally observable state of a provider connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderStatus {
    Missing,
    Connecting,
    Connected {
        detail: Option<String>,
        chatgpt_account_email: Option<ChatgptAccountEmail>,
        chatgpt_models: Vec<ChatgptModel>,
    },
    Error(String),
}

impl ProviderStatus {
    pub(crate) fn apply_chatgpt_context_window(&mut self, model_id: &str, tokens: Option<u64>) {
        if let Self::Connected { chatgpt_models, .. } = self
            && let Some(model) = chatgpt_models.iter_mut().find(|model| model.id == model_id)
        {
            model.apply_override(tokens);
        }
    }

    pub(crate) fn overlay_chatgpt_windows(&mut self, lookup: impl Fn(&str) -> Option<u64>) {
        if let Self::Connected { chatgpt_models, .. } = self {
            for model in chatgpt_models {
                if let Some(tokens) = lookup(&model.id) {
                    model.context_window = Some(tokens);
                }
            }
        }
    }

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
    /// Persist or clear a ChatGPT subscription model context-window override.
    SetChatgptContextWindow {
        model_id: String,
        tokens: Option<u64>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum XaiChoiceAction {
    Connect,
    Disconnect,
}

#[derive(Clone, PartialEq, Eq)]
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
    ChatgptSubscription {
        selected: usize,
        editor: Option<LineEditor>,
    },
}

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
            Self::ChatgptSubscription { selected, editor } => f
                .debug_struct("ChatgptSubscription")
                .field("selected", selected)
                .field("editing_context_window", &editor.is_some())
                .finish(),
        }
    }
}

impl ProviderModalMode {
    /// Sub-modes that own Esc instead of closing `/providers`.
    pub(crate) fn owns_escape(&self) -> bool {
        matches!(
            self,
            Self::EditingKey { .. } | Self::ChatgptSubscription { .. }
        )
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
                    ProviderModalMode::ChatgptSubscription { editor, .. } => {
                        if editor.is_some() {
                            "editing_chatgpt_context_window"
                        } else {
                            "chatgpt_subscription"
                        }
                    }
                },
            )
            .field("focus_provider_id", &self.focus_provider_id)
            .field("has_submitted_secret", &self.submitted_secret.is_some())
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
        self.clamp_chatgpt_subscription_selection();
    }

    pub(crate) fn status_mut(&mut self, provider: &ProviderKind) -> Option<&mut ProviderStatus> {
        let idx = self.provider_index(provider)?;
        self.statuses.get_mut(idx)
    }

    fn clamp_chatgpt_subscription_selection(&mut self) {
        if !matches!(self.mode, ProviderModalMode::ChatgptSubscription { .. }) {
            return;
        }
        let len = chatgpt_models(self).len();
        if let ProviderModalMode::ChatgptSubscription { selected, .. } = &mut self.mode {
            *selected = if len == 0 {
                0
            } else {
                (*selected).min(len - 1)
            };
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

fn chatgpt_models(state: &ProviderModalState) -> &[ChatgptModel] {
    match state.status(&ProviderKind::OpenAi) {
        ProviderStatus::Connected { chatgpt_models, .. } => chatgpt_models,
        _ => &[],
    }
}

fn chatgpt_oauth_connected(state: &ProviderModalState) -> bool {
    matches!(
        state.status(&ProviderKind::OpenAi),
        ProviderStatus::Connected {
            chatgpt_account_email,
            chatgpt_models,
            ..
        } if chatgpt_account_email.is_some() || !chatgpt_models.is_empty()
    )
}

fn chatgpt_subscription_available(state: &ProviderModalState) -> bool {
    state.selected_provider() == ProviderKind::OpenAi && chatgpt_oauth_connected(state)
}

fn handle_chatgpt_subscription(
    state: &mut ProviderModalState,
    key: &KeyEvent,
) -> Option<ProviderModalOutcome> {
    let ProviderModalMode::ChatgptSubscription { selected, editor } = state.mode.clone() else {
        return None;
    };
    let len = chatgpt_models(state).len();
    let selected = if len == 0 { 0 } else { selected.min(len - 1) };
    if let ProviderModalMode::ChatgptSubscription {
        selected: stored, ..
    } = &mut state.mode
    {
        *stored = selected;
    }
    let models = chatgpt_models(state);
    if let Some(mut editor) = editor {
        match key.code {
            KeyCode::Esc => {
                state.mode = ProviderModalMode::ChatgptSubscription {
                    selected,
                    editor: None,
                };
                Some(ProviderModalOutcome::Changed)
            }
            KeyCode::Enter => {
                let Ok(tokens) = editor.text().trim().parse::<u64>() else {
                    return Some(ProviderModalOutcome::Unchanged);
                };
                if !crate::config_toml_edit::chatgpt_context_window_in_range(tokens) {
                    return Some(ProviderModalOutcome::Unchanged);
                }
                let Some(model_id) = models
                    .get(selected)
                    .filter(|model| crate::config_toml_edit::is_chatgpt_model_id(&model.id))
                    .map(|model| model.id.clone())
                else {
                    return Some(ProviderModalOutcome::Unchanged);
                };
                state.mode = ProviderModalMode::ChatgptSubscription {
                    selected,
                    editor: None,
                };
                Some(ProviderModalOutcome::Command(
                    ProviderCommand::SetChatgptContextWindow {
                        model_id,
                        tokens: Some(tokens),
                    },
                ))
            }
            _ => {
                let outcome = line_edit_outcome(editor.handle_key(key));
                state.mode = ProviderModalMode::ChatgptSubscription {
                    selected,
                    editor: Some(editor),
                };
                Some(outcome)
            }
        }
    } else {
        match key.code {
            KeyCode::Esc => {
                state.mode = ProviderModalMode::Browse;
                Some(ProviderModalOutcome::Changed)
            }
            KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => {
                state.mode = ProviderModalMode::ChatgptSubscription {
                    selected: selected.saturating_sub(1),
                    editor: None,
                };
                Some(ProviderModalOutcome::Changed)
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => {
                state.mode = ProviderModalMode::ChatgptSubscription {
                    selected: (selected + 1).min(len.saturating_sub(1)),
                    editor: None,
                };
                Some(ProviderModalOutcome::Changed)
            }
            KeyCode::Enter if len > 0 => {
                state.mode = ProviderModalMode::ChatgptSubscription {
                    selected,
                    editor: Some(LineEditor::default()),
                };
                Some(ProviderModalOutcome::Changed)
            }
            KeyCode::Char('x') if key.modifiers.is_empty() && len > 0 => {
                let Some(model_id) = models
                    .get(selected)
                    .filter(|model| crate::config_toml_edit::is_chatgpt_model_id(&model.id))
                    .map(|model| model.id.clone())
                else {
                    return Some(ProviderModalOutcome::Unchanged);
                };
                Some(ProviderModalOutcome::Command(
                    ProviderCommand::SetChatgptContextWindow {
                        model_id,
                        tokens: None,
                    },
                ))
            }
            _ => Some(ProviderModalOutcome::Unchanged),
        }
    }
}

pub fn handle_key(state: &mut ProviderModalState, key: &KeyEvent) -> ProviderModalOutcome {
    if let Some(outcome) = handle_chatgpt_subscription(state, key) {
        return outcome;
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
        KeyCode::Char('s') if key.modifiers.is_empty() && chatgpt_subscription_available(state) => {
            state.mode = ProviderModalMode::ChatgptSubscription {
                selected: 0,
                editor: None,
            };
            ProviderModalOutcome::Changed
        }
        KeyCode::Char('a') if key.modifiers.is_empty() => {
            ProviderModalOutcome::Command(ProviderCommand::AddConfigured)
        }
        _ => ProviderModalOutcome::Unchanged,
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
    let editor = match &mut state.mode {
        ProviderModalMode::EditingKey { editor, .. } => editor,
        ProviderModalMode::ChatgptSubscription {
            editor: Some(editor),
            ..
        } => editor,
        _ => return ProviderModalOutcome::Unchanged,
    };
    line_edit_outcome(editor.insert_paste_with_byte_limit(text, 16_384))
}

pub fn render_modal(buf: &mut Buffer, area: Rect, state: &mut ProviderModalState, compact: bool) {
    let theme = Theme::current();
    let footer: Vec<Shortcut<'_>> = match &state.mode {
        ProviderModalMode::ChatgptSubscription {
            editor: Some(_), ..
        } => vec![
            Shortcut {
                label: "↵ save",
                clickable: false,
                id: 0,
            },
            Shortcut {
                label: "Esc cancel",
                clickable: false,
                id: 0,
            },
        ],
        ProviderModalMode::ChatgptSubscription { editor: None, .. } => vec![
            Shortcut {
                label: "↵ set override",
                clickable: false,
                id: 0,
            },
            Shortcut {
                label: "x clear",
                clickable: false,
                id: 0,
            },
            Shortcut {
                label: "Esc back",
                clickable: false,
                id: 0,
            },
        ],
        _ => {
            let mut items = vec![
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
            ];
            if chatgpt_subscription_available(state) {
                items.push(Shortcut {
                    label: "s subscription",
                    clickable: false,
                    id: 0,
                });
            }
            items.push(Shortcut {
                label: "Esc close",
                clickable: false,
                id: 0,
            });
            items
        }
    };
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
    put_line(
        buf,
        content.content,
        &mut y,
        "Configure model providers. Keys are masked and are never written to config.toml.",
        Style::default().fg(theme.gray),
    );
    if let ProviderModalMode::ChatgptSubscription { selected, editor } = &state.mode {
        put_line(
            buf,
            content.content,
            &mut y,
            "ChatGPT subscription context windows",
            Style::default()
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD),
        );
        let models = chatgpt_models(state);
        if models.is_empty() {
            put_line(
                buf,
                content.content,
                &mut y,
                "No ChatGPT models in the catalog.",
                Style::default().fg(theme.gray),
            );
        }
        for (idx, model) in models.iter().enumerate() {
            let prefix = if *selected == idx { "› " } else { "  " };
            let window = model
                .context_window
                .map(|tokens| format!("{tokens} tokens"))
                .unwrap_or_else(|| "unknown".to_owned());
            put_line(
                buf,
                content.content,
                &mut y,
                &format!("{prefix}{} · {window}", model.label),
                Style::default().fg(if *selected == idx {
                    theme.accent_user
                } else {
                    theme.gray
                }),
            );
        }
        if let Some(editor) = editor {
            put_line(
                buf,
                content.content,
                &mut y,
                &format!("Context window: {}", editor.text()),
                Style::default().fg(theme.accent_user),
            );
            put_line(
                buf,
                content.content,
                &mut y,
                "Enter saves · Esc cancels · 8,000–1,050,000 tokens",
                Style::default().fg(theme.gray_dim),
            );
            if editor.text().trim().parse::<u64>().is_ok_and(|tokens| {
                tokens > crate::config_toml_edit::CHATGPT_LONG_CONTEXT_THRESHOLD
            }) {
                put_line(
                    buf,
                    content.content,
                    &mut y,
                    "OpenAI may apply long-context limits or pricing to this subscription.",
                    Style::default().fg(theme.warning),
                );
            }
        } else {
            put_line(
                buf,
                content.content,
                &mut y,
                "Enter set override · x clear override · Esc back",
                Style::default().fg(theme.gray_dim),
            );
        }
        return;
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
                chatgpt_account_email,
                ..
            } if provider == &ProviderKind::OpenAi => match chatgpt_account_email {
                Some(email) => format!("{detail} — {}", email.as_str()),
                None => detail.clone(),
            },
            ProviderStatus::Connected {
                detail: Some(detail),
                ..
            } => detail.clone(),
            ProviderStatus::Error(error) => error.clone(),
            _ => owned_detail,
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

    fn sol_model() -> ChatgptModel {
        ChatgptModel::from_catalog(
            "chatgpt-gpt-5.6-sol".into(),
            "GPT-5.6 Sol".into(),
            Some(272_000),
        )
    }

    fn connected_openai(models: Vec<ChatgptModel>) -> ProviderStatus {
        ProviderStatus::Connected {
            detail: Some("Connected with ChatGPT OAuth".into()),
            chatgpt_account_email: ChatgptAccountEmail::new("user@example.com"),
            chatgpt_models: models,
        }
    }

    fn buffer_text(buf: &Buffer, area: Rect) -> String {
        let mut rendered = String::new();
        for y in 0..area.height {
            let mut line = String::new();
            for x in 0..area.width {
                line.push_str(buf[(x, y)].symbol());
            }
            rendered.push_str(line.trim_end());
            rendered.push('\n');
        }
        rendered.trim_end().to_string()
    }

    fn rendered_modal(state: &mut ProviderModalState) -> String {
        let area = Rect::new(0, 0, 110, 32);
        let mut buf = Buffer::empty(area);
        render_modal(&mut buf, area, state, false);
        buffer_text(&buf, area)
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
                chatgpt_account_email: None,
                chatgpt_models: Vec::new(),
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
                chatgpt_account_email: None,
                chatgpt_models: Vec::new(),
            },
        );
        state.set_claude_cli_status(ClaudeCliStatus::Ready {
            version: "2.1.219".into(),
            auth_summary: "Claude subscription logged in".into(),
        });

        let rendered = rendered_modal(&mut state);

        assert!(rendered.contains("Messages API: Configured by environment variable"));
        assert!(rendered.contains("Claude Agent CLI [Ready]: v2.1.219"));
        assert!(rendered.contains("subscription logged in"));
        assert!(rendered.contains("separate from the API key"));
    }

    #[test]
    fn release_build_hides_cli_mode_when_feature_is_not_compiled() {
        let mut state = ProviderModalState::new();
        state.set_claude_cli_status(ClaudeCliStatus::FeatureNotCompiled);
        let rendered = rendered_modal(&mut state);

        assert!(!rendered.contains("Claude Agent CLI"));
    }

    #[test]
    fn anthropic_card_renders_actionable_cli_gate_status() {
        let mut state = ProviderModalState::new();
        state.set_claude_cli_status(ClaudeCliStatus::OptInMissing);
        let rendered = rendered_modal(&mut state);

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
                chatgpt_account_email: None,
                chatgpt_models: Vec::new(),
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
    #[test]
    fn chatgpt_account_email_renders_but_is_redacted_from_provider_state_debug() {
        let mut state = ProviderModalState::new();
        state.selected = 1;
        state.set_status(&ProviderKind::OpenAi, connected_openai(Vec::new()));

        let rendered = rendered_modal(&mut state);
        assert!(rendered.contains("Connected with ChatGPT OAuth — user@example.com"));
        assert!(rendered.contains("s subscription"));
        insta::assert_snapshot!("chatgpt_openai_card_with_account_email", rendered);

        let debug = format!("{state:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("user@example.com"));
    }

    #[test]
    fn chatgpt_account_email_rejects_invalid_values() {
        assert!(ChatgptAccountEmail::new("user@example.com").is_some());
        assert!(ChatgptAccountEmail::new("user@example.com\nforged").is_none());
        assert!(ChatgptAccountEmail::new("not-an-email").is_none());
        assert!(ChatgptAccountEmail::new("a@b@c.com").is_none());
        assert!(ChatgptAccountEmail::new("").is_none());
        assert!(ChatgptAccountEmail::new("user@ex ample.com").is_none());
    }

    #[test]
    fn chatgpt_subscription_is_unavailable_until_chatgpt_oauth_connects() {
        let mut state = ProviderModalState::new();
        state.selected = 1;
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Char('s'))),
            ProviderModalOutcome::Unchanged
        );
        assert!(matches!(state.mode, ProviderModalMode::Browse));
        let rendered = rendered_modal(&mut state);
        assert!(!rendered.contains("s subscription"));
    }

    #[test]
    fn chatgpt_subscription_keys_set_and_clear_context_window() {
        let mut state = ProviderModalState::new();
        state.selected = 1;
        state.set_status(&ProviderKind::OpenAi, connected_openai(vec![sol_model()]));
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Char('s'))),
            ProviderModalOutcome::Changed
        );
        assert!(state.mode.owns_escape());
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Enter)),
            ProviderModalOutcome::Changed
        );
        for digit in "1000000".chars() {
            handle_key(&mut state, &key(KeyCode::Char(digit)));
        }
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Enter)),
            ProviderModalOutcome::Command(ProviderCommand::SetChatgptContextWindow {
                model_id: "chatgpt-gpt-5.6-sol".into(),
                tokens: Some(1_000_000),
            })
        );
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Char('x'))),
            ProviderModalOutcome::Command(ProviderCommand::SetChatgptContextWindow {
                model_id: "chatgpt-gpt-5.6-sol".into(),
                tokens: None,
            })
        );
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Esc)),
            ProviderModalOutcome::Changed
        );
        assert!(matches!(state.mode, ProviderModalMode::Browse));
    }

    #[test]
    fn chatgpt_subscription_rejects_out_of_range_and_non_integer_overrides() {
        let mut state = ProviderModalState::new();
        state.selected = 1;
        state.set_status(&ProviderKind::OpenAi, connected_openai(vec![sol_model()]));
        handle_key(&mut state, &key(KeyCode::Char('s')));
        handle_key(&mut state, &key(KeyCode::Enter));
        for digit in "7999".chars() {
            handle_key(&mut state, &key(KeyCode::Char(digit)));
        }
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Enter)),
            ProviderModalOutcome::Unchanged
        );
        handle_key(&mut state, &key(KeyCode::Esc));
        handle_key(&mut state, &key(KeyCode::Enter));
        for digit in "1050001".chars() {
            handle_key(&mut state, &key(KeyCode::Char(digit)));
        }
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Enter)),
            ProviderModalOutcome::Unchanged
        );
        handle_key(&mut state, &key(KeyCode::Esc));
        handle_key(&mut state, &key(KeyCode::Enter));
        handle_paste(&mut state, "abc");
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Enter)),
            ProviderModalOutcome::Unchanged
        );
        handle_key(&mut state, &key(KeyCode::Esc));
        handle_key(&mut state, &key(KeyCode::Enter));
        for digit in "8000".chars() {
            handle_key(&mut state, &key(KeyCode::Char(digit)));
        }
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Enter)),
            ProviderModalOutcome::Command(ProviderCommand::SetChatgptContextWindow {
                model_id: "chatgpt-gpt-5.6-sol".into(),
                tokens: Some(8_000),
            })
        );
    }

    #[test]
    fn chatgpt_subscription_missing_selection_stays_in_sub_view() {
        let mut state = ProviderModalState::new();
        state.selected = 1;
        state.set_status(&ProviderKind::OpenAi, connected_openai(Vec::new()));
        state.mode = ProviderModalMode::ChatgptSubscription {
            selected: 0,
            editor: Some(LineEditor::default()),
        };
        for digit in "1000000".chars() {
            handle_key(&mut state, &key(KeyCode::Char(digit)));
        }
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Enter)),
            ProviderModalOutcome::Unchanged
        );
        assert!(matches!(
            state.mode,
            ProviderModalMode::ChatgptSubscription { .. }
        ));

        state.mode = ProviderModalMode::ChatgptSubscription {
            selected: 0,
            editor: None,
        };
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Char('x'))),
            ProviderModalOutcome::Unchanged
        );
        assert!(matches!(
            state.mode,
            ProviderModalMode::ChatgptSubscription { .. }
        ));
    }

    #[test]
    fn chatgpt_subscription_clamps_selected_when_model_list_shrinks() {
        let mut state = ProviderModalState::new();
        state.selected = 1;
        state.set_status(
            &ProviderKind::OpenAi,
            connected_openai(vec![
                sol_model(),
                ChatgptModel::from_catalog(
                    "chatgpt-gpt-5.5".into(),
                    "GPT-5.5".into(),
                    Some(400_000),
                ),
            ]),
        );
        handle_key(&mut state, &key(KeyCode::Char('s')));
        handle_key(&mut state, &key(KeyCode::Char('j')));
        assert!(matches!(
            state.mode,
            ProviderModalMode::ChatgptSubscription {
                selected: 1,
                editor: None
            }
        ));
        state.set_status(&ProviderKind::OpenAi, connected_openai(vec![sol_model()]));
        assert!(matches!(
            state.mode,
            ProviderModalMode::ChatgptSubscription {
                selected: 0,
                editor: None
            }
        ));
    }

    #[test]
    fn chatgpt_subscription_navigates_models_with_j_k() {
        let mut state = ProviderModalState::new();
        state.selected = 1;
        state.set_status(
            &ProviderKind::OpenAi,
            connected_openai(vec![
                sol_model(),
                ChatgptModel::from_catalog(
                    "chatgpt-gpt-5.5".into(),
                    "GPT-5.5".into(),
                    Some(400_000),
                ),
            ]),
        );
        handle_key(&mut state, &key(KeyCode::Char('s')));
        assert!(matches!(
            state.mode,
            ProviderModalMode::ChatgptSubscription {
                selected: 0,
                editor: None
            }
        ));
        handle_key(&mut state, &key(KeyCode::Char('j')));
        assert!(matches!(
            state.mode,
            ProviderModalMode::ChatgptSubscription {
                selected: 1,
                editor: None
            }
        ));
        handle_key(&mut state, &key(KeyCode::Char('k')));
        assert!(matches!(
            state.mode,
            ProviderModalMode::ChatgptSubscription {
                selected: 0,
                editor: None
            }
        ));
    }

    #[test]
    fn chatgpt_subscription_render_shows_account_models_and_long_context_note() {
        let mut state = ProviderModalState::new();
        state.selected = 1;
        state.set_status(&ProviderKind::OpenAi, connected_openai(vec![sol_model()]));
        handle_key(&mut state, &key(KeyCode::Char('s')));
        let list = rendered_modal(&mut state);
        assert!(list.contains("ChatGPT subscription context windows"));
        assert!(list.contains("GPT-5.6 Sol"));
        assert!(list.contains("272000 tokens"));
        assert!(list.contains("↵ set override"));
        insta::assert_snapshot!("chatgpt_subscription_model_list", list);

        handle_key(&mut state, &key(KeyCode::Enter));
        for digit in "300000".chars() {
            handle_key(&mut state, &key(KeyCode::Char(digit)));
        }
        let editing = rendered_modal(&mut state);
        assert!(editing.contains("long-context limits or pricing"));
        insta::assert_snapshot!("chatgpt_subscription_long_context_note", editing);
    }
}
