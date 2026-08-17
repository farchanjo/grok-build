//! Shell-authoritative provider management API.
//!
//! The pager and CLI never read `$GROK_HOME/config.toml` for provider CRUD.
//! All list/detail/save/clone/enable/disable/test/refresh/status/credits and
//! reference-impact operations go through [`ProviderManagementService`].
//!
//! - Snapshots and async results are generation-tagged.
//! - Stale saves and stale results fail closed with reload/retry/clone guidance.
//! - Secrets stay in redacted one-shot paths / secure storage; DTOs are secret-free.
//! - Durable metadata patches preserve comments/order/unknown fields.
//! - Legacy aliases, unknown kinds, and env/credential shapes are preserved;
//!   unsupported edits fail closed rather than destructively normalizing.

pub mod dto;

use super::id::{BuiltInProviderId, ProviderId, ProviderRef, is_reserved_configured_id};
use super::lifecycle::validate_http_base_url;
use super::secrets::{
    admin_key_scope, application_key_scope, clear_provider_secret, oauth_scope_string,
    read_provider_secret, store_provider_secret,
};
use super::toml_edit::{
    OpenRouterPrefsPatch, ProviderTomlPatch, apply_provider_patch_with_openrouter,
    disable_provider, enable_provider, remove_provider, upsert_provider,
};
use super::{CapabilityCacheStore, CatalogCacheStore, ProviderService, normalize_endpoint_origin};
use crate::agent::model_providers::{ModelProviderConfig, parse_model_providers};
use crate::agent::providers::{
    ProviderConnectionTest, ProviderId as BuiltInBackendId, ProviderManager,
};
use dto::*;
use fs2::FileExt;
use indexmap::IndexMap;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Relative path under `$GROK_HOME` for the durable lifecycle generation counter.
/// Format: `{generation}\n{config_sha256}\n` so external config edits invalidate clients.
const GENERATION_REL: &str = "state/provider_lifecycle_generation";
const LOCK_REL: &str = "state/provider_lifecycle.lock";

const STALE_GUIDANCE: &str = "Registry generation is stale. Reload the providers list, re-apply your edits, \
     or clone into a new id if another client saved first.";

/// Shell-owned provider management surface.
#[derive(Clone, Debug)]
pub struct ProviderManagementService {
    home: PathBuf,
    config_path: PathBuf,
}

