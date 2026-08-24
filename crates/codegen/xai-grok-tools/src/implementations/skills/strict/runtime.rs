//! Runtime inventory, command-only DTO, and load-time revalidation gates.
//!
//! Every SKILL.md source is ingested through the canonical strict validator.
//! Only valid rows become [`SkillInfo`]. Invalid rows are published only as
//! quarantined diagnostics. Flat `commands/*.md` files stay on a command-only
//! path and cannot enter skill advertisement, invocation, preload, or Prime.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::inventory::{DiscoveredSkill, SkillIdentity, SkillInventory};
use super::spec::SKILL_MD_FILE_NAME;
use super::validator::{
    StrictSkillInput, StrictSkillOutcome, validate_strict_skill, validate_strict_skill_dir,
};
use crate::implementations::skills::discovery::{
    SkillParseError, extract_first_paragraph, is_vendor_default_skill, parse_skill_frontmatter,
};
use crate::implementations::skills::skill::{extract_skill_body, format_skill_name};
use crate::implementations::skills::types::{SkillInfo, SkillScope};

/// Flat legacy command markdown. Never a skill: not advertised to the model
/// skill tool, not preloaded, and not primed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyCommand {
    pub name: String,
    pub description: String,
    pub path: String,
    pub scope: SkillScope,
    pub argument_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_data: Option<String>,
}

impl LegacyCommand {
    /// Slash-only projection. `disable_model_invocation` is always true so a
    /// command cannot leak into the native skill tool or Prime eligibility.
    pub fn to_slash_skill(&self) -> SkillInfo {
        SkillInfo {
            name: self.name.clone(),
            description: self.description.clone(),
            has_user_specified_description: !self.description.is_empty(),
            argument_hint: self.argument_hint.clone(),
            path: self.path.clone(),
            scope: self.scope,
            plugin_name: self.plugin_name.clone(),
            plugin_version: self.plugin_version.clone(),
            plugin_root: self.plugin_root.clone(),
            plugin_data: self.plugin_data.clone(),
            user_invocable: true,
            disable_model_invocation: true,
            enabled: true,
            ..SkillInfo::default()
        }
    }
}

/// Complete ingest of one source generation: valid skills, quarantined
/// diagnostics, and command-only rows.
#[derive(Debug, Clone)]
pub struct SkillSourceReport {
    pub inventory: SkillInventory,
    pub skills: Vec<SkillInfo>,
    pub commands: Vec<LegacyCommand>,
}

impl SkillSourceReport {
    pub fn empty(generation: u64) -> Self {
        Self {
            inventory: SkillInventory::new(generation, Vec::new(), Vec::new()),
            skills: Vec::new(),
            commands: Vec::new(),
        }
    }

    /// Combine source reports into one generation. Valid/quarantined rows are
    /// concatenated; callers still apply name-precedence dedup on `skills`.
    pub fn merge(generation: u64, reports: impl IntoIterator<Item = Self>) -> Self {
        let mut skills = Vec::new();
        let mut commands = Vec::new();
        let mut valid = Vec::new();
        let mut quarantined = Vec::new();
        for report in reports {
            skills.extend(report.skills);
            commands.extend(report.commands);
            valid.extend(report.inventory.valid);
            quarantined.extend(report.inventory.quarantined);
        }
        Self {
            inventory: SkillInventory::new(generation, valid, quarantined),
            skills,
            commands,
        }
    }
}

/// Bounded, path-free reason a skill failed load-time revalidation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillLoadError {
    NotASkill,
    NotRegularFile,
    Symlink,
    NotFound,
    Unreadable,
    Quarantined,
    IdentityChanged,
}

/// Strictly revalidated skill plus the SKILL.md bytes read from the
/// no-follow file descriptor. Invocation paths must use `content` instead
/// of re-opening the path string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevalidatedSkillFile {
    pub discovered: DiscoveredSkill,
    pub content: String,
}

/// Hard cap on SKILL.md bytes loaded at invocation (same order as Prime).
const MAX_SKILL_MD_BYTES: u64 = 64 * 1024;
/// Cap on openat components from the trusted walk root to SKILL.md.
const MAX_NOFOLLOW_WALK_COMPONENTS: usize = 16;
/// Cap on openat components from `/` (or a platform prefix alias) to the
/// trusted walk root. Tempfile and nested collection paths exceed the
/// relative SKILL.md cap.
#[cfg(unix)]
const MAX_ABSOLUTE_NOFOLLOW_WALK_COMPONENTS: usize = 64;

thread_local! {
    static AFTER_NOFOLLOW_READ: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        const { std::cell::RefCell::new(None) };
    static BEFORE_WALK_ROOT_OPEN: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        const { std::cell::RefCell::new(None) };
}

/// Install a one-shot hook that runs after the no-follow fd is read and
/// before the post-read path identity check. Tests use this to swap the
/// directory entry between revalidation and the would-be following reopen.
#[doc(hidden)]
pub fn set_after_nofollow_read_hook(hook: impl FnOnce() + 'static) {
    AFTER_NOFOLLOW_READ.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(hook));
    });
}

fn run_after_nofollow_read_hook() {
    let hook = AFTER_NOFOLLOW_READ.with(|slot| slot.borrow_mut().take());
    if let Some(hook) = hook {
        hook();
    }
}

/// Install a one-shot hook that runs after last-component ancestor lstat
/// of `walk_root` and before the no-follow openat walk from `/`. Tests use
/// this to swap a stamped skill-dir grandparent in the POSIX last-component
/// window that a path-open of `walk_root` would follow.
#[doc(hidden)]
pub fn set_before_walk_root_open_hook(hook: impl FnOnce() + 'static) {
    BEFORE_WALK_ROOT_OPEN.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(hook));
    });
}

fn run_before_walk_root_open_hook() {
    let hook = BEFORE_WALK_ROOT_OPEN.with(|slot| slot.borrow_mut().take());
    if let Some(hook) = hook {
        hook();
    }
}

impl std::fmt::Display for SkillLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotASkill => f.write_str("not a SKILL.md skill file"),
            Self::NotRegularFile => f.write_str("skill file is not a regular file"),
            Self::Symlink => f.write_str("skill file is a symlink"),
            Self::NotFound => f.write_str("skill file not found"),
            Self::Unreadable => f.write_str("skill file could not be read"),
            Self::Quarantined => f.write_str("skill failed strict validation"),
            Self::IdentityChanged => f.write_str("skill identity changed after discovery"),
        }
    }
}

/// True when `path` is a regular `SKILL.md` candidate (name only; does not stat).
pub fn is_skill_md_path(path: impl AsRef<Path>) -> bool {
    path.as_ref()
        .file_name()
        .is_some_and(|name| name == SKILL_MD_FILE_NAME)
}

