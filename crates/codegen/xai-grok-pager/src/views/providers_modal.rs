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
    /// User-configured provider id (e.g. `local_vllm`, `zai-model-api`).
    Configured(String),
}

impl ProviderKind {
    pub const BUILTINS: [Self; 3] = [Self::Xai, Self::OpenAi, Self::OpenRouter];

    /// Default browse list before configured providers are loaded.
    pub const ALL: [Self; 3] = Self::BUILTINS;

    pub fn label(&self) -> String {
        match self {
            Self::Xai => "xAI".into(),
            Self::OpenAi => "OpenAI".into(),
            Self::OpenRouter => "OpenRouter".into(),
            Self::Configured(id) => id.clone(),
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Self::Xai => "Grok/xAI account (OAuth) or xAI API key".into(),
            Self::OpenAi => "ChatGPT OAuth or API key · Responses".into(),
            Self::OpenRouter => "Chat Completions · key stored securely".into(),
            Self::Configured(id) if id == "zai-model-api" || id == "zai" => {
                "Z.ai Model API · Chat Completions · key stored securely".into()
            }
            Self::Configured(_) => {
                "OpenAI-compatible · app/admin keys · catalog & capabilities".into()
            }
        }
    }

    pub fn needs_api_key(&self) -> bool {
        matches!(self, Self::OpenAi | Self::OpenRouter | Self::Configured(_))
    }

    pub fn id_str(&self) -> &str {
        match self {
            Self::Xai => "xai",
            Self::OpenAi => "openai",
            Self::OpenRouter => "openrouter",
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
            .field(
                "mode",
                &match &self.mode {
                    ProviderModalMode::Browse => "browse",
                    ProviderModalMode::ChoosingXai { .. } => "choosing_xai_auth",
                    ProviderModalMode::ChoosingOpenAi { .. } => "choosing_openai_auth",
                    ProviderModalMode::EditingKey { .. } => "editing_key_redacted",
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

impl ProviderModalState {
    pub fn new() -> Self {
        let rows: Vec<ProviderKind> = ProviderKind::BUILTINS.to_vec();
        let n = rows.len();
        Self {
            window: ModalWindowState::new(),
            selected: 0,
            rows,
            statuses: vec![ProviderStatus::Missing; n],
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
    pub fn set_status(&mut self, provider: ProviderKind, status: ProviderStatus) {
        if let Some(idx) = self.provider_index(&provider) {
            if idx >= self.statuses.len() {
                self.statuses.resize(idx + 1, ProviderStatus::Missing);
            }
            self.statuses[idx] = status;
        }
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
            state.set_status(ProviderKind::OpenAi, ProviderStatus::Connecting);
            Some(ProviderModalOutcome::Command(ProviderCommand::LoginCodex))
        }
        _ => Some(ProviderModalOutcome::Unchanged),
    }
}

pub fn handle_key(state: &mut ProviderModalState, key: &KeyEvent) -> ProviderModalOutcome {
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
                    ProviderStatus::Connected { .. } => ProviderCommand::ReplaceKey(provider.clone()),
                    _ => ProviderCommand::Connect(provider.clone()),
                };
                state.set_status(provider, ProviderStatus::Connecting);
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
            state.set_status(provider.clone(), ProviderStatus::Connecting);
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
            state.set_status(provider.clone(), ProviderStatus::Missing);
            // OpenAI: clear ChatGPT OAuth and/or API key (mutual exclusion store).
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
        // ChatGPT OAuth or API key (mutually exclusive).
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
        state.set_status(provider, ProviderStatus::Connecting);
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
    put_line(
        buf,
        content.content,
        &mut y,
        "Configure model providers. Keys are masked and are never written to config.toml.",
        Style::default().fg(theme.gray),
    );
    for (idx, provider) in state.rows.iter().enumerate() {
        if y.saturating_add(2) >= content.content.y.saturating_add(content.content.height) {
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
        put_line(
            buf,
            content.content,
            &mut y,
            &format!("    {detail}"),
            detail_style,
        );
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
            "Connect OpenAI (one method at a time):",
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
            "Switching methods clears the other credential.",
            Style::default().fg(theme.gray_dim),
        );
    } else {
        put_line(
            buf,
            content.content,
            &mut y,
            "xAI: OAuth or API key. OpenAI: ChatGPT login or API key. OpenRouter: API key.",
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
            state.take_submitted_secret(&ProviderKind::OpenAi).as_deref(),
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
            ProviderKind::OpenRouter,
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
            ProviderKind::OpenRouter,
            ProviderStatus::Error("invalid key".into()),
        );
        assert_eq!(
            state.status(&ProviderKind::OpenRouter).label(),
            "Connection error"
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