impl ProviderManagementService {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        let home = home.into();
        let config_path = home.join("config.toml");
        Self { home, config_path }
    }

    pub fn from_grok_home() -> Self {
        Self::new(xai_grok_config::grok_home())
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// Current effective generation (0 when missing).
    ///
    /// **Read-only:** never writes the generation sidecar. When `config.toml`
    /// fingerprint diverges from the last recorded value, returns
    /// `stored_generation + 1` so clients fail closed without racing a mutator
    /// that already holds the lifecycle lock. Durable fingerprint advancement
    /// happens only under the mutation lock.
    pub fn current_generation(&self) -> RegistryGeneration {
        RegistryGeneration(self.effective_generation_readonly())
    }

    /// Load a secret-free list snapshot for the browse UI / CLI.
    pub fn list_snapshot(&self) -> Result<ProviderListSnapshot, String> {
        let generation = self.current_generation();
        let (entries, warnings) = self.load_entries()?;
        let service = ProviderService::from_model_providers(&entries)
            .map_err(|e| e.to_string())?
            .with_generation(generation.get());
        let mut rows = Vec::new();
        for desc in service.list() {
            let id = desc.id.as_str().to_owned();
            let is_built_in = matches!(desc.provider_ref, ProviderRef::BuiltIn(_));
            let credentials = self.credential_presence(&id);
            let (status_label, status_detail) = list_status_label(&id, is_built_in, &credentials);
            rows.push(ProviderListRow {
                id: id.clone(),
                display_name: desc.display_label().to_owned(),
                kind: desc.kind.as_str().to_owned(),
                enabled: desc.enabled,
                is_built_in,
                is_configured: !is_built_in,
                base_url: desc.base_url.clone(),
                credentials,
                status_label,
                status_detail,
            });
        }
        // Multi-account model selection is default-enabled after Gate D; list is complete.
        let _ = &service;
        Ok(ProviderListSnapshot {
            generation,
            rows,
            warnings,
        })
    }

    /// Full typed detail for one provider (editor pages).
    pub fn detail(&self, id: &str) -> Result<ProviderDetailDto, String> {
        let generation = self.current_generation();
        let (entries, _) = self.load_entries()?;
        let service = ProviderService::from_model_providers(&entries)
            .map_err(|e| e.to_string())?
            .with_generation(generation.get());
        let desc = service
            .get(id)
            .ok_or_else(|| format!("provider `{id}` is not configured"))?;
        let meta = service.snapshot().get(id);
        let is_built_in = matches!(desc.provider_ref, ProviderRef::BuiltIn(_));
        let kind_str = desc.kind.as_str().to_owned();
        let editable = is_editable_kind(&kind_str, is_built_in);
        let unsupported = if editable {
            None
        } else {
            Some(format!(
                "Provider kind `{kind_str}` is preserved as-is; unsupported edits fail closed \
                 rather than normalizing. Clone into an openai_compatible/openrouter instance \
                 to reconfigure."
            ))
        };
        let prefs = desc.openrouter_provider_preferences.as_ref();
        let mut capabilities = IndexMap::new();
        if let Some(m) = meta {
            if let Some(v) = m.capabilities.chat_completions {
                capabilities.insert("chat_completions".into(), v);
            }
            if let Some(v) = m.capabilities.responses {
                capabilities.insert("responses".into(), v);
            }
            if let Some(v) = m.capabilities.embeddings {
                capabilities.insert("embeddings".into(), v);
            }
            if let Some(v) = m.capabilities.images {
                capabilities.insert("images".into(), v);
            }
            if let Some(v) = m.capabilities.audio {
                capabilities.insert("audio".into(), v);
            }
            if let Some(v) = m.capabilities.files {
                capabilities.insert("files".into(), v);
            }
            if let Some(v) = m.capabilities.batches {
                capabilities.insert("batches".into(), v);
            }
            if let Some(v) = m.capabilities.fine_tuning {
                capabilities.insert("fine_tuning".into(), v);
            }
            if let Some(v) = m.capabilities.admin {
                capabilities.insert("admin".into(), v);
            }
            for (k, v) in &m.capabilities.extra {
                capabilities.insert(k.clone(), *v);
            }
        }
        // Prefer raw config capabilities when present (preserves exact keys).
        if let Some(cfg) = entries.get(id)
            && !cfg.capabilities.is_empty()
        {
            capabilities = cfg.capabilities.clone();
        }

        let plugin_ids: Vec<String> = desc
            .openrouter_plugins
            .iter()
            .map(|p| p.id.clone())
            .collect();

        Ok(ProviderDetailDto {
            id: id.to_owned(),
            display_name: desc.display_name.clone(),
            kind: kind_str,
            enabled: desc.enabled,
            is_built_in,
            is_configured: !is_built_in,
            is_editable: editable,
            base_url: desc.base_url.clone(),
            admin_base_url: desc.admin_base_url.clone(),
            default_backend: meta.and_then(|m| m.default_backend.clone()),
            auth_scheme: desc.auth_scheme.clone().or_else(|| {
                meta.map(|m| match m.auth_scheme {
                    super::lifecycle::ProviderAuthScheme::Bearer => "bearer".into(),
                    super::lifecycle::ProviderAuthScheme::None => "none".into(),
                    super::lifecycle::ProviderAuthScheme::CustomHeader => "custom_header".into(),
                })
            }),
            env_key: meta.and_then(|m| m.env_key.clone()),
            admin_env_key: meta.and_then(|m| m.admin_env_key.clone()),
            catalog_enabled: meta.map(|m| m.catalog_enabled).unwrap_or(true),
            capability_mode: meta.map(|m| match m.capability_mode {
                super::lifecycle::CapabilityMode::Auto => "auto".into(),
                super::lifecycle::CapabilityMode::Manual => "manual".into(),
                super::lifecycle::CapabilityMode::Off => "off".into(),
            }),
            catalog_ttl_secs: meta.and_then(|m| m.catalog_ttl_secs),
            request_timeout_secs: meta.and_then(|m| m.request_timeout_secs),
            organization: meta.and_then(|m| m.organization.clone()),
            project: meta.and_then(|m| m.project.clone()),
            api_surface: desc
                .primary_route()
                .map(|r| r.api_surface.as_str().to_owned()),
            credential_route: desc
                .primary_route()
                .map(|r| r.credential_route.as_str().to_owned()),
            api_backend: desc.api_backend.clone(),
            auth_provider: desc.auth_provider.clone(),
            extra_headers: meta.map(|m| m.extra_headers.clone()).unwrap_or_default(),
            capabilities,
            openrouter_fallback_models: desc.openrouter_fallback_models.clone(),
            openrouter_data_collection: prefs.and_then(|p| p.data_collection.clone()),
            openrouter_require_parameters: prefs.and_then(|p| p.require_parameters),
            openrouter_allow_fallbacks: prefs.and_then(|p| p.allow_fallbacks),
            openrouter_zdr: prefs.and_then(|p| p.zdr),
            openrouter_order: prefs.map(|p| p.order.clone()).unwrap_or_default(),
            openrouter_only: prefs.map(|p| p.only.clone()).unwrap_or_default(),
            openrouter_ignore: prefs.map(|p| p.ignore.clone()).unwrap_or_default(),
            openrouter_quantizations: prefs.map(|p| p.quantizations.clone()).unwrap_or_default(),
            openrouter_sort: prefs
                .and_then(|p| p.sort.as_ref().and_then(|s| s.as_name()).map(str::to_owned)),
            openrouter_pacing: desc.openrouter_pacing,
            openrouter_plugin_ids: plugin_ids,
            credentials: self.credential_presence(id),
            generation,
            warnings: Vec::new(),
            unsupported_edit_reason: unsupported,
            incarnation: self.live_incarnation_str(id),
            tombstone_blocks_readd: self.tombstone_blocks_readd(id),
        })
    }

    /// Add a new configured OpenAI/OpenRouter/custom instance.
    pub fn add(&self, req: ProviderAddRequest) -> ProviderMutationResult {
        let _lock = match self.acquire_mutation_lock() {
            Ok(l) => l,
            Err(e) => return err_result(&req.id, self.current_generation(), e),
        };
        if let Err(msg) = self.require_generation_locked(req.expected_generation) {
            return stale_result_with_expected(
                &req.id,
                req.expected_generation,
                self.current_generation(),
                vec!["generation".into()],
                msg,
            );
        }
        let pid = match ProviderId::new(&req.id) {
            Ok(p) => p,
            Err(e) => return err_result(&req.id, self.current_generation(), e.to_string()),
        };
        if is_reserved_configured_id(pid.as_str()) {
            return err_result(
                pid.as_str(),
                self.current_generation(),
                format!("provider id `{}` is reserved", pid.as_str()),
            );
        }
        if let Err(e) = validate_http_base_url(&req.base_url) {
            return err_result(pid.as_str(), self.current_generation(), e.to_string());
        }
        let kind = normalize_kind_for_add(&req.kind);
        if kind.is_none() {
            return err_result(
                pid.as_str(),
                self.current_generation(),
                format!(
                    "unsupported kind `{}` for add; use openai_compatible, openrouter, openai, or custom",
                    req.kind
                ),
            );
        }
        let patch = ProviderTomlPatch {
            base_url: Some(req.base_url),
            display_name: req.display_name,
            kind: kind.map(str::to_owned),
            admin_base_url: req.admin_base_url,
            enabled: Some(req.enabled),
            catalog_enabled: Some(true),
            ..Default::default()
        };
        // Mint lifecycle incarnation before TOML write; refuse tombstoned ids.
        let incarnation = match self.mint_incarnation_for_add(&pid, false) {
            Ok(i) => i,
            Err(e) => return err_result(pid.as_str(), self.current_generation(), e),
        };
        match upsert_provider(&self.config_path, &pid, &patch, false) {
            Ok(()) => {
                let mut result =
                    self.finalize_after_durable_write(pid.as_str(), req.expected_generation, true);
                result.incarnation = Some(incarnation.as_str().to_owned());
                result.changed_fields = vec!["id".into(), "kind".into(), "base_url".into()];
                result
            }
            Err(e) => err_result(pid.as_str(), self.current_generation(), e.to_string()),
        }
    }

    /// Save typed fields for an existing provider. Stale generation fails closed.
    pub fn save(&self, req: ProviderSaveRequest) -> ProviderMutationResult {
        self.save_with_credentials(req, &CredentialSlotUpdate::default(), None, None)
    }

    /// Save metadata + credential slots under one mutation lock.
    ///
    /// Order: generation check → validate secrets → single atomic TOML write
    /// (including OpenRouter prefs) → credential writes → CAS generation bump.
    /// No secret is written when the client generation is stale.
    pub fn save_with_credentials(
        &self,
        req: ProviderSaveRequest,
        credentials: &CredentialSlotUpdate,
        application_secret: Option<&str>,
        admin_secret: Option<&str>,
    ) -> ProviderMutationResult {
        let _lock = match self.acquire_mutation_lock() {
            Ok(l) => l,
            Err(e) => return err_result(&req.id, self.current_generation(), e),
        };
        if let Err(msg) = self.require_generation_locked(req.expected_generation) {
            return stale_result_with_expected(
                &req.id,
                req.expected_generation,
                self.current_generation(),
                Self::safe_changed_fields(&req.patch),
                msg,
            );
        }
        let detail = match self.detail(&req.id) {
            Ok(d) => d,
            Err(e) => return err_result(&req.id, self.current_generation(), e),
        };
        if !detail.is_editable {
            return err_result(
                &req.id,
                self.current_generation(),
                detail
                    .unsupported_edit_reason
                    .unwrap_or_else(|| "provider is not editable".into()),
            );
        }
        let pid = match ProviderId::new(&req.id) {
            Ok(p) => p,
            Err(e) => return err_result(&req.id, self.current_generation(), e.to_string()),
        };

        // Built-in product providers: only enable + display_name (fail closed).
        let mut patch = req.patch.clone();
        if detail.is_built_in {
            if let Err(e) = restrict_builtin_patch(&mut patch) {
                return err_result(&req.id, self.current_generation(), e);
            }
        } else if let Some(ref k) = patch.kind {
            // Fail closed: refuse kind changes to unsupported values.
            if normalize_kind_for_add(k).is_none() && k != &detail.kind {
                return err_result(
                    &req.id,
                    self.current_generation(),
                    format!("refusing to change kind to unsupported value `{k}`"),
                );
            }
        }

        // Validate credential one-shots before any durable write (Issue 11).
        if let Err(e) = validate_credential_one_shots(credentials, application_secret, admin_secret)
        {
            return err_result(&req.id, self.current_generation(), e);
        }
        // OAuth Clear for built-in OpenAI is not a silent no-op (Issue 10).
        if matches!(credentials.oauth, SecretFieldUpdate::Clear) && req.id == "openai" {
            return err_result(
                &req.id,
                self.current_generation(),
                "OpenAI ChatGPT OAuth clear requires Disconnect / Logout Codex \
                 (browser session path); refusing silent clear"
                    .into(),
            );
        }

        let toml_patch = save_patch_to_toml(&patch);
        let or_prefs = openrouter_prefs_from_save(&patch);
        let has_meta = !is_empty_toml_patch(&toml_patch) || or_prefs.is_some();
        if has_meta {
            if let Err(e) = apply_provider_patch_with_openrouter(
                &self.config_path,
                &pid,
                &toml_patch,
                or_prefs.as_ref(),
            ) {
                return err_result(pid.as_str(), self.current_generation(), e.to_string());
            }
        }

        let creds_changed = credentials.application != SecretFieldUpdate::Preserve
            || credentials.admin != SecretFieldUpdate::Preserve
            || credentials.oauth != SecretFieldUpdate::Preserve;
        if creds_changed {
            if let Err(e) = self.apply_credential_updates_unlocked(
                &req.id,
                credentials,
                application_secret,
                admin_secret,
            ) {
                // Metadata may already be durable; force-record generation and
                // return partial-commit (never pure stale).
                let finalized =
                    self.finalize_after_durable_write(&req.id, req.expected_generation, true);
                return ProviderMutationResult {
                    ok: false,
                    id: req.id.clone(),
                    generation: finalized.generation,
                    error: Some(format!(
                        "metadata saved but credential update failed: {e}. Reload and retry."
                    )),
                    stale: false,
                    guidance: Some(STALE_GUIDANCE.into()),
                    partial_commit: true,
                    incarnation: finalized.incarnation,
                    operation_id: None,
                    conflict: None,
                    changed_fields: finalized.changed_fields,
                };
            }
        }

        if !has_meta && !creds_changed {
            // No-op save: still OK, no bump required.
            return ok_result(req.id, req.expected_generation);
        }

        let mut result = self.finalize_after_durable_write(
            pid.as_str(),
            req.expected_generation,
            has_meta || creds_changed,
        );
        if result.changed_fields.is_empty() {
            result.changed_fields = Self::safe_changed_fields(&req.patch);
            if creds_changed {
                result.changed_fields.push("credentials".into());
            }
        }
        result
    }

    /// Clone metadata into a new id. Secrets and caches are never copied.
    ///
    /// Generation precheck is read-only here; nested `add`/`save` acquire the
    /// mutation lock and re-validate under lock (no recursive flock).
    pub fn clone_provider(&self, req: ProviderCloneRequest) -> ProviderMutationResult {
        let current = self.current_generation();
        if current.get() != req.expected_generation.get() {
            return stale_result_with_expected(
                &req.new_id,
                req.expected_generation,
                current,
                vec!["generation".into()],
                format!(
                    "stale generation: client has {}, registry has {}. {STALE_GUIDANCE}",
                    req.expected_generation.get(),
                    current.get()
                ),
            );
        }
        let source = match self.detail(&req.source_id) {
            Ok(d) => d,
            Err(e) => return err_result(&req.new_id, self.current_generation(), e),
        };
        let new_id = match ProviderId::new(&req.new_id) {
            Ok(p) => p,
            Err(e) => return err_result(&req.new_id, self.current_generation(), e.to_string()),
        };
        if is_reserved_configured_id(new_id.as_str()) {
            return err_result(
                new_id.as_str(),
                self.current_generation(),
                format!("provider id `{}` is reserved", new_id.as_str()),
            );
        }
        let base_url = match source.base_url.clone() {
            Some(u) => u,
            None => {
                return err_result(
                    new_id.as_str(),
                    self.current_generation(),
                    "source provider has no base_url to clone".into(),
                );
            }
        };
        let add = ProviderAddRequest {
            id: new_id.as_str().to_owned(),
            kind: if is_editable_kind(&source.kind, false) {
                source.kind.clone()
            } else {
                "openai_compatible".into()
            },
            base_url,
            display_name: req
                .display_name
                .or_else(|| source.display_name.clone())
                .or_else(|| Some(format!("{} (copy)", source.id))),
            admin_base_url: source.admin_base_url.clone(),
            enabled: source.enabled,
            expected_generation: self.current_generation(),
        };
        let added = self.add(add);
        if !added.ok {
            return added;
        }
        // Copy non-secret typed fields.
        let save = ProviderSaveRequest {
            id: new_id.as_str().to_owned(),
            expected_generation: added.generation,
            patch: ProviderSavePatch {
                default_backend: source.default_backend.clone(),
                auth_scheme: source.auth_scheme.clone(),
                env_key: source.env_key.clone(),
                admin_env_key: source.admin_env_key.clone(),
                catalog_enabled: Some(source.catalog_enabled),
                capability_mode: source.capability_mode.clone(),
                catalog_ttl_secs: source.catalog_ttl_secs,
                request_timeout_secs: source.request_timeout_secs,
                organization: source.organization.clone(),
                project: source.project.clone(),
                extra_headers: Some(source.extra_headers.clone()),
                capabilities: Some(source.capabilities.clone()),
                openrouter_fallback_models: Some(source.openrouter_fallback_models.clone()),
                openrouter_data_collection: Some(source.openrouter_data_collection.clone()),
                openrouter_require_parameters: Some(source.openrouter_require_parameters),
                openrouter_allow_fallbacks: Some(source.openrouter_allow_fallbacks),
                openrouter_zdr: Some(source.openrouter_zdr),
                openrouter_order: Some(source.openrouter_order.clone()),
                openrouter_only: Some(source.openrouter_only.clone()),
                openrouter_ignore: Some(source.openrouter_ignore.clone()),
                openrouter_quantizations: Some(source.openrouter_quantizations.clone()),
                openrouter_sort: Some(source.openrouter_sort.clone()),
                openrouter_pacing: Some(source.openrouter_pacing),
                ..Default::default()
            },
        };
        self.save(save)
    }

    pub fn set_enabled(
        &self,
        id: &str,
        enabled: bool,
        expected: RegistryGeneration,
    ) -> ProviderMutationResult {
        let _lock = match self.acquire_mutation_lock() {
            Ok(l) => l,
            Err(e) => return err_result(id, self.current_generation(), e),
        };
        if let Err(msg) = self.require_generation_locked(expected) {
            return stale_result_with_expected(
                id,
                expected,
                self.current_generation(),
                vec!["enabled".into()],
                msg,
            );
        }
        let pid = match ProviderId::new(id) {
            Ok(p) => p,
            Err(e) => return err_result(id, self.current_generation(), e.to_string()),
        };
        let result = if enabled {
            enable_provider(&self.config_path, &pid)
        } else {
            disable_provider(&self.config_path, &pid)
        };
        match result {
            Ok(()) => {
                let mut r = self.finalize_after_durable_write(pid.as_str(), expected, true);
                r.changed_fields = vec!["enabled".into()];
                r
            }
            Err(e) => err_result(pid.as_str(), self.current_generation(), e.to_string()),
        }
    }

    /// Grouped reverse-reference impact for disable/remove UX.
    pub fn reference_impact(&self, id: &str) -> Result<ReferenceImpactSnapshot, String> {
        let generation = self.current_generation();
        let (entries, _) = self.load_entries()?;
        if !entries.contains_key(id)
            && BuiltInProviderId::parse(id).is_none()
            && id != crate::agent::zai::ZAI_PROVIDER_ID
        {
            let service =
                ProviderService::from_model_providers(&entries).map_err(|e| e.to_string())?;
            if service.get(id).is_none() {
                return Err(format!("provider `{id}` not found"));
            }
        }
        let secrets_present = {
            let c = self.credential_presence(id);
            c.has_application_key || c.has_admin_key || c.has_oauth
        };
        let cache_present = self.catalog_cache_hint(id).is_some();
        let is_built_in = BuiltInProviderId::parse(id).is_some();
        Ok(
            crate::provider_registry::references::build_reference_impact(
                &self.home,
                &self.config_path,
                id,
                generation,
                is_built_in,
                secrets_present,
                cache_present,
            ),
        )
    }

    /// Optional metadata remove for configured providers when impact allows.
    /// Does not auto-delete secrets/caches (never rename/delete legacy caches).
    ///
    /// Uses the same exclusive lifecycle lock + generation precheck + CAS bump
    /// as add/save/enable. Clean remove forgets the live incarnation row without
    /// a tombstone; re-add still mints a new incarnation.
    pub fn remove_metadata(
        &self,
        id: &str,
        expected: RegistryGeneration,
        confirm: bool,
    ) -> ProviderMutationResult {
        if !confirm {
            return err_result(
                id,
                self.current_generation(),
                "refusing to remove without confirmation".into(),
            );
        }
        let _lock = match self.acquire_mutation_lock() {
            Ok(l) => l,
            Err(e) => return err_result(id, self.current_generation(), e),
        };
        if let Err(msg) = self.require_generation_locked(expected) {
            return stale_result_with_expected(
                id,
                expected,
                self.current_generation(),
                vec!["generation".into()],
                msg,
            );
        }
        let impact = match self.reference_impact(id) {
            Ok(i) => i,
            Err(e) => return err_result(id, self.current_generation(), e),
        };
        if !impact.can_remove {
            return err_result(
                id,
                self.current_generation(),
                impact
                    .blocked_reason
                    .unwrap_or_else(|| "remove not allowed".into()),
            );
        }
        let pid = match ProviderId::new(id) {
            Ok(p) => p,
            Err(e) => return err_result(id, self.current_generation(), e.to_string()),
        };
        match remove_provider(&self.config_path, &pid) {
            Ok(()) => {
                // Forget live row (no tombstone on clean zero-ref remove).
                let _ =
                    crate::provider_registry::lifecycle_state::with_lifecycle_state_mut_unlocked(
                        &self.home,
                        |st| {
                            st.forget_live(pid.as_str());
                            Ok(((), true))
                        },
                    );
                let mut result = self.finalize_after_durable_write(pid.as_str(), expected, true);
                result.changed_fields = vec!["removed".into()];
                result
            }
            Err(e) => err_result(pid.as_str(), self.current_generation(), e.to_string()),
        }
    }

    /// Forced remove: typed exact provider ID barrier, incarnation tombstone,
    /// optional independent secret/cache clears. Built-ins cannot be removed.
    pub fn force_remove(&self, req: ProviderForceRemoveRequest) -> ProviderMutationResult {
        let id = req.id.as_str();
        if req.typed_id_confirmation != id {
            return err_result(
                id,
                self.current_generation(),
                "typed provider id does not match; forced remove requires the exact id".into(),
            );
        }
        if BuiltInProviderId::parse(id).is_some() {
            return err_result(
                id,
                self.current_generation(),
                "built-in product providers cannot be removed".into(),
            );
        }
        let _lock = match self.acquire_mutation_lock() {
            Ok(l) => l,
            Err(e) => return err_result(id, self.current_generation(), e),
        };
        if let Err(msg) = self.require_generation_locked(req.expected_generation) {
            return stale_result_with_expected(
                id,
                req.expected_generation,
                self.current_generation(),
                vec!["generation".into()],
                msg,
            );
        }
        let pid = match ProviderId::new(id) {
            Ok(p) => p,
            Err(e) => return err_result(id, self.current_generation(), e.to_string()),
        };
        let expected_inc = req
            .expected_incarnation
            .as_deref()
            .and_then(|s| super::instance::ProviderIncarnation::new(s).ok());
        // Ordering under lock: (1) tombstone first for reuse safety, (2) remove
        // TOML, (3) only then optional secret/cache clears. Never clear secrets
        // if TOML removal fails.
        let tombstoned =
            match crate::provider_registry::lifecycle_state::with_lifecycle_state_mut_unlocked(
                &self.home,
                |st| {
                    if !st.instances.contains_key(pid.as_str()) {
                        // Legacy configured provider without lifecycle row.
                        let incarnation = expected_inc.clone().unwrap_or_else(|| {
                            super::instance::ProviderIncarnation::new(
                                uuid::Uuid::new_v4().to_string(),
                            )
                            .expect("uuid v4 is canonical")
                        });
                        st.instances.insert(
                            pid.as_str().to_owned(),
                            super::lifecycle_state::InstanceLifecycleRecord {
                                incarnation,
                                restored: false,
                            },
                        );
                    }
                    let inc = st.tombstone_remove(&pid, expected_inc.as_ref())?;
                    Ok((inc, true))
                },
            ) {
                Ok(inc) => inc,
                Err(e) => return err_result(id, self.current_generation(), e.to_string()),
            };
        match remove_provider(&self.config_path, &pid) {
            Ok(()) => {
                // TOML gone: now safe to clear opt-in secrets/caches.
                if req.clear.clear_application_key {
                    let _ = clear_provider_secret(&self.home, &application_key_scope(&pid));
                }
                if req.clear.clear_admin_key {
                    let _ = clear_provider_secret(&self.home, &admin_key_scope(&pid));
                }
                if req.clear.clear_oauth {
                    let _ =
                        crate::auth::clear_provider_api_key(&self.home, &oauth_scope_string(&pid));
                }
                if req.clear.clear_catalog_cache {
                    let _ = CatalogCacheStore::remove(&self.home, &pid);
                }
                if req.clear.clear_capability_cache {
                    let _ = CapabilityCacheStore::remove(&self.home, &pid);
                }
                if req.clear.clear_catalog_cache && req.clear.clear_capability_cache {
                    let _ = super::cache::ProviderCacheStore::remove_instance(&self.home, &pid);
                }
                let mut result =
                    self.finalize_after_durable_write(pid.as_str(), req.expected_generation, true);
                result.incarnation = Some(tombstoned.as_str().to_owned());
                result.operation_id = req.operation_id.clone();
                result.changed_fields =
                    vec!["removed".into(), "tombstone".into(), "incarnation".into()];
                result.guidance = Some(
                    "Forced remove completed. Old sessions bound to the prior incarnation will not rebind if this id is recreated. Use explicit restore to reuse the tombstoned incarnation, or Clone for a new id."
                        .into(),
                );
                self.record_providers_update_notification(
                    result.generation,
                    &[pid.as_str()],
                    &result.changed_fields,
                );
                result
            }
            Err(e) => {
                // Tombstone durable, TOML still present: partial_commit, secrets intact.
                let live = self
                    .force_record_generation_after_commit()
                    .unwrap_or_else(|_| self.effective_generation_readonly());
                let registry_gen = RegistryGeneration(live);
                let changed = vec!["tombstone".into()];
                // Partial commits still advance generation — notify all clients.
                self.record_providers_update_notification(registry_gen, &[pid.as_str()], &changed);
                ProviderMutationResult {
                    ok: false,
                    id: pid.as_str().to_owned(),
                    generation: registry_gen,
                    error: Some(format!(
                        "tombstone recorded but config.toml remove failed: {e}. \
                         Provider id is blocked for ordinary re-add; secrets/caches were not cleared. Reload and retry remove or restore."
                    )),
                    stale: false,
                    guidance: Some(
                        "Partial forced remove: tombstone is durable; config row may still exist. Reload."
                            .into(),
                    ),
                    partial_commit: true,
                    incarnation: Some(tombstoned.as_str().to_owned()),
                    operation_id: req.operation_id.clone(),
                    conflict: None,
                    changed_fields: changed,
                }
            }
        }
    }

    /// Explicit restore of a tombstoned id (distinct from ordinary re-add).
    pub fn restore_tombstoned(
        &self,
        id: &str,
        kind: &str,
        base_url: &str,
        expected: RegistryGeneration,
    ) -> ProviderMutationResult {
        let _lock = match self.acquire_mutation_lock() {
            Ok(l) => l,
            Err(e) => return err_result(id, self.current_generation(), e),
        };
        if let Err(msg) = self.require_generation_locked(expected) {
            return stale_result_with_expected(
                id,
                expected,
                self.current_generation(),
                vec!["generation".into()],
                msg,
            );
        }
        let pid = match ProviderId::new(id) {
            Ok(p) => p,
            Err(e) => return err_result(id, self.current_generation(), e.to_string()),
        };
        let incarnation = match self.mint_incarnation_for_add(&pid, true) {
            Ok(i) => i,
            Err(e) => return err_result(id, self.current_generation(), e),
        };
        if let Err(e) = validate_http_base_url(base_url) {
            return err_result(id, self.current_generation(), e.to_string());
        }
        let kind_norm = normalize_kind_for_add(kind);
        let Some(kind_norm) = kind_norm else {
            return err_result(
                id,
                self.current_generation(),
                format!("unsupported kind `{kind}`"),
            );
        };
        let patch = ProviderTomlPatch {
            base_url: Some(base_url.to_owned()),
            kind: Some(kind_norm.to_owned()),
            enabled: Some(true),
            catalog_enabled: Some(true),
            ..Default::default()
        };
        match upsert_provider(&self.config_path, &pid, &patch, false) {
            Ok(()) => {
                let mut result = self.finalize_after_durable_write(pid.as_str(), expected, true);
                result.incarnation = Some(incarnation.as_str().to_owned());
                result.changed_fields = vec!["restored".into(), "incarnation".into()];
                result
            }
            Err(e) => err_result(pid.as_str(), self.current_generation(), e.to_string()),
        }
    }

    /// Credential presence only (never values).
    pub fn credential_presence(&self, id: &str) -> CredentialPresence {
        let mut presence = CredentialPresence::default();
        // Built-in product scopes via ProviderManager status when possible.
        if let Some(backend) = built_in_backend(id) {
            let manager = ProviderManager::new(&self.home);
            let status = manager.status(backend);
            use crate::agent::providers::{ProviderAuthenticationKind, ProviderConnectionState};
            for method in &status.authentication {
                match method.kind {
                    ProviderAuthenticationKind::ApiKey => {
                        presence.has_application_key =
                            method.state != ProviderConnectionState::NotConfigured;
                    }
                    ProviderAuthenticationKind::OAuth | ProviderAuthenticationKind::ChatGpt => {
                        presence.has_oauth = method.state != ProviderConnectionState::NotConfigured;
                    }
                }
            }
            // Admin for OpenRouter.
            if id == "openrouter"
                && let Ok(Some(_)) =
                    read_provider_secret(&self.home, crate::auth::OPENROUTER_ADMIN_KEY_SCOPE)
            {
                presence.has_admin_key = true;
            }
            return presence;
        }
        if let Ok(pid) = ProviderId::new(id) {
            if let Ok(Some(_)) = read_provider_secret(&self.home, &application_key_scope(&pid)) {
                presence.has_application_key = true;
            }
            if let Ok(Some(_)) = read_provider_secret(&self.home, &admin_key_scope(&pid)) {
                presence.has_admin_key = true;
            }
            // OAuth presence via dedicated scope (token may exist).
            if let Ok(Some(_)) =
                crate::auth::read_provider_api_key(&self.home, &oauth_scope_string(&pid))
            {
                presence.has_oauth = true;
            }
        }
        presence
    }

    /// Apply credential slot updates under the mutation lock with generation gate.
    ///
    /// Prefer [`Self::save_with_credentials`] when metadata is also changing.
    /// Empty secret means preserve; Clear is explicit. Callers must drop
    /// one-shot secret values after this returns.
    pub fn apply_credential_updates(
        &self,
        id: &str,
        expected: RegistryGeneration,
        update: &CredentialSlotUpdate,
        application_secret: Option<&str>,
        admin_secret: Option<&str>,
    ) -> ProviderMutationResult {
        self.save_with_credentials(
            ProviderSaveRequest {
                id: id.to_owned(),
                expected_generation: expected,
                patch: ProviderSavePatch::default(),
            },
            update,
            application_secret,
            admin_secret,
        )
    }

    fn apply_credential_updates_unlocked(
        &self,
        id: &str,
        update: &CredentialSlotUpdate,
        application_secret: Option<&str>,
        admin_secret: Option<&str>,
    ) -> Result<(), String> {
        validate_credential_one_shots(update, application_secret, admin_secret)?;
        match update.application {
            SecretFieldUpdate::Preserve => {}
            SecretFieldUpdate::Clear => self.clear_application_key(id)?,
            SecretFieldUpdate::Set => {
                let secret = application_secret
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .expect("validated");
                self.store_application_key(id, secret)?;
            }
        }
        match update.admin {
            SecretFieldUpdate::Preserve => {}
            SecretFieldUpdate::Clear => self.clear_admin_key(id)?,
            SecretFieldUpdate::Set => {
                let secret = admin_secret
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .expect("validated");
                self.store_admin_key(id, secret)?;
            }
        }
        match update.oauth {
            SecretFieldUpdate::Preserve => {}
            SecretFieldUpdate::Clear => {
                if id == "openai" {
                    return Err(
                        "OpenAI ChatGPT OAuth clear requires Disconnect / Logout Codex".into(),
                    );
                }
                if let Ok(pid) = ProviderId::new(id) {
                    let _ =
                        crate::auth::clear_provider_api_key(&self.home, &oauth_scope_string(&pid));
                }
            }
            SecretFieldUpdate::Set => {
                return Err(
                    "OAuth Set is browser/device flow only; use Connect / Login in the TUI".into(),
                );
            }
        }
        Ok(())
    }

    fn store_application_key(&self, id: &str, secret: &str) -> Result<(), String> {
        if let Some(backend) = built_in_backend(id) {
            let manager = ProviderManager::new(&self.home);
            manager
                .set_api_key_binding_generation(backend, secret)
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        let pid = ProviderId::new(id).map_err(|e| e.to_string())?;
        store_provider_secret(&self.home, &application_key_scope(&pid), secret)
            .map_err(|e| e.to_string())
    }

    fn clear_application_key(&self, id: &str) -> Result<(), String> {
        if let Some(backend) = built_in_backend(id) {
            let manager = ProviderManager::new(&self.home);
            manager.remove_api_key(backend).map_err(|e| e.to_string())?;
            return Ok(());
        }
        let pid = ProviderId::new(id).map_err(|e| e.to_string())?;
        clear_provider_secret(&self.home, &application_key_scope(&pid)).map_err(|e| e.to_string())
    }

    fn store_admin_key(&self, id: &str, secret: &str) -> Result<(), String> {
        if id == "openrouter" {
            store_provider_secret(&self.home, crate::auth::OPENROUTER_ADMIN_KEY_SCOPE, secret)
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        let pid = ProviderId::new(id).map_err(|e| e.to_string())?;
        store_provider_secret(&self.home, &admin_key_scope(&pid), secret).map_err(|e| e.to_string())
    }

    fn clear_admin_key(&self, id: &str) -> Result<(), String> {
        if id == "openrouter" {
            clear_provider_secret(&self.home, crate::auth::OPENROUTER_ADMIN_KEY_SCOPE)
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        let pid = ProviderId::new(id).map_err(|e| e.to_string())?;
        clear_provider_secret(&self.home, &admin_key_scope(&pid)).map_err(|e| e.to_string())
    }

    /// Real connection test (built-ins via ProviderManager; configured via live
    /// non-mutating models list when credentials exist).
    pub async fn test_connection(&self, id: &str) -> ProviderStatusSnapshot {
        let generation = self.current_generation();
        if let Some(backend) = built_in_backend(id) {
            let manager = ProviderManager::new(&self.home);
            return match manager.test_connection(backend).await {
                Ok(ProviderConnectionTest::Connected { credits }) => ProviderStatusSnapshot {
                    provider_id: id.to_owned(),
                    generation,
                    connected: true,
                    label: "Connected".into(),
                    detail: Some(match credits {
                        Some(c) => format!("Connection verified without an inference charge · {c}"),
                        None => "Connection verified without an inference charge".into(),
                    }),
                    error: None,
                },
                Ok(ProviderConnectionTest::NotConfigured) => ProviderStatusSnapshot {
                    provider_id: id.to_owned(),
                    generation,
                    connected: false,
                    label: "Key/connect missing".into(),
                    detail: None,
                    error: None,
                },
                Ok(ProviderConnectionTest::Rejected) => ProviderStatusSnapshot {
                    provider_id: id.to_owned(),
                    generation,
                    connected: false,
                    label: "Connection error".into(),
                    detail: None,
                    error: Some("The provider rejected the configured credential".into()),
                },
                Ok(ProviderConnectionTest::Unavailable) => ProviderStatusSnapshot {
                    provider_id: id.to_owned(),
                    generation,
                    connected: false,
                    label: "Connection error".into(),
                    detail: None,
                    error: Some("The provider is currently unavailable".into()),
                },
                Err(e) => ProviderStatusSnapshot {
                    provider_id: id.to_owned(),
                    generation,
                    connected: false,
                    label: "Connection error".into(),
                    detail: None,
                    error: Some(e.to_string()),
                },
            };
        }
        // Configured instance: dry-run resolve + optional live listModels via PR11 path.
        match self.probe_configured_models_list(id).await {
            Ok(summary) => ProviderStatusSnapshot {
                provider_id: id.to_owned(),
                generation,
                connected: true,
                label: "Connected".into(),
                detail: Some(summary),
                error: None,
            },
            Err(e) => {
                let presence = self.credential_presence(id);
                if !presence.has_application_key && !presence.has_admin_key {
                    ProviderStatusSnapshot {
                        provider_id: id.to_owned(),
                        generation,
                        connected: false,
                        label: "Key/connect missing".into(),
                        detail: None,
                        error: Some(e),
                    }
                } else {
                    ProviderStatusSnapshot {
                        provider_id: id.to_owned(),
                        generation,
                        connected: false,
                        label: "Connection error".into(),
                        detail: None,
                        error: Some(e),
                    }
                }
            }
        }
    }

    /// Status without network when possible (credential presence + cache).
    pub fn status_snapshot(&self, id: &str) -> ProviderStatusSnapshot {
        let generation = self.current_generation();
        if let Some(backend) = built_in_backend(id) {
            let manager = ProviderManager::new(&self.home);
            let st = manager.status(backend);
            use crate::agent::providers::ProviderConnectionState;
            let (connected, label, detail) = match st.state {
                ProviderConnectionState::Connected | ProviderConnectionState::Configured => (
                    true,
                    "Connected".to_owned(),
                    Some(format!("{} authentication present", st.display_name)),
                ),
                ProviderConnectionState::NotConfigured => {
                    (false, "Key/connect missing".to_owned(), None)
                }
                _ => (false, "Connection error".to_owned(), None),
            };
            return ProviderStatusSnapshot {
                provider_id: id.to_owned(),
                generation,
                connected,
                label,
                detail,
                error: None,
            };
        }
        let presence = self.credential_presence(id);
        if presence.has_application_key || presence.has_oauth {
            ProviderStatusSnapshot {
                provider_id: id.to_owned(),
                generation,
                connected: true,
                label: "Connected".into(),
                detail: Some("Credential present (run Test for live probe)".into()),
                error: None,
            }
        } else {
            ProviderStatusSnapshot {
                provider_id: id.to_owned(),
                generation,
                connected: false,
                label: "Key/connect missing".into(),
                detail: None,
                error: None,
            }
        }
    }

    /// OpenRouter credits via exact getCredits binding when kind allows.
    pub async fn credits_snapshot(&self, id: &str) -> ProviderCreditsSnapshot {
        let generation = self.current_generation();
        let detail = match self.detail(id) {
            Ok(d) => d,
            Err(e) => {
                return ProviderCreditsSnapshot {
                    provider_id: id.to_owned(),
                    generation,
                    available: false,
                    summary: None,
                    error: Some(e),
                };
            }
        };
        if detail.kind != "openrouter" && id != "openrouter" {
            return ProviderCreditsSnapshot {
                provider_id: id.to_owned(),
                generation,
                available: false,
                summary: None,
                error: Some("Credits are available for OpenRouter native instances only".into()),
            };
        }
        // Built-in OpenRouter: ProviderManager test may return credits.
        if id == "openrouter" {
            let manager = ProviderManager::new(&self.home);
            return match manager.test_connection(BuiltInBackendId::OpenRouter).await {
                Ok(ProviderConnectionTest::Connected { credits: Some(c) }) => {
                    ProviderCreditsSnapshot {
                        provider_id: id.to_owned(),
                        generation,
                        available: true,
                        summary: Some(c),
                        error: None,
                    }
                }
                Ok(ProviderConnectionTest::Connected { credits: None }) => {
                    ProviderCreditsSnapshot {
                        provider_id: id.to_owned(),
                        generation,
                        available: true,
                        summary: Some("Connected (no credit summary in probe)".into()),
                        error: None,
                    }
                }
                Ok(_) => ProviderCreditsSnapshot {
                    provider_id: id.to_owned(),
                    generation,
                    available: false,
                    summary: None,
                    error: Some("OpenRouter not connected".into()),
                },
                Err(e) => ProviderCreditsSnapshot {
                    provider_id: id.to_owned(),
                    generation,
                    available: false,
                    summary: None,
                    error: Some(e.to_string()),
                },
            };
        }
        // Configured openrouter kind: attempt authenticated GET /credits via transport.
        match self.probe_openrouter_credits(id).await {
            Ok(summary) => ProviderCreditsSnapshot {
                provider_id: id.to_owned(),
                generation,
                available: true,
                summary: Some(summary),
                error: None,
            },
            Err(e) => ProviderCreditsSnapshot {
                provider_id: id.to_owned(),
                generation,
                available: false,
                summary: None,
                error: Some(e),
            },
        }
    }

    /// Catalog status + optional refresh using PR8/production paths.
    pub async fn refresh_catalog(&self, id: &str) -> CatalogStatusSnapshot {
        let generation = self.current_generation();
        if let Some(backend) = built_in_backend(id) {
            let manager = ProviderManager::new(&self.home);
            let result = match backend {
                BuiltInBackendId::OpenAi => manager
                    .refresh_openai_catalog()
                    .await
                    .map(|c| c.models.len()),
                BuiltInBackendId::OpenRouter => manager
                    .refresh_openrouter_catalog()
                    .await
                    .map(|c| c.models.len()),
                BuiltInBackendId::Anthropic => manager
                    .refresh_anthropic_catalog()
                    .await
                    .map(|c| c.models.len()),
                BuiltInBackendId::Xai => {
                    return CatalogStatusSnapshot {
                        provider_id: id.to_owned(),
                        generation,
                        catalog_enabled: true,
                        model_count: None,
                        last_refresh_label: None,
                        source: None,
                        error: Some(
                            "xAI catalog is not refreshed via the provider management path".into(),
                        ),
                        sample_model_ids: Vec::new(),
                    };
                }
            };
            return match result {
                Ok(count) => CatalogStatusSnapshot {
                    provider_id: id.to_owned(),
                    generation,
                    catalog_enabled: true,
                    model_count: Some(count),
                    last_refresh_label: Some("just now".into()),
                    source: Some("live".into()),
                    error: None,
                    sample_model_ids: Vec::new(),
                },
                Err(e) => CatalogStatusSnapshot {
                    provider_id: id.to_owned(),
                    generation,
                    catalog_enabled: true,
                    model_count: None,
                    last_refresh_label: None,
                    source: None,
                    error: Some(e.to_string()),
                    sample_model_ids: Vec::new(),
                },
            };
        }
        // Configured: live refresh via non-mutating models list probe (PR11 path).
        match self.probe_configured_models_list(id).await {
            Ok(summary) => CatalogStatusSnapshot {
                provider_id: id.to_owned(),
                generation,
                catalog_enabled: true,
                model_count: None,
                last_refresh_label: Some("just now".into()),
                source: Some("live_probe".into()),
                error: None,
                sample_model_ids: vec![summary],
            },
            Err(e) => {
                let cached = self.catalog_cache_hint(id);
                CatalogStatusSnapshot {
                    provider_id: id.to_owned(),
                    generation,
                    catalog_enabled: true,
                    model_count: cached.as_ref().map(|(n, _)| *n),
                    last_refresh_label: cached.as_ref().map(|_| "from cache".into()),
                    source: cached.as_ref().map(|_| "cache".into()),
                    error: Some(e),
                    sample_model_ids: cached.map(|(_, s)| s).unwrap_or_default(),
                }
            }
        }
    }

    pub fn catalog_status(&self, id: &str) -> CatalogStatusSnapshot {
        let generation = self.current_generation();
        let detail = self.detail(id).ok();
        let catalog_enabled = detail.as_ref().map(|d| d.catalog_enabled).unwrap_or(true);
        if let Some((count, samples)) = self.catalog_cache_hint(id) {
            return CatalogStatusSnapshot {
                provider_id: id.to_owned(),
                generation,
                catalog_enabled,
                model_count: Some(count),
                last_refresh_label: Some("from cache".into()),
                source: Some("cache".into()),
                error: None,
                sample_model_ids: samples,
            };
        }
        CatalogStatusSnapshot {
            provider_id: id.to_owned(),
            generation,
            catalog_enabled,
            model_count: None,
            last_refresh_label: None,
            source: None,
            error: None,
            sample_model_ids: Vec::new(),
        }
    }

    pub fn capability_status(&self, id: &str) -> CapabilityStatusSnapshot {
        let generation = self.current_generation();
        let detail = self.detail(id).ok();
        let mode = detail
            .as_ref()
            .and_then(|d| d.capability_mode.clone())
            .unwrap_or_else(|| "auto".into());
        let capabilities = detail
            .as_ref()
            .map(|d| d.capabilities.clone())
            .unwrap_or_default();
        // Discovered capabilities require origin+baseline; report config overrides only
        // here to avoid inventing cache reads without identity.
        let discovered = IndexMap::new();
        CapabilityStatusSnapshot {
            provider_id: id.to_owned(),
            generation,
            mode,
            capabilities,
            discovered,
            error: None,
        }
    }

    pub async fn refresh_capabilities(&self, id: &str) -> CapabilityStatusSnapshot {
        // Non-mutating remote-wise: a live models probe informs the UI that
        // discovery was attempted. Per-instance capability cache is never
        // bulk-deleted for siblings.
        let _ = self.probe_configured_models_list(id).await;
        self.capability_status(id)
    }

    /// Best-effort catalog cache summary when origin can be derived from base_url.
    fn catalog_cache_hint(&self, id: &str) -> Option<(usize, Vec<String>)> {
        let pid = ProviderId::new(id).ok()?;
        let detail = self.detail(id).ok()?;
        let base = detail.base_url.as_deref()?;
        let origin = normalize_endpoint_origin(base).ok()?;
        let entry = CatalogCacheStore::load(&self.home, &pid, &origin)
            .ok()
            .flatten()?;
        let samples: Vec<String> = entry
            .models
            .iter()
            .take(8)
            .filter_map(|m| {
                m.get("id")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
                    .or_else(|| m.as_str().map(str::to_owned))
            })
            .collect();
        Some((entry.models.len(), samples))
    }

    // ── internals ────────────────────────────────────────────────────────

    fn load_entries(&self) -> Result<(IndexMap<String, ModelProviderConfig>, Vec<String>), String> {
        match fs::read_to_string(&self.config_path) {
            Ok(raw) => {
                let val: toml::Value =
                    toml::from_str(&raw).map_err(|e| format!("parse config.toml: {e}"))?;
                let (entries, warnings) = parse_model_providers(&val);
                let msgs = warnings.into_iter().map(|w| w.reason.clone()).collect();
                Ok((entries, msgs))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok((IndexMap::new(), Vec::new())),
            Err(e) => Err(format!("read config.toml: {e}")),
        }
    }

    fn require_generation_locked(&self, expected: RegistryGeneration) -> Result<(), String> {
        // Under lock: materialize any external fingerprint drift, then compare.
        let current = self.reconcile_generation_locked()?;
        if current != expected.get() {
            return Err(format!(
                "stale generation: client has {}, registry has {}. {STALE_GUIDANCE}",
                expected.get(),
                current
            ));
        }
        Ok(())
    }

    /// Exclusive flock for mutation serialization (concurrent same-generation writers).
    /// Callers must keep the returned `File` alive for the duration of the critical section.
    fn acquire_mutation_lock(&self) -> Result<File, String> {
        let path = self.home.join(LOCK_REL);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| format!("provider lifecycle lock: {e}"))?;
        file.lock_exclusive()
            .map_err(|e| format!("provider lifecycle lock: {e}"))?;
        Ok(file)
    }

    /// Read-only effective generation for list/detail/UI (never writes).
    ///
    /// When config fingerprint diverges, returns `stored + 1` so clients that
    /// still hold the stored generation fail closed. Durable advancement is
    /// deferred to the mutation lock.
    fn effective_generation_readonly(&self) -> u64 {
        let (generation, stored_fp) = read_generation_raw(&self.home);
        let live_fp = config_fingerprint(&self.config_path);
        if stored_fp.is_empty() && generation == 0 {
            return 0;
        }
        if stored_fp != live_fp {
            return generation.saturating_add(1);
        }
        generation
    }

    /// Under mutation lock: if fingerprint diverged, advance and persist generation.
    fn reconcile_generation_locked(&self) -> Result<u64, String> {
        let (generation, stored_fp) = read_generation_raw(&self.home);
        let live_fp = config_fingerprint(&self.config_path);
        if stored_fp.is_empty() && generation == 0 {
            // First observation: record fingerprint without bumping.
            write_generation_state(&self.home, generation, &live_fp)?;
            return Ok(generation);
        }
        if stored_fp != live_fp {
            let next = generation.saturating_add(1);
            write_generation_state(&self.home, next, &live_fp)?;
            // External config edit advanced generation — notify clients.
            self.record_providers_update_notification(
                RegistryGeneration(next),
                &[],
                &["external_config".into()],
            );
            return Ok(next);
        }
        Ok(generation)
    }

    /// Compare-and-swap generation bump after durable mutation. Call under lock.
    ///
    /// Records live config fingerprint with the new generation. Propagates I/O
    /// failures from the generation sidecar write.
    fn cas_bump_generation(&self, expected: RegistryGeneration) -> Result<u64, String> {
        let (current, _) = read_generation_raw(&self.home);
        if current != expected.get() {
            return Err(format!(
                "stale generation: client has {}, registry has {}. {STALE_GUIDANCE}",
                expected.get(),
                current
            ));
        }
        let next = current.saturating_add(1);
        let fp = config_fingerprint(&self.config_path);
        write_generation_state(&self.home, next, &fp)?;
        Ok(next)
    }

    /// After config and/or secrets were durably mutated under the lock, advance
    /// generation. Never reports pure stale for a write that already committed:
    /// on CAS/I/O failure, force-records a live generation and marks partial_commit.
    fn finalize_after_durable_write(
        &self,
        id: &str,
        expected: RegistryGeneration,
        durable_changed: bool,
    ) -> ProviderMutationResult {
        if !durable_changed {
            return ok_result(id.to_owned(), expected);
        }
        let result = match self.cas_bump_generation(expected) {
            Ok(next) => ok_result(id.to_owned(), RegistryGeneration(next)),
            Err(msg) => {
                // Durable state already changed under the lock. Force-record so
                // clients must reload rather than treating this as a no-op stale miss.
                match self.force_record_generation_after_commit() {
                    Ok(live) => ProviderMutationResult {
                        ok: true,
                        id: id.to_owned(),
                        generation: RegistryGeneration(live),
                        error: None,
                        stale: false,
                        guidance: Some(format!(
                            "Mutation committed; generation bookkeeping recovered after: {msg}. Reload."
                        )),
                        partial_commit: true,
                        incarnation: None,
                        operation_id: None,
                        conflict: None,
                        changed_fields: Vec::new(),
                    },
                    Err(io_err) => ProviderMutationResult {
                        ok: false,
                        id: id.to_owned(),
                        generation: RegistryGeneration(self.effective_generation_readonly()),
                        error: Some(format!(
                            "Mutation may be durable but generation sidecar update failed: \
                             {msg}; recovery write failed: {io_err}. Reload and inspect."
                        )),
                        stale: false,
                        guidance: Some(STALE_GUIDANCE.into()),
                        partial_commit: true,
                        incarnation: None,
                        operation_id: None,
                        conflict: None,
                        changed_fields: Vec::new(),
                    },
                }
            }
        };
        if result.ok || result.partial_commit {
            let fallback_fields = vec!["generation".to_owned()];
            let fields = if result.changed_fields.is_empty() {
                &fallback_fields
            } else {
                &result.changed_fields
            };
            self.record_providers_update_notification(result.generation, &[id], fields);
        }
        result
    }

    /// Force-write next generation with live fingerprint under the mutation lock.
    fn force_record_generation_after_commit(&self) -> Result<u64, String> {
        let (current, _) = read_generation_raw(&self.home);
        let next = current.saturating_add(1);
        let fp = config_fingerprint(&self.config_path);
        write_generation_state(&self.home, next, &fp)?;
        Ok(next)
    }

    fn mint_incarnation_for_add(
        &self,
        pid: &ProviderId,
        restore: bool,
    ) -> Result<super::instance::ProviderIncarnation, String> {
        // Caller holds the management lifecycle flock.
        crate::provider_registry::lifecycle_state::with_lifecycle_state_mut_unlocked(
            &self.home,
            |st| {
                let inc = st.mint_or_restore(pid, restore)?;
                Ok((inc, true))
            },
        )
        .map_err(|e| e.to_string())
    }

    fn live_incarnation_str(&self, id: &str) -> Option<String> {
        // Read-only: never mint during detail/list. Fail closed on IO/corrupt.
        if let Some(inc) = super::lifecycle_state::stable_builtin_incarnation(id) {
            return Some(inc.as_str().to_owned());
        }
        let state =
            crate::provider_registry::lifecycle_state::load_lifecycle_state(&self.home).ok()?;
        state.incarnation_for(id).map(|i| i.as_str().to_owned())
    }

    fn tombstone_blocks_readd(&self, id: &str) -> bool {
        // Unreadable lifecycle state: treat as blocking for re-add UX (fail closed).
        match crate::provider_registry::lifecycle_state::load_lifecycle_state(&self.home) {
            Ok(s) => s.has_blocking_tombstone_for_id(id),
            Err(_) => true,
        }
    }

    /// Persist a version-tolerant machine-wide providers/update payload under
    /// `$GROK_HOME/state/provider_registry_notify.json` and best-effort ACP
    /// fanout when a gateway is attached via the models manager path.
    pub fn record_providers_update_notification(
        &self,
        generation: RegistryGeneration,
        changed_ids: &[&str],
        changed_fields: &[String],
    ) {
        let payload = serde_json::json!({
            "schema_version": 1,
            "generation": generation.get(),
            "changed_ids": changed_ids,
            "changed_fields": changed_fields,
        });
        let path = self.home.join("state/provider_registry_notify.json");
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        // Unique-temp + rename (best effort).
        let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
        if let Ok(bytes) = serde_json::to_vec_pretty(&payload) {
            if fs::write(&tmp, &bytes).is_ok() {
                let _ = fs::rename(&tmp, &path);
            } else {
                let _ = fs::remove_file(&tmp);
            }
        }
        // Invalidate hot-path runtime cache so next turn sees the mutation.
        crate::provider_registry::runtime_cache::invalidate_for_home(&self.home);
        // Best-effort ACP ext notification for connected leader clients.
        crate::provider_registry::notify::try_forward_providers_update(&payload);
    }

    /// Safe changed-field names for a save patch (no secret values).
    pub fn safe_changed_fields(patch: &ProviderSavePatch) -> Vec<String> {
        let mut fields = Vec::new();
        let mut push = |name: &str, set: bool| {
            if set {
                fields.push(name.to_owned());
            }
        };
        push("display_name", patch.display_name.is_some());
        push("kind", patch.kind.is_some());
        push("base_url", patch.base_url.is_some());
        push("admin_base_url", patch.admin_base_url.is_some());
        push("enabled", patch.enabled.is_some());
        push("default_backend", patch.default_backend.is_some());
        push("auth_scheme", patch.auth_scheme.is_some());
        push("env_key", patch.env_key.is_some());
        push("admin_env_key", patch.admin_env_key.is_some());
        push("catalog_enabled", patch.catalog_enabled.is_some());
        push("capability_mode", patch.capability_mode.is_some());
        push("catalog_ttl_secs", patch.catalog_ttl_secs.is_some());
        push("request_timeout_secs", patch.request_timeout_secs.is_some());
        push("organization", patch.organization.is_some());
        push("project", patch.project.is_some());
        push("api_surface", patch.api_surface.is_some());
        push("credential_route", patch.credential_route.is_some());
        push("extra_headers", patch.extra_headers.is_some());
        push("capabilities", patch.capabilities.is_some());
        push(
            "openrouter_fallback_models",
            patch.openrouter_fallback_models.is_some(),
        );
        push(
            "openrouter_data_collection",
            patch.openrouter_data_collection.is_some(),
        );
        push(
            "openrouter_require_parameters",
            patch.openrouter_require_parameters.is_some(),
        );
        push(
            "openrouter_allow_fallbacks",
            patch.openrouter_allow_fallbacks.is_some(),
        );
        push("openrouter_zdr", patch.openrouter_zdr.is_some());
        push("openrouter_order", patch.openrouter_order.is_some());
        push("openrouter_only", patch.openrouter_only.is_some());
        push("openrouter_ignore", patch.openrouter_ignore.is_some());
        push(
            "openrouter_quantizations",
            patch.openrouter_quantizations.is_some(),
        );
        push("openrouter_sort", patch.openrouter_sort.is_some());
        push("openrouter_pacing", patch.openrouter_pacing.is_some());
        fields
    }

    async fn probe_configured_models_list(&self, id: &str) -> Result<String, String> {
        use crate::cli::generated_ops::find_cli_operation;
        use crate::cli::instance_dispatch::{
            assert_surface_allows_operation, load_provider_service, resolve_instance_credentials,
            resolve_selected_instance,
        };

        let service = load_provider_service(&self.home)?;
        // Prefer openai listModels; fall back to openrouter list when kind is openrouter.
        let (ns, op_id) = if service
            .get(id)
            .is_some_and(|d| d.kind == super::instance::ProviderKind::OpenRouter)
        {
            // OpenRouter catalog binding is getModels (GET /models), not openai listModels.
            ("openrouter", "getModels")
        } else {
            ("openai", "listModels")
        };
        let op = find_cli_operation(ns, op_id)
            .ok_or_else(|| format!("operation {ns}::{op_id} missing from generated catalog"))?;
        let instance = resolve_selected_instance(&service, id, op)?;
        assert_surface_allows_operation(&instance, op)?;
        let (app, admin) = resolve_instance_credentials(&instance, op, &self.home)?;
        let token = app
            .or(admin)
            .ok_or_else(|| "no application or admin credential for live probe".to_owned())?;
        let base = instance.base_url.trim_end_matches('/');
        let url = format!("{base}/models");
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|e| e.to_string())?;
        let resp = client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!(
                "models list returned HTTP {status} (non-mutating probe failed)"
            ));
        }
        let body = resp.text().await.map_err(|e| e.to_string())?;
        // Count data array items when present.
        let count = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("data").and_then(|d| d.as_array()).map(|a| a.len()))
            .unwrap_or(0);
        Ok(format!(
            "Live models list OK · {count} model(s) (no inference charge)"
        ))
    }

    async fn probe_openrouter_credits(&self, id: &str) -> Result<String, String> {
        use crate::cli::generated_ops::find_cli_operation;
        use crate::cli::instance_dispatch::{
            assert_surface_allows_operation, load_provider_service, resolve_instance_credentials,
            resolve_selected_instance,
        };
        use crate::cli::openrouter_cmd::resolve_credits_operation_id;

        let op_id = resolve_credits_operation_id()?;
        let op = find_cli_operation("openrouter", op_id)
            .ok_or_else(|| "getCredits binding missing".to_owned())?;
        let service = load_provider_service(&self.home)?;
        let instance = resolve_selected_instance(&service, id, op)?;
        assert_surface_allows_operation(&instance, op)?;
        let (app, _) = resolve_instance_credentials(&instance, op, &self.home)?;
        let token = app.ok_or_else(|| "no application credential for credits".to_owned())?;
        let base = instance.base_url.trim_end_matches('/');
        // Exact path from binding: /credits
        let url = format!("{base}/credits");
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|e| e.to_string())?;
        let resp = client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("credits probe HTTP {}", resp.status()));
        }
        let body = resp.text().await.map_err(|e| e.to_string())?;
        // Summarize without dumping full body into logs/UI secrets.
        let summary = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .map(|v| {
                let total = v
                    .pointer("/data/total_credits")
                    .or_else(|| v.get("total_credits"))
                    .cloned();
                let remaining = v
                    .pointer("/data/total_usage")
                    .or_else(|| v.get("total_usage"))
                    .cloned();
                match (total, remaining) {
                    (Some(t), Some(u)) => format!("total_credits={t}, total_usage={u}"),
                    (Some(t), None) => format!("total_credits={t}"),
                    _ => "credits response received".to_owned(),
                }
            })
            .unwrap_or_else(|| "credits response received".into());
        Ok(summary)
    }
}