/// True when `path` is a flat legacy command markdown file (not SKILL.md).
pub fn is_legacy_command_path(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    path.extension().and_then(|ext| ext.to_str()) == Some("md") && !is_skill_md_path(path)
}

/// Directory that is not a symlink.
pub fn is_real_directory(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|meta| meta.is_dir())
        .unwrap_or(false)
}

/// Regular file that is not a symlink.
pub fn is_regular_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|meta| meta.is_file())
        .unwrap_or(false)
}

/// Stable qualified identity used for toggles and invocation (`scope:name` or
/// `plugin:name`).
pub fn skill_qualified_identity(skill: &SkillInfo) -> String {
    format_skill_name(skill)
}

/// True when `disabled` names this skill by qualified identity, dedup key, or
/// bare name.
pub fn skill_matches_toggle(skill: &SkillInfo, disabled: &HashSet<&str>) -> bool {
    disabled.contains(skill.name.as_str())
        || disabled.contains(skill_qualified_identity(skill).as_str())
        || disabled.contains(skill.dedup_key().as_str())
}

/// Convert a strictly valid discovered skill into the runtime `SkillInfo`.
pub fn skill_info_from_discovered(
    discovered: &DiscoveredSkill,
    path: impl Into<String>,
) -> SkillInfo {
    let manifest = &discovered.manifest;
    let grok = &manifest.grok;
    let author = manifest.metadata.get("author").cloned();
    let metadata = (!manifest.metadata.is_empty()).then(|| {
        manifest
            .metadata
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    });
    let allowed_tools = {
        let tokens = manifest
            .allowed_tool_tokens()
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        (!tokens.is_empty()).then_some(tokens)
    };
    SkillInfo {
        name: manifest.name.clone(),
        display_name: None,
        description: manifest.description.clone(),
        has_user_specified_description: true,
        paths: grok.paths.clone(),
        when_to_use: grok.when_to_use.clone(),
        short_description: grok.short_description.clone(),
        author,
        argument_hint: grok.argument_hint.clone(),
        license: manifest.license.clone(),
        compatibility: manifest.compatibility.clone(),
        metadata,
        path: path.into(),
        scope: discovered.identity.scope.unwrap_or(SkillScope::User),
        config_source: None,
        plugin_name: None,
        plugin_version: None,
        plugin_root: None,
        collection_root: None,
        plugin_data: None,
        allowed_tools,
        model: grok.model.clone(),
        effort: grok.effort.clone(),
        user_invocable: grok.user_invocable.unwrap_or(true),
        disable_model_invocation: grok.disable_model_invocation.unwrap_or(false),
        enabled: true,
        body: None,
    }
}

/// Ingest skill and command files for one generation.
///
/// SKILL.md files are validated strictly. Invalid rows appear only in
/// `inventory.quarantined`. Flat `*.md` command files are parsed onto the
/// command-only path. Vendor-default denylist entries are dropped silently.
pub fn ingest_skill_sources(
    files: Vec<(PathBuf, SkillScope)>,
    generation: u64,
) -> SkillSourceReport {
    let mut valid = Vec::new();
    let mut quarantined = Vec::new();
    let mut skills = Vec::new();
    let mut commands = Vec::new();

    for (path, scope) in files {
        if is_legacy_command_path(&path) {
            if let Some(command) = parse_legacy_command_file(&path, scope) {
                commands.push(command);
            }
            continue;
        }

        let path_str = path.to_string_lossy();
        let parent_name = path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if is_vendor_default_skill(&path_str, parent_name) {
            continue;
        }

        match ingest_skill_path(&path, scope) {
            StrictSkillOutcome::Valid(discovered) => {
                skills.push(skill_info_from_discovered(
                    &discovered,
                    path_str.into_owned(),
                ));
                valid.push(discovered);
            }
            StrictSkillOutcome::Quarantined(row) => quarantined.push(row),
        }
    }

    SkillSourceReport {
        inventory: SkillInventory::new(generation, valid, quarantined),
        skills,
        commands,
    }
}

fn ingest_skill_path(path: &Path, scope: SkillScope) -> StrictSkillOutcome {
    let identity_name = path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if !is_skill_md_path(path) {
        return validate_strict_skill_dir(path.parent().unwrap_or(path), Some(scope));
    }
    let Some(parent) = path.parent() else {
        return StrictSkillOutcome::Quarantined(super::inventory::QuarantinedSkill {
            identity: SkillIdentity::new(identity_name, Some(scope)),
            diagnostics: vec![super::diagnostic::SkillDiagnostic::new(
                super::diagnostic::SkillDiagnosticCode::MissingSkillMd,
                None,
                "Missing required file: SKILL.md.",
                "Add a SKILL.md file with YAML frontmatter to this directory.",
                super::diagnostic::DiagnosticPosition::FILE_START,
            )],
        });
    };
    validate_strict_skill_dir(parent, Some(scope))
}

/// Parse one flat command markdown file. Symlinks and non-regular files are
/// skipped (they cannot bypass skill gates by posing as commands).
pub fn parse_legacy_command_file(path: &Path, scope: SkillScope) -> Option<LegacyCommand> {
    if !is_legacy_command_path(path) || !is_regular_file(path) {
        return None;
    }
    let stem = path.file_stem().and_then(|name| name.to_str())?;
    let content = std::fs::read_to_string(path).ok()?;
    let parsed = match parse_skill_frontmatter(&content, Some(stem)) {
        Ok(parsed) => parsed,
        Err(SkillParseError::NoFrontmatter) => {
            let body = extract_skill_body(&content);
            let description = extract_first_paragraph(&body)
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| stem.to_string());
            return Some(LegacyCommand {
                name: stem.to_string(),
                description,
                path: path.to_string_lossy().into_owned(),
                scope,
                argument_hint: None,
                plugin_name: None,
                plugin_version: None,
                plugin_root: None,
                plugin_data: None,
            });
        }
        Err(_) => {
            return Some(LegacyCommand {
                name: stem.to_string(),
                description: stem.to_string(),
                path: path.to_string_lossy().into_owned(),
                scope,
                argument_hint: None,
                plugin_name: None,
                plugin_version: None,
                plugin_root: None,
                plugin_data: None,
            });
        }
    };
    Some(LegacyCommand {
        name: if parsed.name.is_empty() {
            stem.to_string()
        } else {
            parsed.name
        },
        description: if parsed.description.is_empty() {
            stem.to_string()
        } else {
            parsed.description
        },
        path: path.to_string_lossy().into_owned(),
        scope,
        argument_hint: parsed.argument_hint,
        plugin_name: None,
        plugin_version: None,
        plugin_root: None,
        plugin_data: None,
    })
}

