use super::persist::{update_config, update_config_checked};
use anyhow::Result;

// ---------------------------------------------------------------------------
// Settings helpers — typed disk-write wrappers for each setting.
// All route through `update_config` → `merge_section` → `save_config`.
// ---------------------------------------------------------------------------

/// Persist `[ui].compact_mode` via `update_config`.
pub async fn set_compact_mode(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.compact_mode = value).await
}

/// Persist `[ui].show_timestamps` via `update_config`. `UiConfig::show_timestamps`
/// is `Option<bool>` — pager-side `None` means "use default" — so we wrap.
pub async fn set_show_timestamps(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.show_timestamps = Some(value)).await
}

/// Persist `[ui].show_timeline` via `update_config`. Same `Option<bool>`
/// shape as `show_timestamps`.
pub async fn set_show_timeline(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.show_timeline = Some(value)).await
}

pub async fn set_page_flip_on_send(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.page_flip_on_send = Some(value)).await
}

/// Persist `[ui].combine_queued_prompts` via `update_config`.
pub async fn set_combine_queued_prompts(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.combine_queued_prompts = Some(value)).await
}

/// Persist `[ui].simple_mode` via `update_config`. Same `Option<bool>`
/// shape as `show_timestamps`.
pub async fn set_simple_mode(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.simple_mode = Some(value)).await
}

/// Persist `[ui.contextual_hints].undo` via `update_config`. The nested struct
/// stays out of `config.toml` until a tip is toggled (`skip_serializing_if`).
pub async fn set_contextual_hint_undo(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.contextual_hints.undo = Some(value)).await
}

/// Persist `[ui.contextual_hints].plan_mode` via `update_config`.
pub async fn set_contextual_hint_plan_mode(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.contextual_hints.plan_mode = Some(value)).await
}

/// Persist `[ui.contextual_hints].image_input` via `update_config`.
pub async fn set_contextual_hint_image_input(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.contextual_hints.image_input = Some(value)).await
}

/// Persist `[ui.contextual_hints].send_now` via `update_config`.
pub async fn set_contextual_hint_send_now(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.contextual_hints.send_now = Some(value)).await
}

/// Persist `[ui.contextual_hints].small_screen` via `update_config`.
pub async fn set_contextual_hint_small_screen(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.contextual_hints.small_screen = Some(value)).await
}

/// Persist `[ui.contextual_hints].word_select` via `update_config`.
pub async fn set_contextual_hint_word_select(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.contextual_hints.word_select = Some(value)).await
}

/// Persist `[ui.contextual_hints].ssh_wrap` via `update_config`.
pub async fn set_contextual_hint_ssh_wrap(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.contextual_hints.ssh_wrap = Some(value)).await
}

/// Persist `[ui].theme` via `update_config`. Caller must pass the
/// canonical theme name (`groknight`, `tokyonight`, `auto`, etc.).
pub async fn set_theme(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.theme = Some(value)).await
}

/// Persist `[ui].auto_dark_theme` via `update_config`. `UiConfig::auto_dark_theme`
/// is `Option<String>` (canonical theme name; `auto` is rejected by the
/// pager's `load_auto_theme_config` filter at read time to prevent
/// circular reference).
pub async fn set_auto_dark_theme(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.auto_dark_theme = Some(value)).await
}

/// Persist `[ui].auto_light_theme` via `update_config`. Same shape as
/// [`set_auto_dark_theme`].
pub async fn set_auto_light_theme(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.auto_light_theme = Some(value)).await
}

/// Maximum length (in bytes) accepted by [`set_default_model`].
/// Defense against callers bypassing catalog validation.
pub const MAX_DEFAULT_MODEL_LEN: usize = 256;

/// Persist `[models].default` and dismiss any active campaign nudging it (an
/// explicit user pick wins over the soft campaign default).
///
/// This is the only sanctioned writer of `models.default`; it routes through
/// [`super::campaigns::persist_models_default`] so a user pick always dismisses
/// an active campaign. Do not persist `models.default` via raw `update_config`,
/// or a campaign would keep overriding the user's choice.
///
/// Caller must validate `value` against the model catalog first.
/// Empty string clears the field (falls back to remote/built-in default).
/// Length over [`MAX_DEFAULT_MODEL_LEN`] returns `Err`.
pub async fn set_default_model(value: String) -> Result<()> {
    super::campaigns::persist_models_default(
        if value.is_empty() { None } else { Some(value) },
        None,
    )
    .await
}