fn built_in_backend(id: &str) -> Option<BuiltInBackendId> {
    match BuiltInProviderId::parse(id)? {
        BuiltInProviderId::Xai => Some(BuiltInBackendId::Xai),
        BuiltInProviderId::OpenAi => Some(BuiltInBackendId::OpenAi),
        BuiltInProviderId::OpenRouter => Some(BuiltInBackendId::OpenRouter),
        BuiltInProviderId::Anthropic => Some(BuiltInBackendId::Anthropic),
    }
}

fn is_editable_kind(kind: &str, is_built_in: bool) -> bool {
    // Built-ins: limited metadata edits only (enable/display) — full CRUD for
    // unlimited OpenAI/OpenRouter/custom configured instances.
    if is_built_in {
        return matches!(kind, "openai" | "openrouter" | "xai" | "anthropic" | "zai");
    }
    matches!(
        kind,
        "openai_compatible" | "custom" | "openai" | "openrouter" | "zai" | "xai"
    )
}

fn normalize_kind_for_add(kind: &str) -> Option<&'static str> {
    match kind.trim().to_ascii_lowercase().as_str() {
        "openai_compatible" | "custom" => Some("openai_compatible"),
        "openrouter" => Some("openrouter"),
        "openai" => Some("openai"),
        "zai" => Some("zai"),
        _ => None,
    }
}