/// Revalidate a previously discovered skill at load time: regular file, no
/// follow, strict contract, identity, and a post-read TOCTOU identity check.
///
/// The SKILL.md bytes are read from an `O_NOFOLLOW` fd (Unix `openat` walk
/// of every original component from a trusted config / plugin / skills
/// parent). Callers that need the body must use
/// [`revalidate_skill_file_at_load`] so they never re-open the path.
pub fn revalidate_skill_at_load(skill: &SkillInfo) -> Result<DiscoveredSkill, SkillLoadError> {
    Ok(revalidate_skill_file_at_load(skill)?.discovered)
}

/// Revalidate and return the SKILL.md bytes captured from the no-follow fd.
pub fn revalidate_skill_file_at_load(
    skill: &SkillInfo,
) -> Result<RevalidatedSkillFile, SkillLoadError> {
    let path = Path::new(&skill.path);
    if !is_skill_md_path(path) {
        return Err(SkillLoadError::NotASkill);
    }
    let parent = path.parent().ok_or(SkillLoadError::Unreadable)?;
    let plugin_root = skill.plugin_root.as_ref().map(Path::new);
    let collection_root = skill.collection_root.as_ref().map(Path::new);
    let content = read_skill_md_nofollow(path, plugin_root, collection_root)?;
    let parent_dir_name = parent
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let outcome = validate_strict_skill(StrictSkillInput {
        file_name: SKILL_MD_FILE_NAME,
        parent_dir_name,
        content: &content,
        scope: Some(skill.scope),
    });
    match outcome {
        StrictSkillOutcome::Valid(discovered) => {
            let expected = SkillIdentity::new(parent_dir_name, Some(skill.scope));
            if discovered.identity.parent_dir_name != expected.parent_dir_name {
                return Err(SkillLoadError::IdentityChanged);
            }
            // Plugin re-key uses the directory basename as `name`; native skills
            // keep the official manifest name, which already matches the dir.
            if discovered.manifest.name != skill.name
                && skill.display_name.as_deref() != Some(discovered.manifest.name.as_str())
            {
                return Err(SkillLoadError::IdentityChanged);
            }
            Ok(RevalidatedSkillFile {
                discovered,
                content,
            })
        }
        StrictSkillOutcome::Quarantined(_) => Err(SkillLoadError::Quarantined),
    }
}

/// Stamp the directory that was searched at discovery onto every skill
/// whose path is contained in `root`. Load-time revalidation `openat`-walks
/// from `/` through this root with `O_NOFOLLOW`.
pub fn stamp_collection_root(skills: &mut [SkillInfo], root: &Path) {
    let root_str = root.to_string_lossy().into_owned();
    for skill in skills {
        if Path::new(&skill.path).starts_with(root) {
            skill.collection_root = Some(root_str.clone());
        }
    }
}

/// Read SKILL.md through a no-follow open. On Unix this is an `openat` walk
/// of every original absolute component of the trusted root from `/` (or
/// `/private/{var,tmp,etc}` when that prefix is a platform alias), then
/// every original relative component from that root to SKILL.md. POSIX
/// `lstat`/`O_NOFOLLOW` apply only to the last component, so a path-open of
/// `walk_root` would follow a swapped ancestor. Last-component ancestor
/// lstat is an early reject, not the no-follow walk. The leaf is opened
/// with `O_NOFOLLOW`.
pub(crate) fn read_skill_md_nofollow(
    path: &Path,
    plugin_root: Option<&Path>,
    collection_root: Option<&Path>,
) -> Result<String, SkillLoadError> {
    if !is_skill_md_path(path) {
        return Err(SkillLoadError::NotASkill);
    }
    let path = absolute_skill_path(path)?;
    let (walk_root, components) = plan_nofollow_walk(&path, plugin_root, collection_root)?;

    // Last-component lstat of every ancestor of walk_root is an early
    // reject for an already-swapped grandparent. It does not compose a
    // no-follow walk: a later path-open of walk_root still follows a
    // prefix swapped in this window.
    lstat_walk_root_ancestors(&walk_root)?;
    lstat_required(&walk_root, true)?;
    run_before_walk_root_open_hook();
    let mut cur = walk_root.clone();
    let last = components.len() - 1;
    let mut meta_before = None;
    for (i, name) in components.iter().enumerate() {
        cur.push(name);
        let meta = lstat_required(&cur, i != last)?;
        if i == last {
            meta_before = Some(meta);
        }
    }
    let meta_before = meta_before.ok_or(SkillLoadError::Unreadable)?;

    let (fd_meta, content) = read_skill_md_bytes(&walk_root, &components)?;
    if file_identity_changed(&meta_before, &fd_meta) {
        return Err(SkillLoadError::IdentityChanged);
    }

    run_after_nofollow_read_hook();

    let meta_after = lstat_required(&path, false)?;
    if file_identity_changed(&fd_meta, &meta_after) {
        return Err(SkillLoadError::IdentityChanged);
    }
    Ok(content)
}

/// Trusted directory from which every original SKILL.md component is walked
/// with `openat` + `O_NOFOLLOW` after an `openat` walk of `walk_root` itself
/// from `/`. Never derived as `parent.parent()` of a nested SKILL.md (that
/// would leave the real collection root as a followed intermediate). If no
/// plugin root, stamped collection root, or `skills/` parent can be proven,
/// the load fails closed.
fn plan_nofollow_walk(
    path: &Path,
    plugin_root: Option<&Path>,
    collection_root: Option<&Path>,
) -> Result<(PathBuf, Vec<OsString>), SkillLoadError> {
    if let Some(plugin_root) = plugin_root {
        if let Some(plan) = try_plan_from_root(path, plugin_root)? {
            return Ok(plan);
        }
    }
    if let Some(collection_root) = collection_root {
        return try_plan_from_root(path, collection_root)?.ok_or(SkillLoadError::Unreadable);
    }
    if let Some(config_root) = parent_of_skills_component(path) {
        return try_plan_from_root(path, &config_root)?.ok_or(SkillLoadError::Unreadable);
    }
    Err(SkillLoadError::Unreadable)
}

fn try_plan_from_root(
    path: &Path,
    root: &Path,
) -> Result<Option<(PathBuf, Vec<OsString>)>, SkillLoadError> {
    let root = absolute_skill_path(root)?;
    if !path.starts_with(&root) {
        return Ok(None);
    }
    let Ok(rel) = path.strip_prefix(&root) else {
        return Ok(None);
    };
    let components = match relative_normal_components(rel) {
        Ok(components) => components,
        Err(_) => return Ok(None),
    };
    if components.is_empty() || components.len() > MAX_NOFOLLOW_WALK_COMPONENTS {
        return Err(SkillLoadError::Unreadable);
    }
    Ok(Some((root, components)))
}