/// Persist `[privacy].privacy_banner_acked` (RFC 3339 UTC dismiss time).
pub async fn set_privacy_banner_acked(acked_at_rfc3339: String) -> Result<()> {
    update_config(|cfg| {
        cfg.privacy.privacy_banner_acked = Some(acked_at_rfc3339);
    })
    .await
}

/// Persist `[ui].fork_secondary_model` via `update_config`.
///
/// Caller must validate against the model catalog. Empty string
/// restores the built-in default. Length > [`MAX_DEFAULT_MODEL_LEN`] → `Err`.
pub async fn set_fork_secondary_model(value: String) -> Result<()> {
    if value.len() > MAX_DEFAULT_MODEL_LEN {
        anyhow::bail!(
            "fork_secondary_model name too long ({} > {} bytes)",
            value.len(),
            MAX_DEFAULT_MODEL_LEN
        );
    }
    update_config(|cfg| {
        cfg.ui.fork_secondary_model = if value.is_empty() {
            crate::models::default_model().to_string()
        } else {
            value
        };
    })
    .await
}

/// Bounds for [`set_max_thoughts_width`]. Mirrored from the pager's
/// registry consts; a CI test pins the agreement.
const MAX_THOUGHTS_WIDTH_SHELL_MIN: i64 = 40;
const MAX_THOUGHTS_WIDTH_SHELL_MAX: i64 = 500;

/// Persist `[ui].max_thoughts_width` via `update_config`.
/// Defensively clamps to `[40, 500]` at the shell boundary.
pub async fn set_max_thoughts_width(value: i64) -> Result<()> {
    let clamped = value.clamp(MAX_THOUGHTS_WIDTH_SHELL_MIN, MAX_THOUGHTS_WIDTH_SHELL_MAX) as u16;
    update_config(|cfg| cfg.ui.max_thoughts_width = clamped).await
}

/// Persist `[ui].scroll_speed` via `update_config`.
/// Defensively clamps to `[1, 100]` at the shell boundary.
pub async fn set_scroll_speed(value: i64) -> Result<()> {
    let clamped = value.clamp(1, 100) as u8;
    update_config(|cfg| cfg.ui.scroll_speed = Some(clamped)).await
}

/// Persist `[ui].scroll_mode` (`auto` | `wheel` | `trackpad`) via `update_config`.
pub async fn set_scroll_mode(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.scroll_mode = Some(value)).await
}

/// Persist `[ui].invert_scroll` via `update_config`.
pub async fn set_invert_scroll(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.invert_scroll = Some(value)).await
}

/// Persist `[ui.display_refresh].auto_cadence_enabled` via `update_config`.
/// Nested field only — does not replace the whole `display_refresh` object.
pub async fn set_display_refresh_auto_cadence(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.display_refresh.auto_cadence_enabled = Some(value)).await
}

/// Persist `[ui].scroll_lines` via `update_config`.
/// Defensively clamps to `[1, 10]` at the shell boundary.
pub async fn set_scroll_lines(value: i64) -> Result<()> {
    let clamped = value.clamp(1, 10) as u8;
    update_config(|cfg| cfg.ui.scroll_lines = Some(clamped)).await
}

/// Persist `[ui].vim_mode` via `update_config`.
pub async fn set_vim_mode(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.vim_mode = Some(value)).await
}

/// Persist `[ui].remember_tool_approvals` via `update_config`.
pub async fn set_remember_tool_approvals(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.remember_tool_approvals = Some(value)).await
}

/// Persist `[ui].show_thinking_blocks` via `update_config`.
pub async fn set_show_thinking_blocks(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.show_thinking_blocks = Some(value)).await
}

/// Persist `[ui].prompt_suggestions` via `update_config`.
pub async fn set_prompt_suggestions(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.prompt_suggestions = Some(value)).await
}