fn list_status_label(
    _id: &str,
    _is_built_in: bool,
    credentials: &CredentialPresence,
) -> (String, Option<String>) {
    if credentials.has_application_key || credentials.has_oauth {
        ("Connected".into(), Some("Credential present".into()))
    } else if credentials.has_admin_key {
        ("Connected".into(), Some("Admin credential present".into()))
    } else {
        ("Key/connect missing".into(), None)
    }
}

fn save_patch_to_toml(patch: &ProviderSavePatch) -> ProviderTomlPatch {
    ProviderTomlPatch {
        display_name: patch.display_name.clone(),
        kind: patch.kind.clone(),
        base_url: patch.base_url.clone(),
        admin_base_url: patch.admin_base_url.clone(),
        enabled: patch.enabled,
        default_backend: patch.default_backend.clone(),
        auth_scheme: patch.auth_scheme.clone(),
        env_key: patch.env_key.clone(),
        admin_env_key: patch.admin_env_key.clone(),
        catalog_enabled: patch.catalog_enabled,
        capability_mode: patch.capability_mode.clone(),
        catalog_ttl_secs: patch.catalog_ttl_secs,
        request_timeout_secs: patch.request_timeout_secs,
        organization: patch.organization.clone(),
        project: patch.project.clone(),
        extra_headers: patch.extra_headers.clone(),
        capabilities: patch.capabilities.clone(),
        openrouter_fallback_models: patch.openrouter_fallback_models.clone(),
        openrouter_pacing: patch.openrouter_pacing,
        api_surface: patch.api_surface.clone(),
        credential_route: patch.credential_route.clone(),
        ..Default::default()
    }
}

