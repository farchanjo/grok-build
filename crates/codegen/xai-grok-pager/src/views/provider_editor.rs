//! Typed provider editor pages (General / Authentication / Catalog /
//! Capabilities / Headers / OpenRouter Policy / References).
//!
//! Secrets never enter durable editor state: empty secret fields mean
//! preserve; explicit clear is a separate command. Secret values live only in
//! the one-shot `submitted_*_secret` slots on the parent modal and are consumed
//! by effects.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use xai_grok_shell::provider_registry::management::dto::{
    CapabilityStatusSnapshot, CatalogStatusSnapshot, CredentialSlotUpdate, ProviderCreditsSnapshot,
    ProviderDetailDto, ProviderEditorPage, ProviderSavePatch, ProviderStatusSnapshot,
    ReferenceImpactSnapshot, RegistryGeneration, SecretFieldUpdate,
};

use crate::input::line_editor::{LineEditOutcome, LineEditor};
use crate::theme::Theme;

/// Focusable field on the active editor page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorField {
    DisplayName,
    Kind,
    BaseUrl,
    AdminBaseUrl,
    Enabled,
    DefaultBackend,
    AuthScheme,
    EnvKey,
    AdminEnvKey,
    CatalogEnabled,
    CapabilityMode,
    CatalogTtl,
    RequestTimeout,
    Organization,
    Project,
    HeadersSummary,
    CapChat,
    CapResponses,
    CapEmbeddings,
    CapAdmin,
    OrDataCollection,
    OrRequireParams,
    OrAllowFallbacks,
    OrZdr,
    OrSort,
    OrPacing,
    OrFallbacks,
    AppKeySlot,
    AdminKeySlot,
    Save,
    Test,
    RefreshCatalog,
    RefreshCapabilities,
    Credits,
    ToggleEnabled,
    Clone,
    References,
}

impl EditorField {
    fn label(self) -> &'static str {
        match self {
            Self::DisplayName => "Display name",
            Self::Kind => "Kind",
            Self::BaseUrl => "Base URL",
            Self::AdminBaseUrl => "Admin base URL",
            Self::Enabled => "Enabled",
            Self::DefaultBackend => "Default backend",
            Self::AuthScheme => "Auth scheme",
            Self::EnvKey => "Env key name",
            Self::AdminEnvKey => "Admin env key name",
            Self::CatalogEnabled => "Catalog enabled",
            Self::CapabilityMode => "Capability mode",
            Self::CatalogTtl => "Catalog TTL secs",
            Self::RequestTimeout => "Request timeout secs",
            Self::Organization => "Organization",
            Self::Project => "Project",
            Self::HeadersSummary => "Extra headers",
            Self::CapChat => "Capability: chat_completions",
            Self::CapResponses => "Capability: responses",
            Self::CapEmbeddings => "Capability: embeddings",
            Self::CapAdmin => "Capability: admin",
            Self::OrDataCollection => "OR data_collection",
            Self::OrRequireParams => "OR require_parameters",
            Self::OrAllowFallbacks => "OR allow_fallbacks",
            Self::OrZdr => "OR zdr",
            Self::OrSort => "OR sort",
            Self::OrPacing => "OR pacing",
            Self::OrFallbacks => "OR fallback models",
            Self::AppKeySlot => "Application key",
            Self::AdminKeySlot => "Admin key",
            Self::Save => "Save changes",
            Self::Test => "Test connection",
            Self::RefreshCatalog => "Refresh catalog",
            Self::RefreshCapabilities => "Refresh capabilities",
            Self::Credits => "Refresh credits",
            Self::ToggleEnabled => "Enable / Disable",
            Self::Clone => "Clone instance",
            Self::References => "Reference impact",
        }
    }
}

fn fields_for_page(page: ProviderEditorPage) -> &'static [EditorField] {
    match page {
        ProviderEditorPage::General => &[
            EditorField::DisplayName,
            EditorField::Kind,
            EditorField::BaseUrl,
            EditorField::AdminBaseUrl,
            EditorField::Enabled,
            EditorField::DefaultBackend,
            EditorField::Save,
            EditorField::ToggleEnabled,
            EditorField::Clone,
        ],
        ProviderEditorPage::Authentication => &[
            EditorField::AuthScheme,
            EditorField::EnvKey,
            EditorField::AdminEnvKey,
            EditorField::AppKeySlot,
            EditorField::AdminKeySlot,
            EditorField::Save,
            EditorField::Test,
        ],
        ProviderEditorPage::Catalog => &[
            EditorField::CatalogEnabled,
            EditorField::CatalogTtl,
            EditorField::RequestTimeout,
            EditorField::RefreshCatalog,
            EditorField::Save,
        ],
        ProviderEditorPage::Capabilities => &[
            EditorField::CapabilityMode,
            EditorField::CapChat,
            EditorField::CapResponses,
            EditorField::CapEmbeddings,
            EditorField::CapAdmin,
            EditorField::RefreshCapabilities,
            EditorField::Save,
        ],
        ProviderEditorPage::Headers => &[
            EditorField::HeadersSummary,
            EditorField::Organization,
            EditorField::Project,
            EditorField::Save,
        ],
        ProviderEditorPage::OpenRouterPolicy => &[
            EditorField::OrDataCollection,
            EditorField::OrRequireParams,
            EditorField::OrAllowFallbacks,
            EditorField::OrZdr,
            EditorField::OrSort,
            EditorField::OrPacing,
            EditorField::OrFallbacks,
            EditorField::Credits,
            EditorField::Save,
        ],
        ProviderEditorPage::References => &[EditorField::References, EditorField::Clone],
    }
}