/// Persist `[toolset.ask_user_question].timeout_enabled` via `update_config`
/// (the user tier of the shell's tiered resolver; the effective value is
/// re-resolved at agent build).
pub async fn set_ask_user_question_timeout_enabled(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ask_user_question.timeout_enabled = Some(value)).await
}

/// Persist `[ui].group_tool_verbs` via `update_config`.
pub async fn set_group_tool_verbs(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.group_tool_verbs = Some(value)).await
}

/// Persist `[ui].collapsed_edit_blocks` via `update_config`.
pub async fn set_collapsed_edit_blocks(value: bool) -> Result<()> {
    update_config(|cfg| cfg.ui.collapsed_edit_blocks = Some(value)).await
}

/// Persist `[ui].keep_text_selection` (`flash` | `hold` | `word_select`).
/// Clears the legacy `selection_highlight_duration_ms` and the retired
/// `double_click_action` keys it supersedes so the two can never drift (one-shot
/// disk migration away from the legacy key on any Settings write).
pub async fn set_keep_text_selection(value: String) -> Result<()> {
    update_config(|cfg| {
        cfg.ui.keep_text_selection = Some(value);
        cfg.ui.selection_highlight_duration_ms = None;
        cfg.ui.double_click_action = None;
    })
    .await
}

/// Persist `[ui].render_mermaid` via `update_config`. Value is one of the
/// canonical strings `auto` | `on` | `off`.
pub async fn set_render_mermaid(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.render_mermaid = Some(value)).await
}

/// Persist `[ui].hunk_tracker_mode` via `update_config`. Value is one of the
/// canonical strings `agent_only` | `all_dirty` | `off`.
/// Restart-required: the mode is read once at connect time.
pub async fn set_hunk_tracker_mode(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.hunk_tracker_mode = Some(value)).await
}

/// Persist `[ui].voice_capture_mode` via `update_config`. Value is one of the
/// canonical strings `toggle` | `hold`.
pub async fn set_voice_capture_mode(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.voice_capture_mode = Some(value)).await
}

/// Persist `[ui].voice_stt_language` via `update_config`. Value is a canonical
/// language code from the settings catalog (`en`, `es`, …) or `auto` (system
/// locale, falling back to English).
pub async fn set_voice_stt_language(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.voice_stt_language = Some(value)).await
}

/// Persist `[language].conversation` (BCP-47 tag). Empty string or `"off"`
/// clears the field and deactivates the dual-language policy.
pub async fn set_language_conversation(value: String) -> Result<()> {
    update_config(|cfg| {
        cfg.language.conversation = if value.trim().is_empty() || value == "off" {
            None
        } else {
            Some(value)
        };
    })
    .await
}

/// Persist `[language].artifact` (BCP-47 tag). Rejected when the artifact
/// language is locked by a project/managed/requirements layer.
///
/// Lock status is taken from the **effective** config, not the user-layer
/// file: a managed/requirements/project lock is otherwise invisible to
/// `load_from_disk()`.
pub async fn set_language_artifact(value: String) -> Result<()> {
    if language_artifact_is_locked() {
        anyhow::bail!(
            "artifact language is locked by project or managed configuration and cannot be changed"
        );
    }
    update_config(|cfg| {
        cfg.language.artifact = if value.trim().is_empty() {
            None
        } else {
            Some(value)
        };
    })
    .await
}

/// Whether `[language].artifact` is locked in the merged effective config.
pub fn language_artifact_is_locked() -> bool {
    crate::config::load_effective_config()
        .ok()
        .is_some_and(|root| language_artifact_locked_in(&root))
}

/// Lock bit from an already-merged effective TOML tree.
pub fn language_artifact_locked_in(effective: &toml::Value) -> bool {
    effective
        .get("language")
        .cloned()
        .and_then(|value| {
            value
                .try_into::<crate::agent::config::LanguageConfig>()
                .ok()
        })
        .is_some_and(|language| language.artifact_locked)
}

#[cfg(test)]
mod language_lock_tests {
    use super::language_artifact_locked_in;
    use xai_grok_config::ConfigLayers;