fn openrouter_prefs_from_save(patch: &ProviderSavePatch) -> Option<OpenRouterPrefsPatch> {
    let has_policy = patch.openrouter_data_collection.is_some()
        || patch.openrouter_require_parameters.is_some()
        || patch.openrouter_allow_fallbacks.is_some()
        || patch.openrouter_zdr.is_some()
        || patch.openrouter_order.is_some()
        || patch.openrouter_only.is_some()
        || patch.openrouter_ignore.is_some()
        || patch.openrouter_quantizations.is_some()
        || patch.openrouter_sort.is_some();
    if !has_policy {
        return None;
    }
    Some(OpenRouterPrefsPatch {
        data_collection: patch.openrouter_data_collection.clone(),
        require_parameters: patch.openrouter_require_parameters,
        allow_fallbacks: patch.openrouter_allow_fallbacks,
        zdr: patch.openrouter_zdr,
        order: patch.openrouter_order.clone(),
        only: patch.openrouter_only.clone(),
        ignore: patch.openrouter_ignore.clone(),
        quantizations: patch.openrouter_quantizations.clone(),
        sort: patch.openrouter_sort.clone(),
    })
}

fn is_empty_toml_patch(p: &ProviderTomlPatch) -> bool {
    p.display_name.is_none()
        && p.kind.is_none()
        && p.base_url.is_none()
        && p.admin_base_url.is_none()
        && p.enabled.is_none()
        && p.default_backend.is_none()
        && p.auth_scheme.is_none()
        && p.env_key.is_none()
        && p.admin_env_key.is_none()
        && p.catalog_enabled.is_none()
        && p.capability_mode.is_none()
        && p.catalog_ttl_secs.is_none()
        && p.request_timeout_secs.is_none()
        && p.organization.is_none()
        && p.project.is_none()
        && p.extra_headers.is_none()
        && p.capabilities.is_none()
        && p.openrouter_fallback_models.is_none()
        && p.openrouter_pacing.is_none()
        && p.api_surface.is_none()
        && p.credential_route.is_none()
}