fn lstat_walk_root_ancestors(walk_root: &Path) -> Result<(), SkillLoadError> {
    let mut cur = walk_root.to_path_buf();
    let mut remaining = MAX_NOFOLLOW_WALK_COMPONENTS;
    loop {
        let Some(parent) = cur.parent() else {
            return Ok(());
        };
        if parent.as_os_str().is_empty() || parent == cur.as_path() {
            return Ok(());
        }
        if remaining == 0 {
            return Err(SkillLoadError::Unreadable);
        }
        remaining -= 1;
        match lstat_required(parent, true) {
            Ok(_) => {}
            Err(SkillLoadError::Symlink) if is_platform_prefix_alias(parent) => {
                // macOS `/var` -> `/private/var` (and `/tmp`, `/etc`) must not
                // fail-closed for every tempfile or user path under those
                // aliases. Any other ancestor symlink is a redirected walk.
            }
            Err(err) => return Err(err),
        }
        cur = parent.to_path_buf();
    }
}

/// True when `path` is a root-level platform alias such as macOS
/// `/var` -> `private/var` or `/private/var`. These exist independently of
/// skill discovery and are not a post-discovery collection-root swap.
fn is_platform_prefix_alias(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    if parent.parent().is_some() || !parent.has_root() {
        return false;
    }
    let Some(name) = path.file_name() else {
        return false;
    };
    let Ok(target) = std::fs::read_link(path) else {
        return false;
    };
    target == Path::new("/private").join(name) || target == Path::new("private").join(name)
}

fn parent_of_skills_component(path: &Path) -> Option<PathBuf> {
    let mut prefix = PathBuf::new();
    for component in path.components() {
        if component.as_os_str() == "skills" {
            if prefix.as_os_str().is_empty() {
                return None;
            }
            return Some(prefix);
        }
        prefix.push(component);
    }
    None
}

fn relative_normal_components(rel: &Path) -> Result<Vec<OsString>, SkillLoadError> {
    let mut out = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(name) => {
                if name.is_empty() || name == "." || name == ".." {
                    return Err(SkillLoadError::Unreadable);
                }
                out.push(name.to_os_string());
            }
            Component::CurDir => {}
            _ => return Err(SkillLoadError::Unreadable),
        }
    }
    Ok(out)
}

#[cfg(unix)]
fn absolute_normal_components(path: &Path) -> Result<Vec<OsString>, SkillLoadError> {
    if !path.is_absolute() {
        return Err(SkillLoadError::Unreadable);
    }
    let mut out = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                if name.is_empty() || name == "." || name == ".." {
                    return Err(SkillLoadError::Unreadable);
                }
                out.push(name.to_os_string());
            }
            Component::CurDir => {}
            _ => return Err(SkillLoadError::Unreadable),
        }
    }
    if out.is_empty() {
        return Err(SkillLoadError::Unreadable);
    }
    Ok(out)
}

fn absolute_skill_path(path: &Path) -> Result<PathBuf, SkillLoadError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let cwd = std::env::current_dir().map_err(|_| SkillLoadError::Unreadable)?;
    Ok(cwd.join(path))
}

fn lstat_required(path: &Path, want_dir: bool) -> Result<std::fs::Metadata, SkillLoadError> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(SkillLoadError::NotFound);
        }
        Err(_) => return Err(SkillLoadError::Unreadable),
    };
    if meta.file_type().is_symlink() {
        return Err(SkillLoadError::Symlink);
    }
    if want_dir {
        if !meta.is_dir() {
            return Err(SkillLoadError::NotRegularFile);
        }
    } else if !meta.is_file() {
        return Err(SkillLoadError::NotRegularFile);
    }
    Ok(meta)
}

fn utf8_prefix_trim(buf: &mut Vec<u8>, max: usize) {
    if buf.len() <= max {
        return;
    }
    let mut end = max;
    while end > 0 && (buf[end] & 0xC0) == 0x80 {
        end -= 1;
    }
    buf.truncate(end);
}

#[cfg(unix)]
fn map_open_errno(err: nix::errno::Errno) -> SkillLoadError {
    match err {
        nix::errno::Errno::ELOOP => SkillLoadError::Symlink,
        nix::errno::Errno::ENOENT => SkillLoadError::NotFound,
        nix::errno::Errno::ENOTDIR => SkillLoadError::NotRegularFile,
        _ => SkillLoadError::Unreadable,
    }
}

/// macOS `openat(..., O_DIRECTORY|O_NOFOLLOW)` on a symlink can return
/// `ENOTDIR` instead of `ELOOP` because the symlink vnode is not a
/// directory. Confirm with `fstatat(..., AT_SYMLINK_NOFOLLOW)`.
#[cfg(unix)]
fn openat_directory_nofollow(
    dir: &std::os::fd::OwnedFd,
    name: &[u8],
    flags: nix::fcntl::OFlag,
    mode: nix::sys::stat::Mode,
) -> Result<std::os::fd::OwnedFd, SkillLoadError> {
    use nix::fcntl::{AtFlags, openat};
    use nix::sys::stat::{SFlag, fstatat};

    match openat(dir, name, flags, mode) {
        Ok(fd) => Ok(fd),
        Err(err) => {
            if err == nix::errno::Errno::ELOOP || err == nix::errno::Errno::ENOTDIR {
                if let Ok(stat) = fstatat(dir, name, AtFlags::AT_SYMLINK_NOFOLLOW) {
                    if SFlag::from_bits_truncate(stat.st_mode) & SFlag::S_IFMT == SFlag::S_IFLNK {
                        return Err(SkillLoadError::Symlink);
                    }
                }
            }
            Err(map_open_errno(err))
        }
    }
}

/// Open `walk_root` by walking every absolute component from `/` with
/// `O_DIRECTORY|O_NOFOLLOW`. A path-open of `walk_root` would follow a
/// swapped ancestor because POSIX `O_NOFOLLOW` applies only to the last
/// component. macOS `/var` (and `/tmp`, `/etc`) are skipped in favor of
/// `/private/{var,tmp,etc}` so the platform alias itself is not a
/// fail-closed symlink.
#[cfg(unix)]
fn open_directory_nofollow_from_root(
    walk_root: &Path,
) -> Result<std::os::fd::OwnedFd, SkillLoadError> {
    use std::os::unix::ffi::OsStrExt;

    use nix::fcntl::{OFlag, open};
    use nix::sys::stat::Mode;

    let walk_root = absolute_skill_path(walk_root)?;
    let parts = absolute_normal_components(&walk_root)?;
    if parts.len() > MAX_ABSOLUTE_NOFOLLOW_WALK_COMPONENTS {
        return Err(SkillLoadError::Unreadable);
    }

    let root_flags = OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC;
    let dir_flags = OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC;
    let mut dir: std::os::fd::OwnedFd =
        open("/", root_flags, Mode::empty()).map_err(map_open_errno)?;
    let mut start = 0usize;
    if let Some(first) = parts.first() {
        let prefix = Path::new("/").join(first);
        if is_platform_prefix_alias(&prefix) {
            dir = openat_directory_nofollow(&dir, b"private".as_slice(), dir_flags, Mode::empty())?;
            dir = openat_directory_nofollow(
                &dir,
                first.as_os_str().as_bytes(),
                dir_flags,
                Mode::empty(),
            )?;
            start = 1;
        }
    }
    for name in &parts[start..] {
        dir =
            openat_directory_nofollow(&dir, name.as_os_str().as_bytes(), dir_flags, Mode::empty())?;
    }
    Ok(dir)
}