/// Draft state for the typed editor (secret-free except one-shot slots).
#[derive(Clone)]
pub struct ProviderEditorState {
    pub page: ProviderEditorPage,
    pub field_index: usize,
    pub detail: ProviderDetailDto,
    pub draft: ProviderSavePatch,
    pub status: Option<ProviderStatusSnapshot>,
    pub catalog: Option<CatalogStatusSnapshot>,
    pub capabilities: Option<CapabilityStatusSnapshot>,
    pub credits: Option<ProviderCreditsSnapshot>,
    pub references: Option<ReferenceImpactSnapshot>,
    pub message: Option<String>,
    pub error: Option<String>,
    pub editing: bool,
    editor: LineEditor,
    /// One-shot application secret (never Debug-printed).
    submitted_app_secret: Option<String>,
    submitted_admin_secret: Option<String>,
    pub clear_app_key: bool,
    pub clear_admin_key: bool,
    /// Clone id draft when Clone action is active.
    pub clone_id_draft: String,
}

impl std::fmt::Debug for ProviderEditorState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderEditorState")
            .field("page", &self.page)
            .field("field_index", &self.field_index)
            .field("provider_id", &self.detail.id)
            .field("generation", &self.detail.generation)
            .field("editing", &self.editing)
            .field("message", &self.message)
            .field("error", &self.error)
            .field("has_app_secret", &self.submitted_app_secret.is_some())
            .field("has_admin_secret", &self.submitted_admin_secret.is_some())
            .field("clear_app_key", &self.clear_app_key)
            .field("clear_admin_key", &self.clear_admin_key)
            .finish_non_exhaustive()
    }
}

impl ProviderEditorState {
    pub fn new(detail: ProviderDetailDto) -> Self {
        let draft = ProviderSavePatch {
            display_name: detail.display_name.clone(),
            kind: Some(detail.kind.clone()),
            base_url: detail.base_url.clone(),
            admin_base_url: detail.admin_base_url.clone(),
            enabled: Some(detail.enabled),
            default_backend: detail.default_backend.clone(),
            auth_scheme: detail.auth_scheme.clone(),
            env_key: detail.env_key.clone(),
            admin_env_key: detail.admin_env_key.clone(),
            catalog_enabled: Some(detail.catalog_enabled),
            capability_mode: detail.capability_mode.clone(),
            catalog_ttl_secs: detail.catalog_ttl_secs,
            request_timeout_secs: detail.request_timeout_secs,
            organization: detail.organization.clone(),
            project: detail.project.clone(),
            extra_headers: Some(detail.extra_headers.clone()),
            capabilities: Some(detail.capabilities.clone()),
            openrouter_fallback_models: Some(detail.openrouter_fallback_models.clone()),
            openrouter_data_collection: Some(detail.openrouter_data_collection.clone()),
            openrouter_require_parameters: Some(detail.openrouter_require_parameters),
            openrouter_allow_fallbacks: Some(detail.openrouter_allow_fallbacks),
            openrouter_zdr: Some(detail.openrouter_zdr),
            openrouter_order: Some(detail.openrouter_order.clone()),
            openrouter_only: Some(detail.openrouter_only.clone()),
            openrouter_ignore: Some(detail.openrouter_ignore.clone()),
            openrouter_quantizations: Some(detail.openrouter_quantizations.clone()),
            openrouter_sort: Some(detail.openrouter_sort.clone()),
            openrouter_pacing: Some(detail.openrouter_pacing),
            ..Default::default()
        };
        Self {
            page: ProviderEditorPage::General,
            field_index: 0,
            detail,
            draft,
            status: None,
            catalog: None,
            capabilities: None,
            credits: None,
            references: None,
            message: None,
            error: None,
            editing: false,
            editor: LineEditor::default(),
            submitted_app_secret: None,
            submitted_admin_secret: None,
            clear_app_key: false,
            clear_admin_key: false,
            clone_id_draft: String::new(),
        }
    }