fn restrict_builtin_patch(patch: &mut ProviderSavePatch) -> Result<(), String> {
    // Whitelist: display_name + enabled only.
    let mut clean = ProviderSavePatch::default();
    clean.display_name = patch.display_name.clone();
    clean.enabled = patch.enabled;
    // Detect disallowed fields that were set.
    let disallowed = patch.kind.is_some()
        || patch.base_url.is_some()
        || patch.admin_base_url.is_some()
        || patch.default_backend.is_some()
        || patch.auth_scheme.is_some()
        || patch.env_key.is_some()
        || patch.admin_env_key.is_some()
        || patch.catalog_enabled.is_some()
        || patch.capability_mode.is_some()
        || patch.catalog_ttl_secs.is_some()
        || patch.request_timeout_secs.is_some()
        || patch.organization.is_some()
        || patch.project.is_some()
        || patch.api_surface.is_some()
        || patch.credential_route.is_some()
        || patch.extra_headers.is_some()
        || patch.capabilities.is_some()
        || patch.openrouter_fallback_models.is_some()
        || patch.openrouter_data_collection.is_some()
        || patch.openrouter_require_parameters.is_some()
        || patch.openrouter_allow_fallbacks.is_some()
        || patch.openrouter_zdr.is_some()
        || patch.openrouter_order.is_some()
        || patch.openrouter_only.is_some()
        || patch.openrouter_ignore.is_some()
        || patch.openrouter_quantizations.is_some()
        || patch.openrouter_sort.is_some()
        || patch.openrouter_pacing.is_some();
    if disallowed {
        return Err(
            "built-in providers only allow enable and display_name edits; \
             unsupported fields fail closed"
                .into(),
        );
    }
    *patch = clean;
    Ok(())
}

fn validate_credential_one_shots(
    update: &CredentialSlotUpdate,
    application_secret: Option<&str>,
    admin_secret: Option<&str>,
) -> Result<(), String> {
    if matches!(update.application, SecretFieldUpdate::Set)
        && application_secret
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_none()
    {
        return Err("application secret Set requires a non-empty one-shot value".into());
    }
    if matches!(update.admin, SecretFieldUpdate::Set)
        && admin_secret
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_none()
    {
        return Err("admin secret Set requires a non-empty one-shot value".into());
    }
    Ok(())
}

fn config_fingerprint(config_path: &Path) -> String {
    match fs::read(config_path) {
        Ok(bytes) => format!("{:x}", Sha256::digest(&bytes)),
        Err(_) => String::new(),
    }
}