#[cfg(unix)]
fn read_skill_md_bytes(
    walk_root: &Path,
    components: &[OsString],
) -> Result<(std::fs::Metadata, String), SkillLoadError> {
    use std::io::Read;
    use std::os::fd::OwnedFd;
    use std::os::unix::ffi::OsStrExt;

    use nix::fcntl::{OFlag, openat};
    use nix::sys::stat::Mode;

    if components.is_empty() {
        return Err(SkillLoadError::Unreadable);
    }
    let last = components.len() - 1;
    if components[last].as_os_str() != SKILL_MD_FILE_NAME {
        return Err(SkillLoadError::NotASkill);
    }

    let mut dir: OwnedFd = open_directory_nofollow_from_root(walk_root)?;
    let mut file_fd: Option<OwnedFd> = None;
    for (i, name) in components.iter().enumerate() {
        if i == last {
            file_fd = Some(
                openat(
                    &dir,
                    name.as_os_str().as_bytes(),
                    OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
                    Mode::empty(),
                )
                .map_err(map_open_errno)?,
            );
            break;
        }
        dir = openat_directory_nofollow(
            &dir,
            name.as_os_str().as_bytes(),
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )?;
    }
    let Some(file_fd) = file_fd else {
        return Err(SkillLoadError::Unreadable);
    };

    let mut file = std::fs::File::from(file_fd);
    let fd_meta = file.metadata().map_err(|_| SkillLoadError::Unreadable)?;
    if !fd_meta.is_file() {
        return Err(SkillLoadError::NotRegularFile);
    }

    let mut buf = Vec::new();
    let mut take = file.by_ref().take(MAX_SKILL_MD_BYTES.saturating_add(1));
    take.read_to_end(&mut buf)
        .map_err(|_| SkillLoadError::Unreadable)?;
    utf8_prefix_trim(&mut buf, MAX_SKILL_MD_BYTES as usize);
    let content = String::from_utf8(buf).map_err(|_| SkillLoadError::Unreadable)?;
    Ok((fd_meta, content))
}

#[cfg(not(unix))]
fn read_skill_md_bytes(
    walk_root: &Path,
    components: &[OsString],
) -> Result<(std::fs::Metadata, String), SkillLoadError> {
    use std::io::Read;
    let mut path = walk_root.to_path_buf();
    for name in components {
        path.push(name);
    }
    let mut file = std::fs::File::open(&path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            SkillLoadError::NotFound
        } else {
            SkillLoadError::Unreadable
        }
    })?;
    let fd_meta = file.metadata().map_err(|_| SkillLoadError::Unreadable)?;
    if !fd_meta.is_file() {
        return Err(SkillLoadError::NotRegularFile);
    }
    let mut buf = Vec::new();
    let mut take = file.by_ref().take(MAX_SKILL_MD_BYTES.saturating_add(1));
    take.read_to_end(&mut buf)
        .map_err(|_| SkillLoadError::Unreadable)?;
    utf8_prefix_trim(&mut buf, MAX_SKILL_MD_BYTES as usize);
    let content = String::from_utf8(buf).map_err(|_| SkillLoadError::Unreadable)?;
    Ok((fd_meta, content))
}