    #[test]
    fn managed_lock_cannot_be_overridden_by_user_toml() {
        let layers = ConfigLayers {
            managed: toml::from_str("[language]\nartifact = \"en-US\"\nartifact_locked = true\n")
                .unwrap(),
            user: toml::from_str("[language]\nartifact = \"ja-JP\"\n").unwrap(),
            ..Default::default()
        };
        let effective = layers.effective_config_base();
        assert!(language_artifact_locked_in(&effective));
        assert_eq!(
            effective["language"]["artifact"].as_str(),
            Some("en-US"),
            "user TOML must not beat a managed artifact lock"
        );
    }

    #[test]
    fn unlocked_user_artifact_is_writable() {
        let layers = ConfigLayers {
            user: toml::from_str("[language]\nartifact = \"ja-JP\"\n").unwrap(),
            ..Default::default()
        };
        assert!(!language_artifact_locked_in(
            &layers.effective_config_base()
        ));
    }
}

/// Persist `[ui].default_selected_permission` via `update_config`. Value is
/// one of the canonical strings from `DEFAULT_SELECTED_PERMISSION_CHOICES`
/// (`default` | `allow_once` | `allow_always` | `reject`); `default` is the
/// "no preselection" sentinel.
pub async fn set_default_selected_permission(value: String) -> Result<()> {
    update_config(|cfg| cfg.ui.default_selected_permission = Some(value)).await
}

/// Persist `[ui].cancel_subagents_on_turn_cancel` via `update_config`.
/// Canonical values: `ask` (clear / prompt each time), `always_stop`,
/// `always_continue`.
pub async fn set_cancel_subagents_on_turn_cancel(value: String) -> Result<()> {
    update_config(|cfg| {
        cfg.ui.cancel_subagents_on_turn_cancel = if value == "ask" { None } else { Some(value) };
    })
    .await
}

/// Persist `[ui].screen_mode` (`fullscreen` | `minimal`). Empty clears the key.
pub async fn set_screen_mode(value: String) -> Result<()> {
    update_config(|cfg| {
        cfg.ui.screen_mode = if value.is_empty() { None } else { Some(value) };
    })
    .await
}

/// Persist `[cli].show_tips` via `update_config`.
/// Restart-required: `resolve_tips` reads this once at startup.
pub async fn set_show_tips(value: bool) -> Result<()> {
    update_config(|cfg| cfg.cli.show_tips = Some(value)).await
}

/// Persist `[cli].auto_update` via `update_config`.
/// Restart-required: auto-update check fires once on startup.
pub async fn set_auto_update(value: bool) -> Result<()> {
    update_config(|cfg| cfg.cli.auto_update = Some(value)).await
}

/// Persist `[compaction].strategy` after validating its canonical value.
pub async fn set_compaction_strategy(value: String) -> Result<()> {
    let strategy = match value.as_str() {
        "auto" => crate::agent::config::CompactionStrategy::Auto,
        "rolling" => crate::agent::config::CompactionStrategy::Rolling,
        "full_replace" => crate::agent::config::CompactionStrategy::FullReplace,
        _ => anyhow::bail!("invalid compaction strategy `{value}`"),
    };
    update_config_checked(|cfg| {
        cfg.compaction.strategy = Some(strategy);
        cfg.compaction.normalize_validate()?;
        Ok(())
    })
    .await
}

/// Persist `[compaction].trigger_policy` after validating its canonical value.
pub async fn set_compaction_trigger_policy(value: String) -> Result<()> {
    let trigger = match value.as_str() {
        "fixed" => crate::agent::config::CompactionTriggerPolicy::Fixed,
        "dynamic" => crate::agent::config::CompactionTriggerPolicy::Dynamic,
        _ => anyhow::bail!("invalid compaction trigger policy `{value}`"),
    };
    update_config_checked(|cfg| {
        cfg.compaction.trigger_policy = Some(trigger);
        cfg.compaction.normalize_validate()?;
        Ok(())
    })
    .await
}

/// Persist `[compaction].rolling_band_count`; valid values are 3 through 8.
pub async fn set_compaction_band_count(value: i64) -> Result<()> {
    let count =
        usize::try_from(value).map_err(|_| anyhow::anyhow!("invalid band count {value}"))?;
    if !(3..=8).contains(&count) {
        anyhow::bail!("compaction band count must be between 3 and 8, got {value}");
    }
    update_config_checked(|cfg| {
        cfg.compaction.rolling_band_count = Some(count);
        cfg.compaction.normalize_validate()?;
        Ok(())
    })
    .await
}