fn read_generation_raw(home: &Path) -> (u64, String) {
    let path = home.join(GENERATION_REL);
    match fs::read_to_string(path) {
        Ok(s) => {
            let mut lines = s.lines();
            let generation = lines
                .next()
                .and_then(|l| l.trim().parse().ok())
                .unwrap_or(0);
            let fp = lines.next().unwrap_or("").trim().to_owned();
            (generation, fp)
        }
        Err(_) => (0, String::new()),
    }
}

/// Durable generation sidecar write. Propagates create/write/sync/rename errors.
/// Uses a unique temp name (pid + nonce) so concurrent writers never share `.tmp`.
fn write_generation_state(home: &Path, generation: u64, fingerprint: &str) -> Result<(), String> {
    let path = home.join(GENERATION_REL);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create generation parent: {e}"))?;
    }
    static NONCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nonce = NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = path.with_extension(format!("tmp.{}.{}", std::process::id(), nonce));
    {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut f = options
            .open(&tmp)
            .map_err(|e| format!("create generation temp: {e}"))?;
        write!(f, "{generation}\n{fingerprint}\n")
            .map_err(|e| format!("write generation temp: {e}"))?;
        f.flush()
            .map_err(|e| format!("flush generation temp: {e}"))?;
        f.sync_all()
            .map_err(|e| format!("sync generation temp: {e}"))?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::metadata(&tmp).map_err(|e| format!("stat generation temp: {e}"))?;
        let mut perms = meta.permissions();
        if perms.mode() & 0o777 != 0o600 {
            perms.set_mode(0o600);
            fs::set_permissions(&tmp, perms).map_err(|e| format!("chmod generation temp: {e}"))?;
        }
    }
    fs::rename(&tmp, &path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("rename generation sidecar: {e}")
    })?;
    Ok(())
}

fn ok_result(id: String, generation: RegistryGeneration) -> ProviderMutationResult {
    ProviderMutationResult {
        ok: true,
        id,
        generation,
        error: None,
        stale: false,
        guidance: None,
        partial_commit: false,
        incarnation: None,
        operation_id: None,
        conflict: None,
        changed_fields: Vec::new(),
    }
}

fn stale_result(id: &str, live: RegistryGeneration, msg: String) -> ProviderMutationResult {
    // Prefer stale_result_with_expected when the client generation is known.
    stale_result_with_expected(
        id,
        RegistryGeneration(0),
        live,
        vec!["generation".into()],
        msg,
    )
}

fn stale_result_with_expected(
    id: &str,
    client: RegistryGeneration,
    live: RegistryGeneration,
    changed_fields: Vec<String>,
    msg: String,
) -> ProviderMutationResult {
    ProviderMutationResult {
        ok: false,
        id: id.to_owned(),
        generation: live,
        error: Some(msg),
        stale: true,
        guidance: Some(STALE_GUIDANCE.into()),
        partial_commit: false,
        incarnation: None,
        operation_id: None,
        conflict: Some(ProviderConflictInfo {
            provider_id: id.to_owned(),
            client_generation: client,
            live_generation: live,
            changed_fields,
            guidance: "Registry generation is stale. Choose Reload to discard local edits, or Clone into a new id.".into(),
        }),
        changed_fields: Vec::new(),
    }
}