fn file_identity_changed(before: &std::fs::Metadata, after: &std::fs::Metadata) -> bool {
    if before.file_type() != after.file_type() || before.len() != after.len() {
        return true;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        before.dev() != after.dev() || before.ino() != after.ino()
    }
    #[cfg(not(unix))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::skills::strict::spec::STRICT_VALIDATOR_RUNTIME_ENABLED;

    fn official(name: &str, description: &str) -> String {
        format!("---\nname: {name}\ndescription: {description}\n---\nBody\n")
    }

    fn grok_official(name: &str, description: &str, when_to_use: &str) -> String {
        format!(
            "---\nname: {name}\ndescription: {description}\nmetadata:\n  grok:\n    when-to-use: {when_to_use}\n---\nBody\n"
        )
    }

    fn write_skill(root: &Path, name: &str, content: &str) -> PathBuf {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(SKILL_MD_FILE_NAME);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn runtime_flag_is_enabled() {
        assert!(STRICT_VALIDATOR_RUNTIME_ENABLED);
    }

    #[test]
    fn ingest_exposes_invalid_only_through_inventory() {
        let tmp = tempfile::tempdir().unwrap();
        let good = write_skill(
            tmp.path(),
            "commit",
            &grok_official("commit", "Create commits", "commit changes"),
        );
        let bad = write_skill(
            tmp.path(),
            "broken",
            "---\nname: broken\nwhen-to-use: secret-token\n---\n",
        );
        let report =
            ingest_skill_sources(vec![(good, SkillScope::Local), (bad, SkillScope::Local)], 3);
        assert_eq!(report.skills.len(), 1);
        assert_eq!(report.skills[0].name, "commit");
        assert_eq!(
            report.skills[0].when_to_use.as_deref(),
            Some("commit changes")
        );
        assert_eq!(report.inventory.valid.len(), 1);
        assert_eq!(report.inventory.quarantined.len(), 1);
        assert_eq!(report.inventory.generation, 3);
        let dump = format!("{:?}", report.inventory);
        assert!(!dump.contains("secret-token"));
        assert!(!report.skills.iter().any(|s| s.name == "broken"));
    }

    #[test]
    fn commands_do_not_enter_skill_inventory() {
        let tmp = tempfile::tempdir().unwrap();
        let commands = tmp.path().join("commands");
        std::fs::create_dir_all(&commands).unwrap();
        let cmd = commands.join("frontend.md");
        std::fs::write(
            &cmd,
            "---\nname: frontend\ndescription: UI kit\n---\nUse it.\n",
        )
        .unwrap();
        let skill = write_skill(
            tmp.path().join("skills").as_path(),
            "commit",
            &official("commit", "Create commits"),
        );
        let report =
            ingest_skill_sources(vec![(cmd, SkillScope::Repo), (skill, SkillScope::Repo)], 1);
        assert_eq!(report.skills.len(), 1);
        assert_eq!(report.skills[0].name, "commit");
        assert_eq!(report.commands.len(), 1);
        assert_eq!(report.commands[0].name, "frontend");
        assert!(report.inventory.quarantined.is_empty());
        let slash = report.commands[0].to_slash_skill();
        assert!(slash.disable_model_invocation);
        assert!(!slash.is_native_model_invocable());
        assert!(slash.user_invocable);
    }

    #[test]
    fn symlink_skill_is_quarantined_and_not_loadable() {
        let tmp = tempfile::tempdir().unwrap();
        let real = write_skill(tmp.path(), "commit", &official("commit", "Create commits"));
        let link_dir = tmp.path().join("skills").join("linked");
        std::fs::create_dir_all(&link_dir).unwrap();
        let link = link_dir.join(SKILL_MD_FILE_NAME);
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let report = ingest_skill_sources(vec![(link.clone(), SkillScope::Local)], 1);
        assert!(report.skills.is_empty());
        assert_eq!(report.inventory.quarantined.len(), 1);
        let fake = SkillInfo {
            name: "linked".into(),
            path: link.to_string_lossy().into_owned(),
            scope: SkillScope::Local,
            ..SkillInfo::default()
        };
        assert_eq!(
            revalidate_skill_at_load(&fake),
            Err(SkillLoadError::Symlink)
        );
    }

    #[test]
    fn symlink_directory_is_not_walked_as_real() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        write_skill(
            outside.path(),
            "escape",
            &official("escape", "Should not load"),
        );
        let skills = tmp.path().join("skills");
        std::fs::create_dir_all(&skills).unwrap();
        std::os::unix::fs::symlink(outside.path(), skills.join("escape")).unwrap();
        assert!(!is_real_directory(&skills.join("escape")));
    }

    #[test]
    fn edit_after_discovery_fails_revalidation() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_nested_skill(tmp.path(), "commit", &official("commit", "Create commits"));
        let report = ingest_skill_sources(vec![(path.clone(), SkillScope::Local)], 1);
        assert_eq!(report.skills.len(), 1);
        std::fs::write(&path, "---\nname: commit\n---\nnow invalid\n").unwrap();
        assert_eq!(
            revalidate_skill_at_load(&report.skills[0]),
            Err(SkillLoadError::Quarantined)
        );
    }

    #[test]
    fn source_collisions_keep_both_valid_inventory_rows() {
        let tmp = tempfile::tempdir().unwrap();
        let local = write_skill(
            tmp.path().join("local").as_path(),
            "commit",
            &official("commit", "Local commits"),
        );
        let user = write_skill(
            tmp.path().join("user").as_path(),
            "commit",
            &official("commit", "User commits"),
        );
        let report = ingest_skill_sources(
            vec![(local, SkillScope::Local), (user, SkillScope::User)],
            2,
        );
        assert_eq!(report.skills.len(), 2);
        assert_eq!(report.inventory.valid.len(), 2);
        assert!(report.inventory.quarantined.is_empty());
    }

    #[test]
    fn qualified_toggle_matches_scope_and_bare_name() {
        let skill = SkillInfo {
            name: "commit".into(),
            scope: SkillScope::Local,
            ..SkillInfo::default()
        };
        let mut disabled = HashSet::new();
        disabled.insert("local:commit");
        assert!(skill_matches_toggle(&skill, &disabled));
        disabled.clear();
        disabled.insert("commit");
        assert!(skill_matches_toggle(&skill, &disabled));
        disabled.clear();
        disabled.insert("user:commit");
        assert!(!skill_matches_toggle(&skill, &disabled));
    }

    #[test]
    fn command_symlink_cannot_bypass_skill_gates() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real.md");
        std::fs::write(&real, "---\nname: leak\ndescription: secret\n---\n").unwrap();
        let link = tmp.path().join("commands");
        std::fs::create_dir_all(&link).unwrap();
        let cmd = link.join("leak.md");
        std::os::unix::fs::symlink(&real, &cmd).unwrap();
        assert!(parse_legacy_command_file(&cmd, SkillScope::Local).is_none());
        let report = ingest_skill_sources(vec![(cmd, SkillScope::Local)], 1);
        assert!(report.skills.is_empty());
        assert!(report.commands.is_empty());
        assert!(report.inventory.valid.is_empty());
    }

    #[test]
    fn load_rejects_non_skill_md_as_not_a_skill() {
        let tmp = tempfile::tempdir().unwrap();
        let cmd = tmp.path().join("frontend.md");
        std::fs::write(&cmd, "body").unwrap();
        let skill = SkillInfo {
            name: "frontend".into(),
            path: cmd.to_string_lossy().into_owned(),
            ..SkillInfo::default()
        };
        assert_eq!(
            revalidate_skill_at_load(&skill),
            Err(SkillLoadError::NotASkill)
        );
    }

    #[test]
    fn parent_directory_symlink_fails_load() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        write_skill(
            outside.path(),
            "escape",
            &official("escape", "Should not load"),
        );
        let skills = tmp.path().join("skills");
        std::fs::create_dir_all(&skills).unwrap();
        let link = skills.join("escape");
        std::os::unix::fs::symlink(outside.path().join("escape"), &link).unwrap();
        let path = link.join(SKILL_MD_FILE_NAME);
        let fake = SkillInfo {
            name: "escape".into(),
            path: path.to_string_lossy().into_owned(),
            scope: SkillScope::Local,
            ..SkillInfo::default()
        };
        assert_eq!(
            revalidate_skill_at_load(&fake),
            Err(SkillLoadError::Symlink)
        );
    }

    fn write_nested_skill(root: &Path, name: &str, content: &str) -> PathBuf {
        write_skill(&root.join("skills"), name, content)
    }

    fn write_deep_nested_skill(root: &Path, content: &str) -> PathBuf {
        write_skill(&root.join("skills").join("team"), "infra", content)
    }

    #[cfg(unix)]
    #[test]
    fn ancestor_skills_dir_symlink_after_discovery_fails_load() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_nested_skill(tmp.path(), "commit", &official("commit", "Create commits"));
        let report = ingest_skill_sources(vec![(path.clone(), SkillScope::Local)], 1);
        assert_eq!(report.skills.len(), 1);

        let skills_dir = tmp.path().join("skills");
        let real = tmp.path().join("skills.real");
        std::fs::rename(&skills_dir, &real).unwrap();
        let evil_root = tempfile::tempdir().unwrap();
        write_skill(
            evil_root.path(),
            "commit",
            &official("commit", "EVIL_SECRET_BODY"),
        );
        std::os::unix::fs::symlink(evil_root.path(), &skills_dir).unwrap();

        assert_eq!(
            revalidate_skill_at_load(&report.skills[0]),
            Err(SkillLoadError::Symlink)
        );
        let following = std::fs::read_to_string(&path).expect("path-follow still sees the target");
        assert!(
            following.contains("EVIL_SECRET_BODY"),
            "control: a following read would inject the swapped ancestor"
        );
    }

    #[cfg(unix)]
    #[test]
    fn leaf_symlink_replace_fails_nofollow_read() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_nested_skill(tmp.path(), "commit", &official("commit", "Create commits"));
        let report = ingest_skill_sources(vec![(path.clone(), SkillScope::Local)], 1);
        assert_eq!(report.skills.len(), 1);
        assert!(revalidate_skill_at_load(&report.skills[0]).is_ok());

        let evil = tmp.path().join("evil.md");
        std::fs::write(&evil, official("commit", "EVIL_SECRET_BODY")).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&evil, &path).unwrap();

        assert_eq!(
            revalidate_skill_at_load(&report.skills[0]),
            Err(SkillLoadError::Symlink)
        );
        let following = std::fs::read_to_string(&path).expect("path-follow still sees the target");
        assert!(following.contains("EVIL_SECRET_BODY"));
    }

    #[cfg(unix)]
    #[test]
    fn leaf_symlink_swap_after_nofollow_read_fails_identity_check() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_nested_skill(tmp.path(), "commit", &official("commit", "Create commits"));
        let report = ingest_skill_sources(vec![(path.clone(), SkillScope::Local)], 1);
        let evil = tmp.path().join("evil.md");
        std::fs::write(&evil, official("commit", "EVIL_SECRET_BODY")).unwrap();
        let swap_path = path.clone();
        set_after_nofollow_read_hook(move || {
            std::fs::remove_file(&swap_path).unwrap();
            std::os::unix::fs::symlink(&evil, &swap_path).unwrap();
        });
        assert_eq!(
            revalidate_skill_file_at_load(&report.skills[0]).map(|_| ()),
            Err(SkillLoadError::Symlink)
        );
    }

    #[cfg(unix)]
    #[test]
    fn nested_skill_md_revalidates_from_config_root() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_deep_nested_skill(tmp.path(), &official("infra", "Create commits"));
        let report = ingest_skill_sources(vec![(path.clone(), SkillScope::Local)], 1);
        assert_eq!(report.skills.len(), 1);
        let loaded = revalidate_skill_file_at_load(&report.skills[0]).expect("nested skill loads");
        assert!(loaded.content.contains("Create commits"));
        assert_eq!(loaded.discovered.manifest.name, "infra");
    }

    #[cfg(unix)]
    #[test]
    fn nested_ancestor_skills_dir_symlink_after_discovery_fails_load() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_deep_nested_skill(tmp.path(), &official("infra", "Create commits"));
        let report = ingest_skill_sources(vec![(path.clone(), SkillScope::Local)], 1);
        assert_eq!(report.skills.len(), 1);

        let skills_dir = tmp.path().join("skills");
        let real = tmp.path().join("skills.real");
        std::fs::rename(&skills_dir, &real).unwrap();
        let evil_root = tempfile::tempdir().unwrap();
        write_skill(
            &evil_root.path().join("team"),
            "infra",
            &official("infra", "EVIL_SECRET_BODY"),
        );
        std::os::unix::fs::symlink(evil_root.path(), &skills_dir).unwrap();

        assert_eq!(
            revalidate_skill_at_load(&report.skills[0]),
            Err(SkillLoadError::Symlink)
        );
        let following = std::fs::read_to_string(&path).expect("path-follow still sees the target");
        assert!(
            following.contains("EVIL_SECRET_BODY"),
            "control: a following read would inject the swapped nested ancestor"
        );
    }

    #[cfg(unix)]
    #[test]
    fn parent_of_skills_dir_symlink_after_discovery_fails_load() {
        let tmp = tempfile::tempdir().unwrap();
        let grok = tmp.path().join(".grok");
        let path = write_nested_skill(&grok, "commit", &official("commit", "Create commits"));
        let report = ingest_skill_sources(vec![(path.clone(), SkillScope::Local)], 1);
        assert_eq!(report.skills.len(), 1);

        let real = tmp.path().join(".grok.real");
        std::fs::rename(&grok, &real).unwrap();
        let evil_root = tempfile::tempdir().unwrap();
        write_nested_skill(
            evil_root.path(),
            "commit",
            &official("commit", "EVIL_SECRET_BODY"),
        );
        std::os::unix::fs::symlink(evil_root.path(), &grok).unwrap();

        assert_eq!(
            revalidate_skill_at_load(&report.skills[0]),
            Err(SkillLoadError::Symlink)
        );
        let following = std::fs::read_to_string(&path).expect("path-follow still sees the target");
        assert!(
            following.contains("EVIL_SECRET_BODY"),
            "control: a following read would inject the swapped parent of skills/"
        );
    }

    #[cfg(unix)]
    #[test]
    fn project_dir_symlink_after_discovery_fails_load() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path().join("proj");
        std::fs::create_dir_all(&proj).unwrap();
        let grok = proj.join(".grok");
        let path = write_nested_skill(&grok, "commit", &official("commit", "Create commits"));
        let report = ingest_skill_sources(vec![(path.clone(), SkillScope::Local)], 1);
        assert_eq!(report.skills.len(), 1);

        let real = tmp.path().join("proj.real");
        std::fs::rename(&proj, &real).unwrap();
        let evil_root = tempfile::tempdir().unwrap();
        write_nested_skill(
            &evil_root.path().join(".grok"),
            "commit",
            &official("commit", "EVIL_SECRET_BODY"),
        );
        std::os::unix::fs::symlink(evil_root.path(), &proj).unwrap();

        assert_eq!(
            revalidate_skill_at_load(&report.skills[0]),
            Err(SkillLoadError::Symlink)
        );
        let following = std::fs::read_to_string(&path).expect("path-follow still sees the target");
        assert!(
            following.contains("EVIL_SECRET_BODY"),
            "control: a following read would inject the swapped project directory"
        );
    }

    #[cfg(unix)]
    fn write_nested_collection_skill(root: &Path, content: &str) -> PathBuf {
        write_skill(&root.join("team"), "infra", content)
    }

    #[cfg(unix)]
    #[test]
    fn nested_collection_without_trusted_root_is_unreadable() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_nested_collection_skill(tmp.path(), &official("infra", "Create commits"));
        let report = ingest_skill_sources(vec![(path, SkillScope::Local)], 1);
        assert_eq!(report.skills.len(), 1);
        assert!(report.skills[0].collection_root.is_none());
        assert_eq!(
            revalidate_skill_at_load(&report.skills[0]),
            Err(SkillLoadError::Unreadable)
        );
    }

    #[cfg(unix)]
    #[test]
    fn nested_collection_root_symlink_after_discovery_fails_load() {
        let tmp = tempfile::tempdir().unwrap();
        let pack = tmp.path().join("pack");
        let path = write_nested_collection_skill(&pack, &official("infra", "Create commits"));
        let mut report = ingest_skill_sources(vec![(path.clone(), SkillScope::Local)], 1);
        assert_eq!(report.skills.len(), 1);
        stamp_collection_root(&mut report.skills, &pack);
        assert_eq!(
            report.skills[0].collection_root.as_deref(),
            Some(pack.to_str().unwrap())
        );
        assert!(revalidate_skill_at_load(&report.skills[0]).is_ok());

        let real = tmp.path().join("pack.real");
        std::fs::rename(&pack, &real).unwrap();
        let evil_root = tempfile::tempdir().unwrap();
        write_nested_collection_skill(evil_root.path(), &official("infra", "EVIL_SECRET_BODY"));
        std::os::unix::fs::symlink(evil_root.path(), &pack).unwrap();

        assert_eq!(
            revalidate_skill_at_load(&report.skills[0]),
            Err(SkillLoadError::Symlink)
        );
        let following = std::fs::read_to_string(&path).expect("path-follow still sees the target");
        assert!(
            following.contains("EVIL_SECRET_BODY"),
            "control: a following read would inject the swapped collection root"
        );
    }

    #[cfg(unix)]
    #[test]
    fn stamped_skill_dir_grandparent_symlink_after_discovery_fails_load() {
        let tmp = tempfile::tempdir().unwrap();
        let pack = tmp.path().join("pack");
        let path = write_nested_collection_skill(&pack, &official("infra", "Create commits"));
        let skill_dir = path.parent().expect("SKILL.md parent").to_path_buf();
        let mut report = ingest_skill_sources(vec![(path.clone(), SkillScope::Local)], 1);
        assert_eq!(report.skills.len(), 1);
        stamp_collection_root(&mut report.skills, &skill_dir);
        assert_eq!(
            report.skills[0].collection_root.as_deref(),
            Some(skill_dir.to_str().unwrap())
        );
        assert!(revalidate_skill_at_load(&report.skills[0]).is_ok());

        let real = tmp.path().join("pack.real");
        std::fs::rename(&pack, &real).unwrap();
        let evil_root = tempfile::tempdir().unwrap();
        write_nested_collection_skill(evil_root.path(), &official("infra", "EVIL_SECRET_BODY"));
        std::os::unix::fs::symlink(evil_root.path(), &pack).unwrap();

        assert_eq!(
            revalidate_skill_at_load(&report.skills[0]),
            Err(SkillLoadError::Symlink)
        );
        let following = std::fs::read_to_string(&path).expect("path-follow still sees the target");
        assert!(
            following.contains("EVIL_SECRET_BODY"),
            "control: a following read would inject the swapped grandparent of the stamped skill dir"
        );
    }

    #[cfg(unix)]
    #[test]
    fn stamped_skill_md_file_grandparent_symlink_after_discovery_fails_load() {
        let tmp = tempfile::tempdir().unwrap();
        let pack = tmp.path().join("pack");
        let path = write_nested_collection_skill(&pack, &official("infra", "Create commits"));
        let skill_dir = path.parent().expect("SKILL.md parent").to_path_buf();
        let mut report = ingest_skill_sources(vec![(path.clone(), SkillScope::Local)], 1);
        assert_eq!(report.skills.len(), 1);
        // Config `[skills].paths = [pack/team/infra/SKILL.md]` stamps the
        // skill directory (parent of the file), not `pack`.
        stamp_collection_root(&mut report.skills, &skill_dir);
        assert!(revalidate_skill_at_load(&report.skills[0]).is_ok());

        let real = tmp.path().join("pack.real");
        std::fs::rename(&pack, &real).unwrap();
        let evil_root = tempfile::tempdir().unwrap();
        write_nested_collection_skill(evil_root.path(), &official("infra", "EVIL_SECRET_BODY"));
        std::os::unix::fs::symlink(evil_root.path(), &pack).unwrap();

        assert_eq!(
            revalidate_skill_file_at_load(&report.skills[0]).map(|file| {
                assert!(
                    !file.content.contains("EVIL_SECRET_BODY"),
                    "swapped grandparent must not yield the evil body"
                );
            }),
            Err(SkillLoadError::Symlink)
        );
    }

    #[cfg(unix)]
    #[test]
    fn stamped_skill_dir_grandparent_symlink_between_ancestor_lstat_and_open_fails_load() {
        let tmp = tempfile::tempdir().unwrap();
        let pack = tmp.path().join("pack");
        let path = write_nested_collection_skill(&pack, &official("infra", "Create commits"));
        let skill_dir = path.parent().expect("SKILL.md parent").to_path_buf();
        let mut report = ingest_skill_sources(vec![(path.clone(), SkillScope::Local)], 1);
        assert_eq!(report.skills.len(), 1);
        stamp_collection_root(&mut report.skills, &skill_dir);
        assert!(revalidate_skill_at_load(&report.skills[0]).is_ok());

        let real = tmp.path().join("pack.real");
        let evil_root = tempfile::tempdir().unwrap();
        write_nested_collection_skill(evil_root.path(), &official("infra", "EVIL_SECRET_BODY"));
        let pack_for_hook = pack.clone();
        let evil_target = evil_root.path().to_path_buf();
        set_before_walk_root_open_hook(move || {
            std::fs::rename(&pack_for_hook, &real).unwrap();
            std::os::unix::fs::symlink(&evil_target, &pack_for_hook).unwrap();
        });

        let loaded = revalidate_skill_file_at_load(&report.skills[0]);
        assert!(
            std::fs::symlink_metadata(&pack)
                .expect("pack still exists")
                .file_type()
                .is_symlink(),
            "hook must swap pack after ancestor lstat and before open"
        );
        match loaded {
            Ok(file) => {
                assert!(
                    !file.content.contains("EVIL_SECRET_BODY"),
                    "swapped grandparent must not yield the evil body"
                );
                assert!(
                    file.content.contains("Create commits"),
                    "only the original inode may load if the walk already held it"
                );
            }
            Err(err) => assert_eq!(err, SkillLoadError::Symlink),
        }
        let following = std::fs::read_to_string(&path).expect("path-follow still sees the target");
        assert!(
            following.contains("EVIL_SECRET_BODY"),
            "control: a following read would inject the swapped grandparent"
        );
    }
}