async fn set_compaction_model_at(index: usize, value: String) -> Result<()> {
    let model = if value.is_empty() {
        None
    } else {
        Some(crate::agent::config::CompactionModelRef::new(value)?)
    };
    update_config_checked(move |cfg| {
        let mut models = cfg.compaction.normalize_validate()?.models;
        if index == 0 {
            models[0] = model.unwrap_or_else(|| {
                crate::agent::config::CompactionModelRef::new("@session".to_owned())
                    .expect("@session is valid")
            });
        } else if let Some(model) = model {
            if models.len() == 1 {
                models.push(model);
            } else {
                models[1] = model;
            }
        } else {
            models.truncate(1);
        }
        let mut candidate = cfg.compaction.clone();
        candidate.models = models;
        candidate.normalize_validate()?;
        cfg.compaction = candidate;
        Ok(())
    })
    .await
}

/// Persist the primary compaction route. Empty restores `@session`.
pub async fn set_compaction_primary_model(value: String) -> Result<()> {
    set_compaction_model_at(0, value).await
}

/// Persist or clear the optional fallback compaction route.
pub async fn set_compaction_fallback_model(value: String) -> Result<()> {
    set_compaction_model_at(1, value).await
}

/// Persist `[media].mode` after validating its canonical value.
pub async fn set_media_routing(value: String) -> Result<()> {
    let mode = match value.as_str() {
        "auto" => crate::config::MediaMode::Auto,
        "tools_only" => crate::config::MediaMode::ToolsOnly,
        "off" => crate::config::MediaMode::Off,
        _ => anyhow::bail!("invalid media routing `{value}`"),
    };
    update_config(|cfg| cfg.media.mode = mode).await
}

fn set_media_model_field(field: &str, value: String) -> impl FnOnce(&mut super::mcp::Config) + '_ {
    move |cfg| {
        let resolved = if value.is_empty() { None } else { Some(value) };
        match field {
            "image_model" => cfg.media.image_model = resolved,
            "audio_model" => cfg.media.audio_model = resolved,
            "video_model" => cfg.media.video_model = resolved,
            "file_model" => cfg.media.file_model = resolved,
            _ => unreachable!("unknown media model field `{field}`"),
        }
    }
}

/// Persist `[media].image_model`. Empty restores the `@session` default.
pub async fn set_media_image_model(value: String) -> Result<()> {
    if value.len() > MAX_DEFAULT_MODEL_LEN {
        anyhow::bail!(
            "media image model name too long ({} > {} bytes)",
            value.len(),
            MAX_DEFAULT_MODEL_LEN
        );
    }
    let value = if value.is_empty() {
        "@session".to_owned()
    } else {
        value
    };
    update_config(set_media_model_field("image_model", value)).await
}

/// Persist or clear `[media].audio_model`. Empty clears the route.
pub async fn set_media_audio_model(value: String) -> Result<()> {
    if value.len() > MAX_DEFAULT_MODEL_LEN {
        anyhow::bail!(
            "media audio model name too long ({} > {} bytes)",
            value.len(),
            MAX_DEFAULT_MODEL_LEN
        );
    }
    update_config(set_media_model_field("audio_model", value)).await
}

/// Persist or clear `[media].video_model`. Empty clears the route.
pub async fn set_media_video_model(value: String) -> Result<()> {
    if value.len() > MAX_DEFAULT_MODEL_LEN {
        anyhow::bail!(
            "media video model name too long ({} > {} bytes)",
            value.len(),
            MAX_DEFAULT_MODEL_LEN
        );
    }
    update_config(set_media_model_field("video_model", value)).await
}

/// Persist or clear `[media].file_model`. Empty clears the route.
pub async fn set_media_file_model(value: String) -> Result<()> {
    if value.len() > MAX_DEFAULT_MODEL_LEN {
        anyhow::bail!(
            "media file model name too long ({} > {} bytes)",
            value.len(),
            MAX_DEFAULT_MODEL_LEN
        );
    }
    update_config(set_media_model_field("file_model", value)).await
}