    pub fn generation(&self) -> RegistryGeneration {
        self.detail.generation
    }

    pub fn fields(&self) -> &'static [EditorField] {
        fields_for_page(self.page)
    }

    pub fn focused_field(&self) -> EditorField {
        let fields = self.fields();
        fields[self.field_index.min(fields.len().saturating_sub(1))]
    }

    pub fn take_app_secret(&mut self) -> Option<String> {
        self.submitted_app_secret.take()
    }

    pub fn take_admin_secret(&mut self) -> Option<String> {
        self.submitted_admin_secret.take()
    }

    pub fn credential_slot_update(&self) -> CredentialSlotUpdate {
        let application = if self.clear_app_key {
            SecretFieldUpdate::Clear
        } else if self.submitted_app_secret.is_some() {
            SecretFieldUpdate::Set
        } else {
            SecretFieldUpdate::Preserve
        };
        let admin = if self.clear_admin_key {
            SecretFieldUpdate::Clear
        } else if self.submitted_admin_secret.is_some() {
            SecretFieldUpdate::Set
        } else {
            SecretFieldUpdate::Preserve
        };
        CredentialSlotUpdate {
            application,
            admin,
            oauth: SecretFieldUpdate::Preserve,
        }
    }

    fn begin_edit_current(&mut self) {
        let field = self.focused_field();
        let initial = match field {
            EditorField::DisplayName => self.draft.display_name.clone().unwrap_or_default(),
            EditorField::Kind => self.draft.kind.clone().unwrap_or_default(),
            EditorField::BaseUrl => self.draft.base_url.clone().unwrap_or_default(),
            EditorField::AdminBaseUrl => self.draft.admin_base_url.clone().unwrap_or_default(),
            EditorField::DefaultBackend => self.draft.default_backend.clone().unwrap_or_default(),
            EditorField::AuthScheme => self.draft.auth_scheme.clone().unwrap_or_default(),
            EditorField::EnvKey => self.draft.env_key.clone().unwrap_or_default(),
            EditorField::AdminEnvKey => self.draft.admin_env_key.clone().unwrap_or_default(),
            EditorField::CapabilityMode => self.draft.capability_mode.clone().unwrap_or_default(),
            EditorField::CatalogTtl => self
                .draft
                .catalog_ttl_secs
                .map(|v| v.to_string())
                .unwrap_or_default(),
            EditorField::RequestTimeout => self
                .draft
                .request_timeout_secs
                .map(|v| v.to_string())
                .unwrap_or_default(),
            EditorField::Organization => self.draft.organization.clone().unwrap_or_default(),
            EditorField::Project => self.draft.project.clone().unwrap_or_default(),
            EditorField::OrDataCollection => self
                .draft
                .openrouter_data_collection
                .clone()
                .flatten()
                .unwrap_or_default(),
            EditorField::OrSort => self
                .draft
                .openrouter_sort
                .clone()
                .flatten()
                .unwrap_or_default(),
            EditorField::OrFallbacks => self
                .draft
                .openrouter_fallback_models
                .clone()
                .unwrap_or_default()
                .join(", "),
            EditorField::HeadersSummary => self
                .draft
                .extra_headers
                .as_ref()
                .map(|h| {
                    h.iter()
                        .map(|(k, v)| format!("{k}={v}"))
                        .collect::<Vec<_>>()
                        .join("; ")
                })
                .unwrap_or_default(),
            EditorField::AppKeySlot | EditorField::AdminKeySlot => String::new(),
            EditorField::Clone => self.clone_id_draft.clone(),
            _ => return,
        };
        self.editor = LineEditor::default();
        let _ = self.editor.insert_paste_with_byte_limit(&initial, 16_384);
        self.editing = true;
    }

    fn commit_edit(&mut self) {
        let text = self.editor.text().to_owned();
        let field = self.focused_field();
        match field {
            EditorField::DisplayName => {
                self.draft.display_name = if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                };
            }
            EditorField::Kind => {
                self.draft.kind = Some(text);
            }
            EditorField::BaseUrl => {
                self.draft.base_url = if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                };
            }
            EditorField::AdminBaseUrl => {
                self.draft.admin_base_url = if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                };
            }
            EditorField::DefaultBackend => {
                self.draft.default_backend = if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                };
            }
            EditorField::AuthScheme => {
                self.draft.auth_scheme = if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                };
            }
            EditorField::EnvKey => {
                self.draft.env_key = if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                };
            }
            EditorField::AdminEnvKey => {
                self.draft.admin_env_key = if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                };
            }
            EditorField::CapabilityMode => {
                self.draft.capability_mode = if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                };
            }
            EditorField::CatalogTtl => {
                self.draft.catalog_ttl_secs = text.trim().parse().ok();
            }
            EditorField::RequestTimeout => {
                self.draft.request_timeout_secs = text.trim().parse().ok();
            }
            EditorField::Organization => {
                self.draft.organization = if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                };
            }
            EditorField::Project => {
                self.draft.project = if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                };
            }
            EditorField::OrDataCollection => {
                self.draft.openrouter_data_collection = Some(if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                });
            }
            EditorField::OrSort => {
                self.draft.openrouter_sort = Some(if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                });
            }
            EditorField::OrFallbacks => {
                let models: Vec<String> = text
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect();
                self.draft.openrouter_fallback_models = Some(models);
            }
            EditorField::HeadersSummary => {
                let mut map = indexmap::IndexMap::new();
                for part in text.split(';') {
                    let part = part.trim();
                    if part.is_empty() {
                        continue;
                    }
                    if let Some((k, v)) = part.split_once('=') {
                        map.insert(k.trim().to_owned(), v.trim().to_owned());
                    }
                }
                self.draft.extra_headers = Some(map);
            }
            EditorField::AppKeySlot => {
                if text.trim().is_empty() {
                    // Empty means preserve (never accidental clear).
                    self.submitted_app_secret = None;
                } else {
                    self.submitted_app_secret = Some(text);
                    self.clear_app_key = false;
                }
            }
            EditorField::AdminKeySlot => {
                if text.trim().is_empty() {
                    self.submitted_admin_secret = None;
                } else {
                    self.submitted_admin_secret = Some(text);
                    self.clear_admin_key = false;
                }
            }
            EditorField::Clone => {
                self.clone_id_draft = text.trim().to_owned();
            }
            _ => {}
        }
        self.editing = false;
        self.editor.reset();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorOutcome {
    Changed,
    Unchanged,
    Back,
    /// Emit a management command for the shell effect layer.
    Command(EditorCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorCommand {
    Save,
    Test,
    RefreshCatalog,
    RefreshCapabilities,
    Credits,
    ToggleEnabled,
    Clone { new_id: String },
    LoadReferences,
    ClearAppKey,
    ClearAdminKey,
}

pub fn handle_key(state: &mut ProviderEditorState, key: &KeyEvent) -> EditorOutcome {
    if state.editing {
        match key.code {
            KeyCode::Esc => {
                state.editing = false;
                state.editor.reset();
                return EditorOutcome::Changed;
            }
            KeyCode::Enter => {
                state.commit_edit();
                return EditorOutcome::Changed;
            }
            _ => {
                let outcome = state.editor.handle_key(key);
                return match outcome {
                    LineEditOutcome::TextChanged
                    | LineEditOutcome::CursorChanged
                    | LineEditOutcome::HandledNoChange => EditorOutcome::Changed,
                    LineEditOutcome::Unhandled => EditorOutcome::Unchanged,
                };
            }
        }
    }

    match key.code {
        KeyCode::Esc => EditorOutcome::Back,
        KeyCode::Tab => {
            let next = (state.page.index() + 1) % ProviderEditorPage::ALL.len();
            state.page = ProviderEditorPage::from_index(next);
            state.field_index = 0;
            EditorOutcome::Changed
        }
        KeyCode::BackTab => {
            let len = ProviderEditorPage::ALL.len();
            let next = (state.page.index() + len - 1) % len;
            state.page = ProviderEditorPage::from_index(next);
            state.field_index = 0;
            EditorOutcome::Changed
        }
        KeyCode::Left if key.modifiers.is_empty() => {
            let len = ProviderEditorPage::ALL.len();
            let next = (state.page.index() + len - 1) % len;
            state.page = ProviderEditorPage::from_index(next);
            state.field_index = 0;
            EditorOutcome::Changed
        }
        KeyCode::Right if key.modifiers.is_empty() => {
            let next = (state.page.index() + 1) % ProviderEditorPage::ALL.len();
            state.page = ProviderEditorPage::from_index(next);
            state.field_index = 0;
            EditorOutcome::Changed
        }
        KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => {
            state.field_index = state.field_index.saturating_sub(1);
            EditorOutcome::Changed
        }
        KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => {
            let max = state.fields().len().saturating_sub(1);
            state.field_index = (state.field_index + 1).min(max);
            EditorOutcome::Changed
        }
        KeyCode::Char('1'..='7') if key.modifiers.is_empty() => {
            if let KeyCode::Char(c) = key.code {
                let idx = (c as u8 - b'1') as usize;
                if idx < ProviderEditorPage::ALL.len() {
                    state.page = ProviderEditorPage::from_index(idx);
                    state.field_index = 0;
                    return EditorOutcome::Changed;
                }
            }
            EditorOutcome::Unchanged
        }
        KeyCode::Enter | KeyCode::Char(' ') if key.modifiers.is_empty() => activate_field(state),
        KeyCode::Char('s') if key.modifiers.is_empty() => {
            EditorOutcome::Command(EditorCommand::Save)
        }
        KeyCode::Char('t') if key.modifiers.is_empty() => {
            EditorOutcome::Command(EditorCommand::Test)
        }
        KeyCode::Char('r') if key.modifiers.is_empty() => {
            EditorOutcome::Command(EditorCommand::RefreshCatalog)
        }
        _ => EditorOutcome::Unchanged,
    }
}

fn activate_field(state: &mut ProviderEditorState) -> EditorOutcome {
    if !state.detail.is_editable
        && !matches!(
            state.focused_field(),
            EditorField::Test
                | EditorField::RefreshCatalog
                | EditorField::RefreshCapabilities
                | EditorField::Credits
                | EditorField::References
                | EditorField::Clone
                | EditorField::ToggleEnabled
        )
    {
        state.error = state.detail.unsupported_edit_reason.clone().or_else(|| {
            Some("This provider is not fully editable; unsupported edits fail closed.".into())
        });
        return EditorOutcome::Changed;
    }
    match state.focused_field() {
        EditorField::Enabled | EditorField::CatalogEnabled | EditorField::OrPacing => {
            toggle_bool_field(state);
            EditorOutcome::Changed
        }
        EditorField::OrRequireParams => {
            let cur = state
                .draft
                .openrouter_require_parameters
                .clone()
                .flatten()
                .unwrap_or(false);
            state.draft.openrouter_require_parameters = Some(Some(!cur));
            EditorOutcome::Changed
        }
        EditorField::OrAllowFallbacks => {
            let cur = state
                .draft
                .openrouter_allow_fallbacks
                .clone()
                .flatten()
                .unwrap_or(false);
            state.draft.openrouter_allow_fallbacks = Some(Some(!cur));
            EditorOutcome::Changed
        }
        EditorField::OrZdr => {
            let cur = state
                .draft
                .openrouter_zdr
                .clone()
                .flatten()
                .unwrap_or(false);
            state.draft.openrouter_zdr = Some(Some(!cur));
            EditorOutcome::Changed
        }
        EditorField::CapChat => toggle_cap(state, "chat_completions"),
        EditorField::CapResponses => toggle_cap(state, "responses"),
        EditorField::CapEmbeddings => toggle_cap(state, "embeddings"),
        EditorField::CapAdmin => toggle_cap(state, "admin"),
        EditorField::Save => EditorOutcome::Command(EditorCommand::Save),
        EditorField::Test => EditorOutcome::Command(EditorCommand::Test),
        EditorField::RefreshCatalog => EditorOutcome::Command(EditorCommand::RefreshCatalog),
        EditorField::RefreshCapabilities => {
            EditorOutcome::Command(EditorCommand::RefreshCapabilities)
        }
        EditorField::Credits => EditorOutcome::Command(EditorCommand::Credits),
        EditorField::ToggleEnabled => EditorOutcome::Command(EditorCommand::ToggleEnabled),
        EditorField::References => EditorOutcome::Command(EditorCommand::LoadReferences),
        EditorField::Clone => {
            if state.clone_id_draft.is_empty() {
                state.begin_edit_current();
                EditorOutcome::Changed
            } else {
                EditorOutcome::Command(EditorCommand::Clone {
                    new_id: state.clone_id_draft.clone(),
                })
            }
        }
        EditorField::AppKeySlot => {
            // Enter to set key; 'c' clear is separate via field double-action:
            // Space with empty + clear flag handled by Char('c') below is not
            // in activate — use begin_edit.
            state.begin_edit_current();
            EditorOutcome::Changed
        }
        EditorField::AdminKeySlot => {
            state.begin_edit_current();
            EditorOutcome::Changed
        }
        _ => {
            state.begin_edit_current();
            EditorOutcome::Changed
        }
    }
}

fn toggle_bool_field(state: &mut ProviderEditorState) {
    match state.focused_field() {
        EditorField::Enabled => {
            let cur = state.draft.enabled.unwrap_or(true);
            state.draft.enabled = Some(!cur);
        }
        EditorField::CatalogEnabled => {
            let cur = state.draft.catalog_enabled.unwrap_or(true);
            state.draft.catalog_enabled = Some(!cur);
        }
        EditorField::OrPacing => {
            let cur = state.draft.openrouter_pacing.unwrap_or(false);
            state.draft.openrouter_pacing = Some(!cur);
        }
        _ => {}
    }
}

fn toggle_cap(state: &mut ProviderEditorState, key: &str) -> EditorOutcome {
    let mut caps = state.draft.capabilities.clone().unwrap_or_default();
    let cur = caps.get(key).copied().unwrap_or(false);
    caps.insert(key.to_owned(), !cur);
    state.draft.capabilities = Some(caps);
    EditorOutcome::Changed
}

pub fn handle_paste(state: &mut ProviderEditorState, text: &str) -> EditorOutcome {
    if !state.editing {
        return EditorOutcome::Unchanged;
    }
    match state.editor.insert_paste_with_byte_limit(text, 16_384) {
        LineEditOutcome::TextChanged
        | LineEditOutcome::CursorChanged
        | LineEditOutcome::HandledNoChange => EditorOutcome::Changed,
        LineEditOutcome::Unhandled => EditorOutcome::Unchanged,
    }
}

pub fn render_editor(buf: &mut Buffer, area: Rect, state: &ProviderEditorState, y: &mut u16) {
    let theme = Theme::current();
    put_line(
        buf,
        area,
        y,
        &format!(
            "Edit {} · generation {} · {}",
            state.detail.id,
            state.detail.generation.get(),
            if state.detail.is_editable {
                "editable"
            } else {
                "read-mostly"
            }
        ),
        Style::default()
            .fg(theme.text_primary)
            .add_modifier(Modifier::BOLD),
    );
    // Page tabs with visible focus (not color-only: prefix › and number labels).
    let mut tab_line = String::new();
    for (i, page) in ProviderEditorPage::ALL.iter().enumerate() {
        let mark = if *page == state.page { "›" } else { " " };
        tab_line.push_str(&format!("{mark}{}.{} ", i + 1, page.label()));
    }
    put_line(
        buf,
        area,
        y,
        &tab_line,
        Style::default().fg(theme.accent_user),
    );
    put_line(
        buf,
        area,
        y,
        "Tab/←→ pages · ↑↓ fields · Enter activate · s save · t test · r catalog · Esc back",
        Style::default().fg(theme.gray_dim),
    );

    if let Some(err) = &state.error {
        put_line(
            buf,
            area,
            y,
            &format!("Error: {err}"),
            Style::default().fg(theme.accent_error),
        );
    }
    if let Some(msg) = &state.message {
        put_line(buf, area, y, msg, Style::default().fg(theme.accent_success));
    }

    let fields = state.fields();
    for (idx, field) in fields.iter().enumerate() {
        let focused = idx == state.field_index && !state.editing;
        let prefix = if focused { "› " } else { "  " };
        let value = field_value_display(state, *field);
        let style = if focused {
            Style::default()
                .fg(theme.text_primary)
                .bg(theme.bg_highlight)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.gray)
        };
        put_line(
            buf,
            area,
            y,
            &format!("{prefix}{}: {value}", field.label()),
            style,
        );
    }

    if state.editing {
        let masked = matches!(
            state.focused_field(),
            EditorField::AppKeySlot | EditorField::AdminKeySlot
        );
        let shown = if masked {
            "•".repeat(state.editor.text().chars().count())
        } else {
            state.editor.text().to_owned()
        };
        put_line(
            buf,
            area,
            y,
            &format!("Editing (Enter commit · Esc cancel): {shown}"),
            Style::default().fg(theme.accent_user),
        );
        if masked {
            put_line(
                buf,
                area,
                y,
                "Empty secret field preserves the stored credential; use Clear action to remove.",
                Style::default().fg(theme.gray_dim),
            );
        }
    }

    // Status panels for Catalog / Auth / References pages.
    match state.page {
        ProviderEditorPage::Catalog => {
            if let Some(c) = &state.catalog {
                put_line(
                    buf,
                    area,
                    y,
                    &format!(
                        "Catalog: models={:?} source={:?} {}",
                        c.model_count,
                        c.source,
                        c.error.clone().unwrap_or_default()
                    ),
                    Style::default().fg(theme.gray),
                );
                for sample in c.sample_model_ids.iter().take(5) {
                    put_line(
                        buf,
                        area,
                        y,
                        &format!("  · {sample}"),
                        Style::default().fg(theme.gray_dim),
                    );
                }
            }
        }
        ProviderEditorPage::Authentication => {
            let creds = &state.detail.credentials;
            put_line(
                buf,
                area,
                y,
                &format!(
                    "Slots: app={} admin={} oauth={} (values never shown)",
                    yn(creds.has_application_key),
                    yn(creds.has_admin_key),
                    yn(creds.has_oauth)
                ),
                Style::default().fg(theme.gray),
            );
            if let Some(s) = &state.status {
                put_line(
                    buf,
                    area,
                    y,
                    &format!(
                        "Status: {} {}",
                        s.label,
                        s.detail.clone().or(s.error.clone()).unwrap_or_default()
                    ),
                    Style::default().fg(if s.connected {
                        theme.accent_success
                    } else {
                        theme.gray
                    }),
                );
            }
        }
        ProviderEditorPage::OpenRouterPolicy => {
            if let Some(c) = &state.credits {
                put_line(
                    buf,
                    area,
                    y,
                    &format!(
                        "Credits: {} {}",
                        if c.available { "available" } else { "n/a" },
                        c.summary.clone().or(c.error.clone()).unwrap_or_default()
                    ),
                    Style::default().fg(theme.gray),
                );
            }
        }
        ProviderEditorPage::References => {
            if let Some(r) = &state.references {
                put_line(
                    buf,
                    area,
                    y,
                    &format!(
                        "Remove ready: {} · caches={} secrets={}",
                        if r.can_remove { "yes" } else { "no" },
                        yn(r.cache_present),
                        yn(r.secrets_present)
                    ),
                    Style::default().fg(theme.gray),
                );
                put_line(
                    buf,
                    area,
                    y,
                    &r.guidance,
                    Style::default().fg(theme.gray_dim),
                );
                for m in &r.model_references {
                    put_line(
                        buf,
                        area,
                        y,
                        &format!("  model ref: {m}"),
                        Style::default().fg(theme.warning),
                    );
                }
            } else {
                put_line(
                    buf,
                    area,
                    y,
                    "Press Enter on Reference impact to load preliminary remove readiness.",
                    Style::default().fg(theme.gray_dim),
                );
            }
        }
        _ => {}
    }
}

fn field_value_display(state: &ProviderEditorState, field: EditorField) -> String {
    match field {
        EditorField::DisplayName => state
            .draft
            .display_name
            .clone()
            .unwrap_or_else(|| "(none)".into()),
        EditorField::Kind => state.draft.kind.clone().unwrap_or_default(),
        EditorField::BaseUrl => state.draft.base_url.clone().unwrap_or_default(),
        EditorField::AdminBaseUrl => state.draft.admin_base_url.clone().unwrap_or_default(),
        EditorField::Enabled => yn(state.draft.enabled.unwrap_or(true)).into(),
        EditorField::DefaultBackend => state.draft.default_backend.clone().unwrap_or_default(),
        EditorField::AuthScheme => state.draft.auth_scheme.clone().unwrap_or_default(),
        EditorField::EnvKey => state.draft.env_key.clone().unwrap_or_default(),
        EditorField::AdminEnvKey => state.draft.admin_env_key.clone().unwrap_or_default(),
        EditorField::CatalogEnabled => yn(state.draft.catalog_enabled.unwrap_or(true)).into(),
        EditorField::CapabilityMode => state.draft.capability_mode.clone().unwrap_or_default(),
        EditorField::CatalogTtl => state
            .draft
            .catalog_ttl_secs
            .map(|v| v.to_string())
            .unwrap_or_else(|| "(default)".into()),
        EditorField::RequestTimeout => state
            .draft
            .request_timeout_secs
            .map(|v| v.to_string())
            .unwrap_or_else(|| "(default)".into()),
        EditorField::Organization => state.draft.organization.clone().unwrap_or_default(),
        EditorField::Project => state.draft.project.clone().unwrap_or_default(),
        EditorField::HeadersSummary => {
            let n = state
                .draft
                .extra_headers
                .as_ref()
                .map(|h| h.len())
                .unwrap_or(0);
            format!("{n} header(s)")
        }
        EditorField::CapChat => cap_yn(state, "chat_completions"),
        EditorField::CapResponses => cap_yn(state, "responses"),
        EditorField::CapEmbeddings => cap_yn(state, "embeddings"),
        EditorField::CapAdmin => cap_yn(state, "admin"),
        EditorField::OrDataCollection => state
            .draft
            .openrouter_data_collection
            .clone()
            .flatten()
            .unwrap_or_else(|| "(unset)".into()),
        EditorField::OrRequireParams => {
            yn_opt(state.draft.openrouter_require_parameters.clone().flatten())
        }
        EditorField::OrAllowFallbacks => {
            yn_opt(state.draft.openrouter_allow_fallbacks.clone().flatten())
        }
        EditorField::OrZdr => yn_opt(state.draft.openrouter_zdr.clone().flatten()),
        EditorField::OrSort => state
            .draft
            .openrouter_sort
            .clone()
            .flatten()
            .unwrap_or_else(|| "(unset)".into()),
        EditorField::OrPacing => yn(state.draft.openrouter_pacing.unwrap_or(false)).into(),
        EditorField::OrFallbacks => {
            let n = state
                .draft
                .openrouter_fallback_models
                .as_ref()
                .map(|v| v.len())
                .unwrap_or(0);
            format!("{n} fallback model(s)")
        }
        EditorField::AppKeySlot => {
            if state.clear_app_key {
                "will clear".into()
            } else if state.submitted_app_secret.is_some() {
                "new value pending (masked)".into()
            } else if state.detail.credentials.has_application_key {
                "stored (empty field preserves)".into()
            } else {
                "missing".into()
            }
        }
        EditorField::AdminKeySlot => {
            if state.clear_admin_key {
                "will clear".into()
            } else if state.submitted_admin_secret.is_some() {
                "new value pending (masked)".into()
            } else if state.detail.credentials.has_admin_key {
                "stored (empty field preserves)".into()
            } else {
                "missing".into()
            }
        }
        EditorField::Save => "Enter to save (generation-tagged)".into(),
        EditorField::Test => "Enter for live non-mutating probe".into(),
        EditorField::RefreshCatalog => "Enter to refresh catalog".into(),
        EditorField::RefreshCapabilities => "Enter to refresh capabilities".into(),
        EditorField::Credits => "Enter for OpenRouter credits".into(),
        EditorField::ToggleEnabled => if state.detail.enabled {
            "currently enabled — Enter to disable"
        } else {
            "currently disabled — Enter to enable"
        }
        .into(),
        EditorField::Clone => {
            if state.clone_id_draft.is_empty() {
                "Enter to type new id".into()
            } else {
                format!("new id: {} (Enter to clone)", state.clone_id_draft)
            }
        }
        EditorField::References => "Enter to load impact".into(),
    }
}

fn cap_yn(state: &ProviderEditorState, key: &str) -> String {
    let v = state
        .draft
        .capabilities
        .as_ref()
        .and_then(|c| c.get(key).copied())
        .unwrap_or(false);
    yn(v).into()
}

fn yn(v: bool) -> &'static str {
    if v { "yes" } else { "no" }
}