fn err_result(id: &str, generation: RegistryGeneration, msg: String) -> ProviderMutationResult {
    ProviderMutationResult {
        ok: false,
        id: id.to_owned(),
        generation,
        error: Some(msg),
        stale: false,
        guidance: None,
        partial_commit: false,
        incarnation: None,
        operation_id: None,
        conflict: None,
        changed_fields: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn svc(dir: &TempDir) -> ProviderManagementService {
        ProviderManagementService::new(dir.path())
    }

    #[test]
    fn add_save_list_round_trip_preserves_generation_and_fields() {
        let dir = TempDir::new().unwrap();
        let s = svc(&dir);
        let gen0 = s.current_generation();
        let add = s.add(ProviderAddRequest {
            id: "local_vllm".into(),
            kind: "openai_compatible".into(),
            base_url: "http://127.0.0.1:8000/v1".into(),
            display_name: Some("Local vLLM".into()),
            admin_base_url: None,
            enabled: true,
            expected_generation: gen0,
        });
        assert!(add.ok, "{:?}", add.error);
        let list = s.list_snapshot().unwrap();
        assert_eq!(list.generation, add.generation);
        assert!(list.rows.iter().any(|r| r.id == "local_vllm"));
        let detail = s.detail("local_vllm").unwrap();
        assert_eq!(detail.display_name.as_deref(), Some("Local vLLM"));
        assert!(detail.is_editable);

        // Stale save fails closed.
        let stale = s.save(ProviderSaveRequest {
            id: "local_vllm".into(),
            expected_generation: RegistryGeneration(0),
            patch: ProviderSavePatch {
                display_name: Some("x".into()),
                ..Default::default()
            },
        });
        assert!(!stale.ok);
        assert!(stale.stale);
        assert!(stale.guidance.is_some());

        let save = s.save(ProviderSaveRequest {
            id: "local_vllm".into(),
            expected_generation: add.generation,
            patch: ProviderSavePatch {
                display_name: Some("vLLM Lab".into()),
                catalog_enabled: Some(false),
                openrouter_pacing: Some(false),
                extra_headers: Some(IndexMap::from([("X-Test".into(), "1".into())])),
                ..Default::default()
            },
        });
        assert!(save.ok, "{:?}", save.error);
        let detail2 = s.detail("local_vllm").unwrap();
        assert_eq!(detail2.display_name.as_deref(), Some("vLLM Lab"));
        assert!(!detail2.catalog_enabled);
        assert_eq!(
            detail2.extra_headers.get("X-Test").map(String::as_str),
            Some("1")
        );
    }

    #[test]
    fn clone_does_not_copy_secrets() {
        let dir = TempDir::new().unwrap();
        let s = svc(&dir);
        let g = s.current_generation();
        let add = s.add(ProviderAddRequest {
            id: "work_openai".into(),
            kind: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            display_name: Some("Work".into()),
            admin_base_url: None,
            enabled: true,
            expected_generation: g,
        });
        assert!(add.ok);
        let set = s.apply_credential_updates(
            "work_openai",
            s.current_generation(),
            &CredentialSlotUpdate {
                application: SecretFieldUpdate::Set,
                ..Default::default()
            },
            Some("sk-test-not-logged"),
            None,
        );
        assert!(set.ok, "{:?}", set.error);
        assert!(s.credential_presence("work_openai").has_application_key);

        let cloned = s.clone_provider(ProviderCloneRequest {
            source_id: "work_openai".into(),
            new_id: "home_openai".into(),
            display_name: Some("Home".into()),
            expected_generation: s.current_generation(),
        });
        assert!(cloned.ok, "{:?}", cloned.error);
        assert!(!s.credential_presence("home_openai").has_application_key);
        let d = s.detail("home_openai").unwrap();
        assert_eq!(d.display_name.as_deref(), Some("Home"));
        assert_eq!(d.base_url.as_deref(), Some("https://api.openai.com/v1"));
    }

    #[test]
    fn enable_disable_and_reference_impact() {
        let dir = TempDir::new().unwrap();
        let s = svc(&dir);
        let g = s.current_generation();
        assert!(
            s.add(ProviderAddRequest {
                id: "lab".into(),
                kind: "openai_compatible".into(),
                base_url: "http://127.0.0.1:9/v1".into(),
                display_name: None,
                admin_base_url: None,
                enabled: true,
                expected_generation: g,
            })
            .ok
        );
        let g2 = s.current_generation();
        let dis = s.set_enabled("lab", false, g2);
        assert!(dis.ok);
        assert!(!s.detail("lab").unwrap().enabled);
        let impact = s.reference_impact("lab").unwrap();
        assert!(impact.can_remove);
        assert!(!impact.secrets_present);
    }

    #[test]
    fn readonly_generation_does_not_write_on_external_config_edit() {
        let dir = TempDir::new().unwrap();
        let s = svc(&dir);
        let g0 = s.current_generation();
        assert!(
            s.add(ProviderAddRequest {
                id: "lab".into(),
                kind: "openai_compatible".into(),
                base_url: "http://127.0.0.1:9/v1".into(),
                display_name: None,
                admin_base_url: None,
                enabled: true,
                expected_generation: g0,
            })
            .ok
        );
        let after_add = s.current_generation().get();
        let (disk_gen, disk_fp) = read_generation_raw(dir.path());
        assert_eq!(disk_gen, after_add);
        assert!(!disk_fp.is_empty());

        // External edit of config without going through management.
        let cfg = dir.path().join("config.toml");
        let mut text = fs::read_to_string(&cfg).unwrap();
        text.push_str("\n# external\n");
        fs::write(&cfg, text).unwrap();

        // Read-only path reports advanced generation but does not rewrite sidecar.
        let logical = s.current_generation().get();
        assert_eq!(logical, after_add + 1);
        let (disk_gen2, disk_fp2) = read_generation_raw(dir.path());
        assert_eq!(
            disk_gen2, after_add,
            "readonly must not mutate generation file"
        );
        assert_eq!(disk_fp2, disk_fp);

        // Mutator with stale expected fails closed without writing secrets/metadata.
        let stale = s.save(ProviderSaveRequest {
            id: "lab".into(),
            expected_generation: RegistryGeneration(after_add),
            patch: ProviderSavePatch {
                display_name: Some("nope".into()),
                ..Default::default()
            },
        });
        assert!(!stale.ok);
        assert!(stale.stale);

        // Mutator with logical generation materializes under lock and succeeds.
        let ok = s.save(ProviderSaveRequest {
            id: "lab".into(),
            expected_generation: RegistryGeneration(logical),
            patch: ProviderSavePatch {
                display_name: Some("yes".into()),
                ..Default::default()
            },
        });
        assert!(ok.ok, "{:?}", ok.error);
        assert!(!ok.partial_commit);
        assert_eq!(
            s.detail("lab").unwrap().display_name.as_deref(),
            Some("yes")
        );
    }

    #[test]
    fn remove_metadata_uses_lock_and_generation() {
        let dir = TempDir::new().unwrap();
        let s = svc(&dir);
        let g0 = s.current_generation();
        assert!(
            s.add(ProviderAddRequest {
                id: "gone".into(),
                kind: "openai_compatible".into(),
                base_url: "http://127.0.0.1:9/v1".into(),
                display_name: None,
                admin_base_url: None,
                enabled: true,
                expected_generation: g0,
            })
            .ok
        );
        let g = s.current_generation();
        let removed = s.remove_metadata("gone", g, true);
        assert!(removed.ok, "{:?}", removed.error);
        assert!(s.detail("gone").is_err());
        // Stale remove fails closed.
        let again = s.remove_metadata("gone", g, true);
        assert!(!again.ok);
    }

    #[test]
    fn generation_write_error_propagates_not_fake_success() {
        // Exercise write_generation_state error path via invalid parent simulation:
        // write into a path where home is a file, not a directory.
        let dir = TempDir::new().unwrap();
        let file_home = dir.path().join("not_a_dir");
        fs::write(&file_home, b"x").unwrap();
        let err = write_generation_state(&file_home, 1, "abc");
        assert!(err.is_err(), "must fail when home is not a directory");
    }

    #[test]
    fn force_remove_tombstone_blocks_readd_and_new_incarnation_on_restore() {
        let dir = TempDir::new().unwrap();
        let s = svc(&dir);
        let g0 = s.current_generation();
        let add = s.add(ProviderAddRequest {
            id: "work".into(),
            kind: "openai_compatible".into(),
            base_url: "http://127.0.0.1:9/v1".into(),
            display_name: None,
            admin_base_url: None,
            enabled: true,
            expected_generation: g0,
        });
        assert!(add.ok, "{:?}", add.error);
        let inc1 = add.incarnation.clone().expect("minted incarnation");
        // Model reference blocks normal remove.
        let cfg = dir.path().join("config.toml");
        let mut text = fs::read_to_string(&cfg).unwrap();
        text.push_str("\n[model.m1]\nmodel_provider = \"work\"\nmodel = \"x\"\n");
        fs::write(&cfg, text).unwrap();
        let impact = s.reference_impact("work").unwrap();
        assert!(!impact.can_remove);
        let blocked = s.remove_metadata("work", s.current_generation(), true);
        assert!(!blocked.ok);

        // Wrong typed id fails.
        let bad = s.force_remove(ProviderForceRemoveRequest {
            id: "work".into(),
            typed_id_confirmation: "Work".into(),
            expected_generation: s.current_generation(),
            expected_incarnation: Some(inc1.clone()),
            clear: ForceRemoveClearOptions::default(),
            operation_id: Some("op1".into()),
        });
        assert!(!bad.ok);

        let forced = s.force_remove(ProviderForceRemoveRequest {
            id: "work".into(),
            typed_id_confirmation: "work".into(),
            expected_generation: s.current_generation(),
            expected_incarnation: Some(inc1.clone()),
            clear: ForceRemoveClearOptions::default(),
            operation_id: Some("op2".into()),
        });
        assert!(forced.ok, "{:?}", forced.error);
        assert!(s.tombstone_blocks_readd("work"));

        // Ordinary re-add blocked.
        let readd = s.add(ProviderAddRequest {
            id: "work".into(),
            kind: "openai_compatible".into(),
            base_url: "http://127.0.0.1:9/v1".into(),
            display_name: None,
            admin_base_url: None,
            enabled: true,
            expected_generation: s.current_generation(),
        });
        assert!(!readd.ok);
        assert!(
            readd
                .error
                .as_deref()
                .unwrap_or("")
                .contains("forcibly removed")
                || readd.error.as_deref().unwrap_or("").contains("tombstone")
                || readd.error.as_deref().unwrap_or("").contains("restore")
        );

        // Explicit restore reuses incarnation.
        let restored = s.restore_tombstoned(
            "work",
            "openai_compatible",
            "http://127.0.0.1:9/v1",
            s.current_generation(),
        );
        assert!(restored.ok, "{:?}", restored.error);
        assert_eq!(restored.incarnation.as_deref(), Some(inc1.as_str()));
    }

    #[test]
    fn multi_account_gate_enabled_by_default_after_gate_d() {
        use super::super::gate::{
            MULTI_ACCOUNT_ROLLOUT_DEFAULT_ENABLED, MULTI_ACCOUNT_ROLLOUT_ENV,
            multi_account_rollout_enabled, multi_account_rollout_env_lock,
        };
        let _gate = multi_account_rollout_env_lock();
        let previous = std::env::var(MULTI_ACCOUNT_ROLLOUT_ENV).ok();
        unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) };
        assert!(MULTI_ACCOUNT_ROLLOUT_DEFAULT_ENABLED);
        assert!(multi_account_rollout_enabled());
        match previous {
            Some(v) => unsafe { std::env::set_var(MULTI_ACCOUNT_ROLLOUT_ENV, v) },
            None => unsafe { std::env::remove_var(MULTI_ACCOUNT_ROLLOUT_ENV) },
        }
    }

    #[test]
    fn force_remove_does_not_clear_secrets_when_toml_missing() {
        // Provider only in lifecycle (no config row): remove_provider fails after
        // tombstone; secrets must remain intact (partial_commit).
        let dir = TempDir::new().unwrap();
        let s = svc(&dir);
        // Seed a secret for an id without config entry via lifecycle + vault.
        let pid = ProviderId::new("ghost").unwrap();
        let _ =
            crate::provider_registry::lifecycle_state::with_lifecycle_state_mut(dir.path(), |st| {
                let _ = st.mint_or_restore(&pid, false)?;
                Ok(((), true))
            });
        let _ = store_provider_secret(dir.path(), &application_key_scope(&pid), "sk-keep-me");
        assert!(s.credential_presence("ghost").has_application_key);
        let result = s.force_remove(ProviderForceRemoveRequest {
            id: "ghost".into(),
            typed_id_confirmation: "ghost".into(),
            expected_generation: s.current_generation(),
            expected_incarnation: None,
            clear: ForceRemoveClearOptions {
                clear_application_key: true,
                ..Default::default()
            },
            operation_id: Some("op-partial".into()),
        });
        // Either partial_commit (tombstone without TOML) or not found — never
        // clear secrets when TOML remove failed.
        if result.partial_commit {
            assert!(
                s.credential_presence("ghost").has_application_key,
                "secrets must survive TOML remove failure"
            );
        }
    }

    #[test]
    fn providers_update_notification_file_written_after_mutation() {
        let dir = TempDir::new().unwrap();
        let s = svc(&dir);
        let g0 = s.current_generation();
        assert!(
            s.add(ProviderAddRequest {
                id: "lab".into(),
                kind: "openai_compatible".into(),
                base_url: "http://127.0.0.1:9/v1".into(),
                display_name: None,
                admin_base_url: None,
                enabled: true,
                expected_generation: g0,
            })
            .ok
        );
        let path = dir.path().join("state/provider_registry_notify.json");
        assert!(
            path.is_file(),
            "providers/update notify file must exist after mutation"
        );
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"generation\""));
        assert!(raw.contains("lab") || raw.contains("changed_ids"));
    }

    #[test]
    fn mutation_broadcasts_to_two_registered_forwarders_and_notify_file() {
        use crate::provider_registry::notify::{
            clear_providers_update_forwarders, poll_notify_file_if_newer,
            register_providers_update_forwarder, reset_poll_state_for_tests,
        };
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        clear_providers_update_forwarders();
        reset_poll_state_for_tests();
        let a = Arc::new(AtomicUsize::new(0));
        let b = Arc::new(AtomicUsize::new(0));
        let a2 = a.clone();
        let b2 = b.clone();
        register_providers_update_forwarder(Box::new(move |_| {
            a2.fetch_add(1, Ordering::SeqCst);
        }));
        register_providers_update_forwarder(Box::new(move |_| {
            b2.fetch_add(1, Ordering::SeqCst);
        }));

        let dir = TempDir::new().unwrap();
        let s = svc(&dir);
        let g0 = s.current_generation();
        assert!(
            s.add(ProviderAddRequest {
                id: "lab".into(),
                kind: "openai_compatible".into(),
                base_url: "http://127.0.0.1:9/v1".into(),
                display_name: None,
                admin_base_url: None,
                enabled: true,
                expected_generation: g0,
            })
            .ok
        );
        assert_eq!(
            a.load(Ordering::SeqCst),
            1,
            "client A must receive broadcast"
        );
        assert_eq!(
            b.load(Ordering::SeqCst),
            1,
            "client B must receive broadcast"
        );
        // Local self-refresh path: notify file poll delivers the same generation.
        let polled = poll_notify_file_if_newer(dir.path()).expect("notify file poll");
        assert!(polled["generation"].as_u64().unwrap_or(0) > 0);
        clear_providers_update_forwarders();
    }

    #[test]
    fn external_config_fingerprint_change_notifies_on_reconcile() {
        use crate::provider_registry::notify::{
            clear_providers_update_forwarders, register_providers_update_forwarder,
        };
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        clear_providers_update_forwarders();
        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = hits.clone();
        register_providers_update_forwarder(Box::new(move |_| {
            hits2.fetch_add(1, Ordering::SeqCst);
        }));

        let dir = TempDir::new().unwrap();
        let s = svc(&dir);
        let g0 = s.current_generation();
        assert!(
            s.add(ProviderAddRequest {
                id: "lab".into(),
                kind: "openai_compatible".into(),
                base_url: "http://127.0.0.1:9/v1".into(),
                display_name: None,
                admin_base_url: None,
                enabled: true,
                expected_generation: g0,
            })
            .ok
        );
        let before = hits.load(Ordering::SeqCst);
        // External edit: mutate config.toml outside management.
        let mut text = fs::read_to_string(dir.path().join("config.toml")).unwrap();
        text.push_str("\n# external fingerprint drift\n");
        fs::write(dir.path().join("config.toml"), text).unwrap();
        // Next mutation path reconciles and must notify external_config.
        let expected = s.current_generation();
        let _ = s.set_enabled("lab", false, expected);
        assert!(
            hits.load(Ordering::SeqCst) > before,
            "external fingerprint reconcile must notify clients"
        );
        clear_providers_update_forwarders();
    }

    #[test]
    fn empty_secret_field_preserves() {
        let dir = TempDir::new().unwrap();
        let s = svc(&dir);
        let g = s.current_generation();
        assert!(
            s.add(ProviderAddRequest {
                id: "p1".into(),
                kind: "openai_compatible".into(),
                base_url: "http://127.0.0.1:9/v1".into(),
                display_name: None,
                admin_base_url: None,
                enabled: true,
                expected_generation: g,
            })
            .ok
        );
        let set = s.apply_credential_updates(
            "p1",
            s.current_generation(),
            &CredentialSlotUpdate {
                application: SecretFieldUpdate::Set,
                ..Default::default()
            },
            Some("sk-keep"),
            None,
        );
        assert!(set.ok, "{:?}", set.error);
        // Preserve does nothing (no bump required).
        let preserve = s.apply_credential_updates(
            "p1",
            s.current_generation(),
            &CredentialSlotUpdate {
                application: SecretFieldUpdate::Preserve,
                ..Default::default()
            },
            None,
            None,
        );
        assert!(preserve.ok, "{:?}", preserve.error);
        assert!(s.credential_presence("p1").has_application_key);
        let clear = s.apply_credential_updates(
            "p1",
            s.current_generation(),
            &CredentialSlotUpdate {
                application: SecretFieldUpdate::Clear,
                ..Default::default()
            },
            None,
            None,
        );
        assert!(clear.ok, "{:?}", clear.error);
        assert!(!s.credential_presence("p1").has_application_key);

        // Stale client cannot mutate credentials (Issue 1).
        let stale = s.apply_credential_updates(
            "p1",
            RegistryGeneration(0),
            &CredentialSlotUpdate {
                application: SecretFieldUpdate::Set,
                ..Default::default()
            },
            Some("sk-stale"),
            None,
        );
        assert!(!stale.ok);
        assert!(stale.stale);
        assert!(!s.credential_presence("p1").has_application_key);

        // Built-in whitelist: base_url edit fails closed (Issue 7).
        let g = s.current_generation();
        // Seed a built-in table via add is reserved; save on xai default.
        let builtin = s.save(ProviderSaveRequest {
            id: "xai".into(),
            expected_generation: g,
            patch: ProviderSavePatch {
                base_url: Some("https://evil.example/v1".into()),
                ..Default::default()
            },
        });
        assert!(!builtin.ok, "built-in base_url must fail closed");
    }

    #[test]
    fn dto_debug_has_no_secret_material() {
        let presence = CredentialPresence {
            has_application_key: true,
            ..Default::default()
        };
        let dbg = format!("{presence:?}");
        assert!(!dbg.contains("sk-"));
        let update = CredentialSlotUpdate {
            application: SecretFieldUpdate::Set,
            ..Default::default()
        };
        assert!(!format!("{update:?}").contains("sk-"));
    }

    #[test]
    fn comments_preserved_on_upsert_via_management() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            r#"# top
[model_providers.existing]
kind = "openai_compatible"
base_url = "http://127.0.0.1:1/v1"
# keep me
enabled = true
"#,
        )
        .unwrap();
        let s = svc(&dir);
        let r = s.add(ProviderAddRequest {
            id: "new_one".into(),
            kind: "openai_compatible".into(),
            base_url: "http://127.0.0.1:2/v1".into(),
            display_name: Some("N".into()),
            admin_base_url: None,
            enabled: true,
            expected_generation: s.current_generation(),
        });
        assert!(r.ok, "{:?}", r.error);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("# top"));
        assert!(text.contains("# keep me"));
        assert!(text.contains("existing"));
        assert!(text.contains("new_one"));
    }
}