fn yn_opt(v: Option<bool>) -> String {
    match v {
        Some(true) => "yes".into(),
        Some(false) => "no".into(),
        None => "(unset)".into(),
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
    use crossterm::event::KeyModifiers;
    use xai_grok_shell::provider_registry::management::dto::{
        CredentialPresence, ProviderDetailDto, RegistryGeneration,
    };

    fn sample_detail() -> ProviderDetailDto {
        ProviderDetailDto {
            id: "local_vllm".into(),
            display_name: Some("Local".into()),
            kind: "openai_compatible".into(),
            enabled: true,
            is_built_in: false,
            is_configured: true,
            is_editable: true,
            base_url: Some("http://127.0.0.1:8000/v1".into()),
            admin_base_url: None,
            default_backend: None,
            auth_scheme: Some("bearer".into()),
            env_key: None,
            admin_env_key: None,
            catalog_enabled: true,
            capability_mode: Some("auto".into()),
            catalog_ttl_secs: None,
            request_timeout_secs: None,
            organization: None,
            project: None,
            api_surface: None,
            credential_route: None,
            api_backend: None,
            auth_provider: None,
            extra_headers: indexmap::IndexMap::new(),
            capabilities: indexmap::IndexMap::new(),
            openrouter_fallback_models: Vec::new(),
            openrouter_data_collection: None,
            openrouter_require_parameters: None,
            openrouter_allow_fallbacks: None,
            openrouter_zdr: None,
            openrouter_order: Vec::new(),
            openrouter_only: Vec::new(),
            openrouter_ignore: Vec::new(),
            openrouter_quantizations: Vec::new(),
            openrouter_sort: None,
            openrouter_pacing: false,
            openrouter_plugin_ids: Vec::new(),
            credentials: CredentialPresence::default(),
            generation: RegistryGeneration(3),
            warnings: Vec::new(),
            unsupported_edit_reason: None,
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn empty_secret_preserves_and_debug_redacts() {
        let mut state = ProviderEditorState::new(sample_detail());
        state.page = ProviderEditorPage::Authentication;
        state.field_index = fields_for_page(ProviderEditorPage::Authentication)
            .iter()
            .position(|f| *f == EditorField::AppKeySlot)
            .unwrap();
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Enter)),
            EditorOutcome::Changed
        );
        assert!(state.editing);
        // Commit empty → preserve.
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Enter)),
            EditorOutcome::Changed
        );
        assert!(state.submitted_app_secret.is_none());
        assert_eq!(
            state.credential_slot_update().application,
            SecretFieldUpdate::Preserve
        );
        state.submitted_app_secret = Some("sk-secret".into());
        assert!(!format!("{state:?}").contains("sk-secret"));
    }

    #[test]
    fn page_navigation_and_save_command() {
        let mut state = ProviderEditorState::new(sample_detail());
        assert_eq!(state.page, ProviderEditorPage::General);
        handle_key(&mut state, &key(KeyCode::Tab));
        assert_eq!(state.page, ProviderEditorPage::Authentication);
        handle_key(&mut state, &key(KeyCode::Char('1')));
        assert_eq!(state.page, ProviderEditorPage::General);
        assert_eq!(
            handle_key(&mut state, &key(KeyCode::Char('s'))),
            EditorOutcome::Command(EditorCommand::Save)
        );
    }
}
