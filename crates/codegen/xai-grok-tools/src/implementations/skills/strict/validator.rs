//! Canonical strict Agent Skills validator.
//!
//! Official field rules follow pinned `skills-ref` behavior. Grok extensions
//! are parsed under `metadata.grok` / `metadata.grok.*` with no coercion,
//! repair, inference, or body-derived fallback.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_yaml::Value;

use super::diagnostic::{
    DiagnosticPosition, SkillAuthoringWarning, SkillDiagnostic, SkillDiagnosticCode,
    SkillWarningCode, cap_diagnostics,
};
use super::inventory::{
    DiscoveredSkill, QuarantinedSkill, SkillIdentity, sanitize_parent_dir_name,
};
use super::manifest::{GrokSkillExtensions, StrictSkillManifest};
use super::spec::{
    GROK_EXTENSION_OBJECT_KEY, MAX_COMPATIBILITY_CHARS, MAX_DESCRIPTION_CHARS,
    MAX_FRONTMATTER_BYTES, MAX_GROK_ARGUMENT_HINT_CHARS, MAX_GROK_EFFORT_CHARS,
    MAX_GROK_MODEL_CHARS, MAX_GROK_PATH_CHARS, MAX_GROK_PATHS, MAX_GROK_SHORT_DESCRIPTION_CHARS,
    MAX_GROK_WHEN_TO_USE_CHARS, MAX_NAME_CHARS, SKILL_MD_FILE_NAME, grok_extension_leaf,
    is_known_grok_extension_leaf, is_legacy_grok_top_level_key, is_official_top_level_key, nfkc,
};

/// Inputs for in-memory strict validation. Callers supply the parent
/// directory name and file name so diagnostics never need an absolute path.
#[derive(Debug, Clone)]
pub struct StrictSkillInput<'a> {
    pub file_name: &'a str,
    pub parent_dir_name: &'a str,
    pub content: &'a str,
    pub scope: Option<crate::implementations::skills::types::SkillScope>,
}

/// Outcome of strict validation. Invalid skills are quarantined, never repaired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrictSkillOutcome {
    Valid(DiscoveredSkill),
    Quarantined(QuarantinedSkill),
}

impl StrictSkillOutcome {
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid(_))
    }

    pub fn diagnostics(&self) -> &[SkillDiagnostic] {
        match self {
            Self::Valid(_) => &[],
            Self::Quarantined(row) => &row.diagnostics,
        }
    }
}

/// Validate SKILL.md content against the pinned official contract plus
/// namespaced `metadata.grok.*` extensions.
pub fn validate_strict_skill(input: StrictSkillInput<'_>) -> StrictSkillOutcome {
    let identity = SkillIdentity::new(input.parent_dir_name, input.scope);
    let mut diagnostics = Vec::new();

    if input.file_name != SKILL_MD_FILE_NAME {
        diagnostics.push(SkillDiagnostic::new(
            SkillDiagnosticCode::WrongSkillFileName,
            None,
            "Skill file name must be SKILL.md.",
            "Rename the file to SKILL.md. Lowercase skill.md is not accepted.",
            DiagnosticPosition::FILE_START,
        ));
        return quarantined(identity, diagnostics);
    }

    if identity.parent_dir_name.is_empty() {
        diagnostics.push(SkillDiagnostic::new(
            SkillDiagnosticCode::NameDirectoryMismatch,
            Some("name"),
            "Skill parent directory name is missing.",
            "Place SKILL.md in a directory whose name matches the skill name.",
            DiagnosticPosition::FILE_START,
        ));
        return quarantined(identity, diagnostics);
    }

    let (frontmatter, body, yaml_start_line) = match split_frontmatter(input.content) {
        FrontmatterSplit::Ok {
            yaml,
            body,
            yaml_start_line,
        } => (yaml, body, yaml_start_line),
        FrontmatterSplit::Err(diag) => {
            diagnostics.push(diag);
            return quarantined(identity, diagnostics);
        }
    };

    if frontmatter.len() > MAX_FRONTMATTER_BYTES {
        diagnostics.push(SkillDiagnostic::new(
            SkillDiagnosticCode::FrontmatterTooLarge,
            None,
            "YAML frontmatter exceeds the bounded size limit.",
            "Keep frontmatter under 4096 bytes and move details to the markdown body.",
            DiagnosticPosition::new(yaml_start_line, 1),
        ));
        return quarantined(identity, diagnostics);
    }

    let mapping = match parse_frontmatter_mapping(frontmatter, yaml_start_line) {
        Ok(mapping) => mapping,
        Err(diag) => {
            diagnostics.push(diag);
            return quarantined(identity, diagnostics);
        }
    };

    let positions = FieldPositions::scan(input.content);
    collect_duplicate_top_level_keys(frontmatter, yaml_start_line, &mut diagnostics);
    collect_top_level_errors(&mapping, &positions, &mut diagnostics);

    let name = match required_string(
        &mapping,
        "name",
        SkillDiagnosticCode::MissingName,
        &positions,
    ) {
        Ok(value) => validate_name(
            value,
            &identity.parent_dir_name,
            &positions,
            &mut diagnostics,
        ),
        Err(diag) => {
            diagnostics.push(diag);
            None
        }
    };

    let description = match required_string(
        &mapping,
        "description",
        SkillDiagnosticCode::MissingDescription,
        &positions,
    ) {
        Ok(value) => validate_description(value, &positions, &mut diagnostics),
        Err(diag) => {
            diagnostics.push(diag);
            None
        }
    };

    let license = optional_string(
        &mapping,
        "license",
        SkillDiagnosticCode::LicenseNotString,
        &positions,
        &mut diagnostics,
    );
    let compatibility = optional_string(
        &mapping,
        "compatibility",
        SkillDiagnosticCode::CompatibilityNotString,
        &positions,
        &mut diagnostics,
    )
    .and_then(|value| {
        validate_compatibility(&value, &positions, &mut diagnostics).then_some(value)
    });

    let allowed_tools = optional_string(
        &mapping,
        "allowed-tools",
        SkillDiagnosticCode::AllowedToolsNotString,
        &positions,
        &mut diagnostics,
    );

    let (metadata, grok) = parse_metadata(&mapping, &positions, &mut diagnostics);

    if !diagnostics.is_empty() {
        return quarantined(identity, diagnostics);
    }

    let name = name.expect("name checked");
    let description = description.expect("description checked");
    let warnings = collect_warnings(
        &description,
        license.as_deref(),
        compatibility.as_deref(),
        allowed_tools.as_deref(),
        &grok,
        body,
        &positions,
    );

    StrictSkillOutcome::Valid(DiscoveredSkill {
        identity,
        manifest: StrictSkillManifest {
            name,
            description,
            license,
            compatibility,
            allowed_tools,
            metadata,
            grok,
        },
        warnings,
    })
}

/// Validate a skill directory. Diagnostics never include the absolute path.
/// Symlinks and non-regular files are quarantined.
pub fn validate_strict_skill_dir(
    dir: &Path,
    scope: Option<crate::implementations::skills::types::SkillScope>,
) -> StrictSkillOutcome {
    let parent_dir_name = dir.file_name().and_then(|name| name.to_str()).unwrap_or("");
    let identity = SkillIdentity::new(parent_dir_name, scope);

    let meta = match std::fs::symlink_metadata(dir) {
        Ok(meta) => meta,
        Err(_) => {
            return quarantined(
                identity,
                vec![SkillDiagnostic::new(
                    SkillDiagnosticCode::NotADirectory,
                    None,
                    "Skill path does not exist.",
                    "Provide a skill directory that contains SKILL.md.",
                    DiagnosticPosition::FILE_START,
                )],
            );
        }
    };
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return quarantined(
            identity,
            vec![SkillDiagnostic::new(
                SkillDiagnosticCode::NotADirectory,
                None,
                "Skill path is not a directory.",
                "Provide a regular directory that contains SKILL.md. Symlinks are not followed.",
                DiagnosticPosition::FILE_START,
            )],
        );
    }

    let listed_name = listed_skill_file_name(dir);
    match listed_name.as_deref() {
        Some(SKILL_MD_FILE_NAME) => {}
        Some(_) => {
            return quarantined(
                identity,
                vec![SkillDiagnostic::new(
                    SkillDiagnosticCode::WrongSkillFileName,
                    None,
                    "Skill file name must be SKILL.md.",
                    "Rename the file to SKILL.md. Lowercase skill.md is not accepted.",
                    DiagnosticPosition::FILE_START,
                )],
            );
        }
        None => {
            return quarantined(
                identity,
                vec![SkillDiagnostic::new(
                    SkillDiagnosticCode::MissingSkillMd,
                    None,
                    "Missing required file: SKILL.md.",
                    "Add a SKILL.md file with YAML frontmatter to this directory.",
                    DiagnosticPosition::FILE_START,
                )],
            );
        }
    }

    let skill_md = dir.join(SKILL_MD_FILE_NAME);
    let file_meta = match std::fs::symlink_metadata(&skill_md) {
        Ok(meta) => meta,
        Err(_) => {
            return quarantined(
                identity,
                vec![SkillDiagnostic::new(
                    SkillDiagnosticCode::MissingSkillMd,
                    None,
                    "Missing required file: SKILL.md.",
                    "Add a SKILL.md file with YAML frontmatter to this directory.",
                    DiagnosticPosition::FILE_START,
                )],
            );
        }
    };
    if file_meta.file_type().is_symlink() || !file_meta.is_file() {
        return quarantined(
            identity,
            vec![SkillDiagnostic::new(
                SkillDiagnosticCode::NotRegularFile,
                None,
                "SKILL.md must be a regular file.",
                "Replace the symlink or special file with a regular SKILL.md.",
                DiagnosticPosition::FILE_START,
            )],
        );
    }

    let content = match std::fs::read_to_string(&skill_md) {
        Ok(content) => content,
        Err(_) => {
            return quarantined(
                identity,
                vec![SkillDiagnostic::new(
                    SkillDiagnosticCode::UnreadableSkillMd,
                    None,
                    "SKILL.md could not be read.",
                    "Ensure SKILL.md is a readable UTF-8 file.",
                    DiagnosticPosition::FILE_START,
                )],
            );
        }
    };

    validate_strict_skill(StrictSkillInput {
        file_name: SKILL_MD_FILE_NAME,
        parent_dir_name,
        content: &content,
        scope,
    })
}

fn quarantined(identity: SkillIdentity, diagnostics: Vec<SkillDiagnostic>) -> StrictSkillOutcome {
    StrictSkillOutcome::Quarantined(QuarantinedSkill {
        identity,
        diagnostics: cap_diagnostics(diagnostics),
    })
}

fn listed_skill_file_name(dir: &Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut lowercase = None;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name == *SKILL_MD_FILE_NAME {
            return Some(SKILL_MD_FILE_NAME.to_string());
        }
        if name == *"skill.md" {
            lowercase = Some("skill.md".to_string());
        }
    }
    lowercase
}

enum FrontmatterSplit<'a> {
    Ok {
        yaml: &'a str,
        body: &'a str,
        yaml_start_line: u32,
    },
    Err(SkillDiagnostic),
}

fn split_frontmatter(content: &str) -> FrontmatterSplit<'_> {
    if !content.starts_with("---") {
        return FrontmatterSplit::Err(SkillDiagnostic::new(
            SkillDiagnosticCode::MissingFrontmatter,
            None,
            "SKILL.md must start with YAML frontmatter.",
            "Start the file with ---, then YAML fields, then a closing ---.",
            DiagnosticPosition::FILE_START,
        ));
    }

    let mut parts = content.splitn(3, "---");
    let _leading = parts.next();
    let Some(yaml) = parts.next() else {
        return FrontmatterSplit::Err(SkillDiagnostic::new(
            SkillDiagnosticCode::UnclosedFrontmatter,
            None,
            "YAML frontmatter is not closed.",
            "Close the frontmatter with a --- line before the markdown body.",
            DiagnosticPosition::FILE_START,
        ));
    };
    let Some(body) = parts.next() else {
        return FrontmatterSplit::Err(SkillDiagnostic::new(
            SkillDiagnosticCode::UnclosedFrontmatter,
            None,
            "YAML frontmatter is not closed.",
            "Close the frontmatter with a --- line before the markdown body.",
            DiagnosticPosition::FILE_START,
        ));
    };

    FrontmatterSplit::Ok {
        yaml,
        body,
        yaml_start_line: 2,
    }
}

fn parse_frontmatter_mapping(
    yaml: &str,
    yaml_start_line: u32,
) -> Result<serde_yaml::Mapping, SkillDiagnostic> {
    let parsed: Value = match serde_yaml::from_str(yaml) {
        Ok(value) => value,
        Err(err) => {
            let position = err
                .location()
                .map(|loc| {
                    DiagnosticPosition::new(
                        yaml_start_line
                            .saturating_add(loc.line() as u32)
                            .saturating_sub(1),
                        loc.column() as u32,
                    )
                })
                .unwrap_or(DiagnosticPosition::new(yaml_start_line, 1));
            return Err(SkillDiagnostic::new(
                SkillDiagnosticCode::InvalidYaml,
                None,
                "Frontmatter is not valid YAML.",
                "Fix the YAML syntax. Quote values that contain colons or special characters.",
                position,
            ));
        }
    };

    // Official skills-ref uses strictyaml, which keeps every scalar as a
    // string (`construct_yaml_str`, including YAML 1.2 null). serde_yaml
    // 0.9 types unquoted `1.0` as Number, `true`/`false` as Bool, and
    // `null`/`Null`/`NULL`/`~` as Null. Preserve mappings and sequences,
    // then apply Grok bool/list rules on the stringly tree.
    let authored = AuthoredPlainScalars::scan(yaml);
    match stringify_yaml_scalars(parsed, "", &authored) {
        Value::Mapping(mapping) => Ok(mapping),
        Value::Null => Err(SkillDiagnostic::new(
            SkillDiagnosticCode::FrontmatterNotMapping,
            None,
            "Frontmatter must be a YAML mapping.",
            "Use key: value fields such as name and description.",
            DiagnosticPosition::new(yaml_start_line, 1),
        )),
        _ => Err(SkillDiagnostic::new(
            SkillDiagnosticCode::FrontmatterNotMapping,
            None,
            "Frontmatter must be a YAML mapping.",
            "Use key: value fields such as name and description.",
            DiagnosticPosition::new(yaml_start_line, 1),
        )),
    }
}

/// Recursively convert YAML scalars to strings, matching official strictyaml
/// `load()` behavior. Mappings and sequences stay structured so nested
/// collections can still be quarantined. Empty fields (`license:`) stay
/// Null because they have no authored token and must remain omitted.
/// Authored YAML 1.2 null lexemes (`null`, `Null`, `NULL`, `~`) are restored
/// to those tokens so they match official `construct_yaml_str`.
///
/// Do not reconstruct Number/Bool/Null through `Display`. serde_yaml 0.9
/// parses unquoted `1.10` as f64 `1.1`, `True` as bool `true`, and `null`
/// as Null, so formatting mutates the official string lexeme. Recover the
/// authored token from the frontmatter line instead. Quoted scalars are
/// already strings and stay unchanged. If the token cannot be recovered,
/// leave the typed value so later checks quarantine rather than invent a
/// scalar.
fn stringify_yaml_scalars(value: Value, path: &str, authored: &AuthoredPlainScalars) -> Value {
    match value {
        value @ (Value::Number(_) | Value::Bool(_) | Value::Null) => match authored.get(path) {
            Some(token) if !token.is_empty() => Value::String(token.to_string()),
            _ => value,
        },
        Value::Sequence(items) => Value::Sequence(
            items
                .into_iter()
                .enumerate()
                .map(|(idx, item)| {
                    stringify_yaml_scalars(item, &format!("{path}[{idx}]"), authored)
                })
                .collect(),
        ),
        Value::Mapping(map) => {
            let mut out = serde_yaml::Mapping::new();
            for (key, item) in map {
                // Official keys are strings. Do not reconstruct typed keys.
                let child = match key.as_str() {
                    Some(name) if path.is_empty() => name.to_string(),
                    Some(name) => format!("{path}.{name}"),
                    None => {
                        out.insert(key, item);
                        continue;
                    }
                };
                out.insert(key, stringify_yaml_scalars(item, &child, authored));
            }
            Value::Mapping(out)
        }
        Value::Tagged(mut tagged) => {
            tagged.value = stringify_yaml_scalars(tagged.value, path, authored);
            Value::Tagged(tagged)
        }
        other => other,
    }
}

/// Authored unquoted YAML 1.2 plain-scalar tokens, keyed by mapping path.
///
/// Used to restore Number/Bool/Null leaves that serde_yaml typed away.
/// Quoted scalars, empty values, and nested collections are omitted so
/// empty fields stay Null (`license:`) and quoted `"1.0"` stays the parsed
/// string. YAML 1.2 null lexemes (`null`/`Null`/`NULL`/`~`) are recorded as
/// their nonempty authored tokens.
struct AuthoredPlainScalars {
    tokens: BTreeMap<String, String>,
}

impl AuthoredPlainScalars {
    fn scan(yaml: &str) -> Self {
        let mut tokens = BTreeMap::new();
        let mut stack: Vec<ScanFrame> = Vec::new();
        for raw_line in yaml.lines() {
            let line = raw_line.trim_end_matches('\r');
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            let indent = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
            let trimmed = line.trim_start();
            while stack.last().is_some_and(|frame| frame.indent >= indent) {
                stack.pop();
            }
            if let Some(item) = strip_sequence_entry(trimmed) {
                let item_path = next_sequence_path(&mut stack);
                stack.push(ScanFrame {
                    indent,
                    path: item_path.clone(),
                    next_seq: 0,
                });
                if let Some((key, rest)) = split_mapping_entry(item) {
                    let child = join_yaml_path(&item_path, key);
                    record_authored_plain(&mut tokens, &child, rest);
                    stack.push(ScanFrame {
                        indent: indent.saturating_add(2),
                        path: child,
                        next_seq: 0,
                    });
                } else {
                    record_authored_plain(&mut tokens, &item_path, item);
                }
                continue;
            }
            let Some((key, rest)) = split_mapping_entry(trimmed) else {
                continue;
            };
            let parent = stack.last().map(|frame| frame.path.as_str()).unwrap_or("");
            let path = join_yaml_path(parent, key);
            record_authored_plain(&mut tokens, &path, rest);
            stack.push(ScanFrame {
                indent,
                path,
                next_seq: 0,
            });
        }
        Self { tokens }
    }

    fn get(&self, path: &str) -> Option<&str> {
        self.tokens.get(path).map(String::as_str)
    }
}

struct ScanFrame {
    indent: usize,
    path: String,
    next_seq: usize,
}

fn next_sequence_path(stack: &mut [ScanFrame]) -> String {
    match stack.last_mut() {
        Some(parent) => {
            let idx = parent.next_seq;
            parent.next_seq = parent.next_seq.saturating_add(1);
            format!("{}[{idx}]", parent.path)
        }
        None => "[0]".to_string(),
    }
}

fn join_yaml_path(parent: &str, key: &str) -> String {
    if parent.is_empty() {
        key.to_string()
    } else {
        format!("{parent}.{key}")
    }
}

fn strip_sequence_entry(trimmed: &str) -> Option<&str> {
    let rest = trimmed.strip_prefix('-')?;
    if rest.is_empty() {
        return Some("");
    }
    if rest.starts_with(' ') || rest.starts_with('\t') {
        Some(rest.trim_start())
    } else {
        None
    }
}

fn split_mapping_entry(trimmed: &str) -> Option<(&str, &str)> {
    let (key, rest) = trimmed.split_once(':')?;
    let key = unquote_yaml_key(key.trim());
    if key.is_empty() {
        None
    } else {
        Some((key, rest))
    }
}

fn unquote_yaml_key(key: &str) -> &str {
    if key.len() >= 2
        && ((key.starts_with('"') && key.ends_with('"'))
            || (key.starts_with('\'') && key.ends_with('\'')))
    {
        &key[1..key.len() - 1]
    } else {
        key
    }
}

fn record_authored_plain(tokens: &mut BTreeMap<String, String>, path: &str, raw: &str) {
    let trimmed = strip_plain_comment(raw).trim();
    if let Some(inner) = flow_mapping_inner(trimmed) {
        for part in split_flow_items(inner) {
            if let Some((key, rest)) = split_mapping_entry(part) {
                record_authored_plain(tokens, &join_yaml_path(path, key), rest);
            }
        }
        return;
    }
    if let Some(inner) = flow_sequence_inner(trimmed) {
        for (idx, part) in split_flow_items(inner).into_iter().enumerate() {
            record_authored_plain(tokens, &format!("{path}[{idx}]"), part);
        }
        return;
    }
    if let Some(token) = plain_scalar_token(raw) {
        tokens.insert(path.to_string(), token);
    }
}

fn flow_mapping_inner(trimmed: &str) -> Option<&str> {
    trimmed
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
}

fn flow_sequence_inner(trimmed: &str) -> Option<&str> {
    trimmed
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
}

fn split_flow_items(inner: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    for (idx, ch) in inner.char_indices() {
        if in_double {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_double = false;
            }
            continue;
        }
        if in_single {
            if ch == '\'' {
                in_single = false;
            }
            continue;
        }
        match ch {
            '"' => in_double = true,
            '\'' => in_single = true,
            '{' | '[' => depth = depth.saturating_add(1),
            '}' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let part = inner[start..idx].trim();
                if !part.is_empty() {
                    parts.push(part);
                }
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    let last = inner[start..].trim();
    if !last.is_empty() {
        parts.push(last);
    }
    parts
}

fn plain_scalar_token(raw: &str) -> Option<String> {
    let trimmed = strip_plain_comment(raw).trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('"') || trimmed.starts_with('\'') {
        return None;
    }
    if trimmed.starts_with(['[', '{', '|', '>', '!', '&', '*']) {
        return None;
    }
    Some(trimmed.to_string())
}

fn strip_plain_comment(raw: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    let mut last_was_ws = true;
    for (idx, ch) in raw.char_indices() {
        if in_double {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_double = false;
            }
            last_was_ws = false;
            continue;
        }
        if in_single {
            if ch == '\'' {
                in_single = false;
            }
            last_was_ws = false;
            continue;
        }
        if ch == '#' && last_was_ws {
            return &raw[..idx];
        }
        if ch == '"' {
            in_double = true;
            last_was_ws = false;
            continue;
        }
        if ch == '\'' {
            in_single = true;
            last_was_ws = false;
            continue;
        }
        last_was_ws = ch.is_whitespace();
    }
    raw
}

struct FieldPositions {
    keys: BTreeMap<String, DiagnosticPosition>,
}

impl FieldPositions {
    fn scan(content: &str) -> Self {
        let mut keys = BTreeMap::new();
        let mut stack: Vec<(usize, String)> = Vec::new();
        for (idx, raw_line) in content.lines().enumerate() {
            let line = raw_line.trim_end_matches('\r');
            if line.trim() == "---" {
                continue;
            }
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            let indent = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
            let trimmed = line.trim_start();
            let Some((raw_key, _)) = trimmed.split_once(':') else {
                continue;
            };
            let key = raw_key.trim();
            if key.is_empty() {
                continue;
            }
            while stack.last().is_some_and(|(ind, _)| *ind >= indent) {
                stack.pop();
            }
            let path = if stack.is_empty() {
                key.to_string()
            } else {
                let prefix = stack
                    .iter()
                    .map(|(_, name)| name.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                format!("{prefix}.{key}")
            };
            let column = indent as u32 + 1;
            keys.entry(path.clone())
                .or_insert(DiagnosticPosition::new((idx as u32) + 1, column));
            stack.push((indent, key.to_string()));
        }
        Self { keys }
    }

    fn get(&self, key: &str) -> DiagnosticPosition {
        self.keys
            .get(key)
            .copied()
            .unwrap_or(DiagnosticPosition::FILE_START)
    }
}

fn collect_duplicate_top_level_keys(
    yaml: &str,
    yaml_start_line: u32,
    diagnostics: &mut Vec<SkillDiagnostic>,
) {
    let mut seen = BTreeSet::new();
    for (idx, raw_line) in yaml.lines().enumerate() {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty()
            || line.starts_with(' ')
            || line.starts_with('\t')
            || line.starts_with('#')
        {
            continue;
        }
        let Some((key, _)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        if !seen.insert(key.to_string()) {
            diagnostics.push(SkillDiagnostic::new(
                SkillDiagnosticCode::DuplicateTopLevelKey,
                Some(key),
                "Duplicate top-level frontmatter key.",
                "Keep a single occurrence of each top-level key.",
                DiagnosticPosition::new(yaml_start_line.saturating_add(idx as u32), 1),
            ));
        }
    }
}

fn collect_top_level_errors(
    mapping: &serde_yaml::Mapping,
    positions: &FieldPositions,
    diagnostics: &mut Vec<SkillDiagnostic>,
) {
    for key in mapping.keys() {
        let Some(name) = key.as_str() else {
            diagnostics.push(SkillDiagnostic::new(
                SkillDiagnosticCode::TopLevelKeyNotString,
                None,
                "Frontmatter keys must be strings.",
                "Use quoted or bare string keys only.",
                DiagnosticPosition::FILE_START,
            ));
            continue;
        };
        if is_official_top_level_key(name) {
            continue;
        }
        let remediation = if is_legacy_grok_top_level_key(name) {
            legacy_key_remediation(name)
        } else {
            "Remove the unknown key or move product-specific data under metadata as a string."
                .to_string()
        };
        diagnostics.push(SkillDiagnostic::new(
            SkillDiagnosticCode::UnexpectedTopLevelKey,
            Some(name),
            "Unexpected top-level frontmatter key.",
            remediation,
            positions.get(name),
        ));
    }
}

fn legacy_key_remediation(name: &str) -> String {
    let leaf = match name {
        "when_to_use" => "when-to-use",
        other => other,
    };
    format!("Move '{name}' to metadata.grok.{leaf}.")
}

fn required_string<'a>(
    mapping: &'a serde_yaml::Mapping,
    key: &str,
    missing: SkillDiagnosticCode,
    positions: &FieldPositions,
) -> Result<&'a str, SkillDiagnostic> {
    match mapping.get(Value::String(key.to_string())) {
        None => Err(SkillDiagnostic::new(
            missing,
            Some(key),
            format!("Required field '{key}' is missing."),
            format!("Add a nonempty string '{key}' field."),
            positions.get(key),
        )),
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(value.as_str()),
        // Empty keys (`name:`) stay YAML Null after stringify. Official
        // skills-ref treats a present empty/null required field as a
        // nonempty-string error, not a type error. Authored `null`/`Null`/
        // `NULL`/`~` are restored to strings before this match.
        Some(Value::String(_) | Value::Null) => Err(SkillDiagnostic::new(
            if key == "name" {
                SkillDiagnosticCode::EmptyName
            } else {
                SkillDiagnosticCode::EmptyDescription
            },
            Some(key),
            format!("Field '{key}' must be a nonempty string."),
            format!("Set '{key}' to a nonempty string."),
            positions.get(key),
        )),
        Some(_) => {
            let code = if key == "name" {
                SkillDiagnosticCode::NameNotString
            } else {
                SkillDiagnosticCode::DescriptionNotString
            };
            Err(SkillDiagnostic::new(
                code,
                Some(key),
                format!("Field '{key}' must be a string."),
                format!("Write '{key}' as a YAML scalar. Nested lists and maps are not accepted."),
                positions.get(key),
            ))
        }
    }
}

fn optional_string(
    mapping: &serde_yaml::Mapping,
    key: &str,
    not_string: SkillDiagnosticCode,
    positions: &FieldPositions,
    diagnostics: &mut Vec<SkillDiagnostic>,
) -> Option<String> {
    match mapping.get(Value::String(key.to_string())) {
        None => None,
        Some(Value::String(value)) => Some(value.clone()),
        Some(Value::Null) => None,
        Some(_) => {
            diagnostics.push(SkillDiagnostic::new(
                not_string,
                Some(key),
                format!("Field '{key}' must be a string."),
                format!("Write '{key}' as a YAML scalar. Nested lists and maps are not accepted."),
                positions.get(key),
            ));
            None
        }
    }
}

fn validate_name(
    raw: &str,
    parent_dir_name: &str,
    positions: &FieldPositions,
    diagnostics: &mut Vec<SkillDiagnostic>,
) -> Option<String> {
    // Official skills-ref strips then applies NFKC before grammar checks.
    // The stored name is the authored string; only comparison/grammar use NFKC.
    let normalized = nfkc(raw.trim());
    let pos = positions.get("name");

    if normalized.is_empty() {
        diagnostics.push(SkillDiagnostic::new(
            SkillDiagnosticCode::EmptyName,
            Some("name"),
            "Field 'name' must be a nonempty string.",
            "Set name to the parent directory name.",
            pos,
        ));
        return None;
    }

    if normalized.chars().count() > MAX_NAME_CHARS {
        diagnostics.push(SkillDiagnostic::new(
            SkillDiagnosticCode::NameTooLong,
            Some("name"),
            "Skill name exceeds the 64-character limit.",
            "Shorten name to at most 64 characters.",
            pos,
        ));
    }

    if normalized != normalized.to_lowercase() {
        diagnostics.push(SkillDiagnostic::new(
            SkillDiagnosticCode::NameNotLowercase,
            Some("name"),
            "Skill name must be lowercase.",
            "Rewrite name using lowercase letters, digits, and hyphens.",
            pos,
        ));
    }

    if normalized.starts_with('-') || normalized.ends_with('-') {
        diagnostics.push(SkillDiagnostic::new(
            SkillDiagnosticCode::NameLeadingOrTrailingHyphen,
            Some("name"),
            "Skill name cannot start or end with a hyphen.",
            "Remove leading and trailing hyphens from name.",
            pos,
        ));
    }

    if normalized.contains("--") {
        diagnostics.push(SkillDiagnostic::new(
            SkillDiagnosticCode::NameConsecutiveHyphens,
            Some("name"),
            "Skill name cannot contain consecutive hyphens.",
            "Replace consecutive hyphens with a single hyphen.",
            pos,
        ));
    }

    if !normalized.chars().all(|c| c.is_alphanumeric() || c == '-') {
        diagnostics.push(SkillDiagnostic::new(
            SkillDiagnosticCode::NameInvalidCharacters,
            Some("name"),
            "Skill name contains invalid characters.",
            "Use only letters, digits, and hyphens in name.",
            pos,
        ));
    }

    let expected = nfkc(&sanitize_parent_dir_name(parent_dir_name));
    if expected != normalized {
        diagnostics.push(SkillDiagnostic::new(
            SkillDiagnosticCode::NameDirectoryMismatch,
            Some("name"),
            "Directory name must match the skill name.",
            "Set name to the parent directory name, or rename the directory to match name.",
            pos,
        ));
    }

    Some(raw.to_string())
}

fn validate_description(
    raw: &str,
    positions: &FieldPositions,
    diagnostics: &mut Vec<SkillDiagnostic>,
) -> Option<String> {
    if raw.trim().is_empty() {
        diagnostics.push(SkillDiagnostic::new(
            SkillDiagnosticCode::EmptyDescription,
            Some("description"),
            "Field 'description' must be a nonempty string.",
            "Write a description of what the skill does and when to use it.",
            positions.get("description"),
        ));
        return None;
    }
    if raw.chars().count() > MAX_DESCRIPTION_CHARS {
        diagnostics.push(SkillDiagnostic::new(
            SkillDiagnosticCode::DescriptionTooLong,
            Some("description"),
            "Description exceeds the 1024-character limit.",
            "Shorten description to at most 1024 characters.",
            positions.get("description"),
        ));
        return None;
    }
    Some(raw.to_string())
}

fn validate_compatibility(
    raw: &str,
    positions: &FieldPositions,
    diagnostics: &mut Vec<SkillDiagnostic>,
) -> bool {
    if raw.chars().count() > MAX_COMPATIBILITY_CHARS {
        diagnostics.push(SkillDiagnostic::new(
            SkillDiagnosticCode::CompatibilityTooLong,
            Some("compatibility"),
            "Compatibility exceeds the 500-character limit.",
            "Shorten compatibility to at most 500 characters.",
            positions.get("compatibility"),
        ));
        return false;
    }
    true
}

fn parse_metadata(
    mapping: &serde_yaml::Mapping,
    positions: &FieldPositions,
    diagnostics: &mut Vec<SkillDiagnostic>,
) -> (BTreeMap<String, String>, GrokSkillExtensions) {
    let Some(raw) = mapping.get(Value::String("metadata".to_string())) else {
        return (BTreeMap::new(), GrokSkillExtensions::default());
    };
    let Some(meta) = raw.as_mapping() else {
        if !matches!(raw, Value::Null) {
            diagnostics.push(SkillDiagnostic::new(
                SkillDiagnosticCode::MetadataNotMapping,
                Some("metadata"),
                "Field 'metadata' must be a mapping of strings.",
                "Write metadata as a YAML mapping with string keys and string values.",
                positions.get("metadata"),
            ));
        }
        return (BTreeMap::new(), GrokSkillExtensions::default());
    };

    let mut official = BTreeMap::new();
    let mut grok = GrokSkillExtensions::default();
    let mut saw_nested_grok = false;
    let mut saw_dotted_grok = false;

    for (key, value) in meta {
        let Some(name) = key.as_str() else {
            diagnostics.push(SkillDiagnostic::new(
                SkillDiagnosticCode::MetadataKeyNotString,
                Some("metadata"),
                "Metadata keys must be strings.",
                "Use string keys in metadata.",
                positions.get("metadata"),
            ));
            continue;
        };

        if name == GROK_EXTENSION_OBJECT_KEY {
            saw_nested_grok = true;
            parse_nested_grok(value, positions, &mut grok, diagnostics);
            continue;
        }

        if let Some(leaf) = grok_extension_leaf(name) {
            saw_dotted_grok = true;
            match value.as_str() {
                Some(raw) => apply_dotted_grok(leaf, raw, positions, &mut grok, diagnostics),
                None => diagnostics.push(SkillDiagnostic::new(
                    SkillDiagnosticCode::MetadataValueNotString,
                    Some(name),
                    "Official metadata values must be strings.",
                    "Write metadata values as YAML scalars. Nested lists and maps are not accepted.",
                    positions.get(&format!("metadata.{name}")),
                )),
            }
            continue;
        }

        if name.starts_with("grok.") {
            diagnostics.push(SkillDiagnostic::new(
                SkillDiagnosticCode::GrokExtensionUnknownKey,
                Some(name),
                "Unknown metadata.grok extension key.",
                "Use a documented metadata.grok.* key or move custom data to another metadata key.",
                positions.get(&format!("metadata.{name}")),
            ));
            continue;
        }

        match value.as_str() {
            Some(raw) => {
                official.insert(name.to_string(), raw.to_string());
            }
            None => diagnostics.push(SkillDiagnostic::new(
                SkillDiagnosticCode::MetadataValueNotString,
                Some(name),
                "Official metadata values must be strings.",
                "Write metadata values as YAML scalars. Nested lists and maps are not accepted.",
                positions.get(&format!("metadata.{name}")),
            )),
        }
    }

    if saw_nested_grok && saw_dotted_grok {
        diagnostics.push(SkillDiagnostic::new(
            SkillDiagnosticCode::GrokExtensionConflict,
            Some("metadata.grok"),
            "Do not mix metadata.grok and metadata.grok.* keys.",
            "Use either a nested metadata.grok mapping or dotted metadata.grok.* keys.",
            positions.get("metadata.grok"),
        ));
    }

    (official, grok)
}

fn parse_nested_grok(
    value: &Value,
    positions: &FieldPositions,
    grok: &mut GrokSkillExtensions,
    diagnostics: &mut Vec<SkillDiagnostic>,
) {
    let Some(map) = value.as_mapping() else {
        diagnostics.push(SkillDiagnostic::new(
            SkillDiagnosticCode::GrokExtensionNotMapping,
            Some("metadata.grok"),
            "metadata.grok must be a mapping.",
            "Write metadata.grok as a YAML mapping of documented extension keys.",
            positions.get("metadata.grok"),
        ));
        return;
    };

    for (key, item) in map {
        let Some(leaf) = key.as_str() else {
            diagnostics.push(SkillDiagnostic::new(
                SkillDiagnosticCode::GrokExtensionUnknownKey,
                Some("metadata.grok"),
                "metadata.grok keys must be strings.",
                "Use documented metadata.grok keys such as when-to-use and paths.",
                positions.get("metadata.grok"),
            ));
            continue;
        };
        if !is_known_grok_extension_leaf(leaf) {
            diagnostics.push(SkillDiagnostic::new(
                SkillDiagnosticCode::GrokExtensionUnknownKey,
                Some(&format!("metadata.grok.{leaf}")),
                "Unknown metadata.grok extension key.",
                "Use a documented metadata.grok.* key.",
                positions.get(&format!("metadata.grok.{leaf}")),
            ));
            continue;
        }
        apply_nested_grok(leaf, item, positions, grok, diagnostics);
    }
}

fn apply_nested_grok(
    leaf: &str,
    value: &Value,
    positions: &FieldPositions,
    grok: &mut GrokSkillExtensions,
    diagnostics: &mut Vec<SkillDiagnostic>,
) {
    let field = format!("metadata.grok.{leaf}");
    let pos = positions.get(&field);
    match leaf {
        "when-to-use" => {
            grok.when_to_use =
                require_bounded_string(value, &field, MAX_GROK_WHEN_TO_USE_CHARS, pos, diagnostics);
        }
        "argument-hint" => {
            grok.argument_hint = require_bounded_string(
                value,
                &field,
                MAX_GROK_ARGUMENT_HINT_CHARS,
                pos,
                diagnostics,
            );
        }
        "model" => {
            grok.model =
                require_bounded_string(value, &field, MAX_GROK_MODEL_CHARS, pos, diagnostics);
        }
        "effort" => {
            grok.effort =
                require_bounded_string(value, &field, MAX_GROK_EFFORT_CHARS, pos, diagnostics);
        }
        "short-description" => {
            grok.short_description = require_bounded_string(
                value,
                &field,
                MAX_GROK_SHORT_DESCRIPTION_CHARS,
                pos,
                diagnostics,
            );
        }
        "user-invocable" => grok.user_invocable = require_bool(value, &field, pos, diagnostics),
        "disable-model-invocation" => {
            grok.disable_model_invocation = require_bool(value, &field, pos, diagnostics);
        }
        "paths" => grok.paths = require_paths(value, &field, pos, diagnostics),
        _ => {}
    }
}

fn apply_dotted_grok(
    leaf: &str,
    raw: &str,
    positions: &FieldPositions,
    grok: &mut GrokSkillExtensions,
    diagnostics: &mut Vec<SkillDiagnostic>,
) {
    let field = format!("metadata.{}.{}", GROK_EXTENSION_OBJECT_KEY, leaf);
    let pos = positions.get(&format!("metadata.grok.{leaf}"));
    match leaf {
        "when-to-use" => {
            if let Some(value) =
                require_nonempty_len(raw, &field, MAX_GROK_WHEN_TO_USE_CHARS, pos, diagnostics)
            {
                grok.when_to_use = Some(value);
            }
        }
        "argument-hint" => {
            if let Some(value) =
                require_nonempty_len(raw, &field, MAX_GROK_ARGUMENT_HINT_CHARS, pos, diagnostics)
            {
                grok.argument_hint = Some(value);
            }
        }
        "model" => {
            if let Some(value) =
                require_nonempty_len(raw, &field, MAX_GROK_MODEL_CHARS, pos, diagnostics)
            {
                grok.model = Some(value);
            }
        }
        "effort" => {
            if let Some(value) =
                require_nonempty_len(raw, &field, MAX_GROK_EFFORT_CHARS, pos, diagnostics)
            {
                grok.effort = Some(value);
            }
        }
        "short-description" => {
            if let Some(value) = require_nonempty_len(
                raw,
                &field,
                MAX_GROK_SHORT_DESCRIPTION_CHARS,
                pos,
                diagnostics,
            ) {
                grok.short_description = Some(value);
            }
        }
        "user-invocable" => grok.user_invocable = parse_bool_string(raw, &field, pos, diagnostics),
        "disable-model-invocation" => {
            grok.disable_model_invocation = parse_bool_string(raw, &field, pos, diagnostics);
        }
        "paths" => {
            if let Some(value) =
                require_nonempty_len(raw, &field, MAX_GROK_PATH_CHARS, pos, diagnostics)
            {
                grok.paths = Some(vec![value]);
            }
        }
        _ => {}
    }
}

fn require_bounded_string(
    value: &Value,
    field: &str,
    max_chars: usize,
    pos: DiagnosticPosition,
    diagnostics: &mut Vec<SkillDiagnostic>,
) -> Option<String> {
    let Some(raw) = value.as_str() else {
        diagnostics.push(invalid_extension(field, pos));
        return None;
    };
    require_nonempty_len(raw, field, max_chars, pos, diagnostics)
}

fn require_nonempty_len(
    raw: &str,
    field: &str,
    max_chars: usize,
    pos: DiagnosticPosition,
    diagnostics: &mut Vec<SkillDiagnostic>,
) -> Option<String> {
    if raw.trim().is_empty() || raw.chars().count() > max_chars {
        diagnostics.push(invalid_extension(field, pos));
        return None;
    }
    Some(raw.to_string())
}

fn require_bool(
    value: &Value,
    field: &str,
    pos: DiagnosticPosition,
    diagnostics: &mut Vec<SkillDiagnostic>,
) -> Option<bool> {
    match value.as_str() {
        Some(raw) => parse_bool_string(raw, field, pos, diagnostics),
        None => {
            diagnostics.push(invalid_extension(field, pos));
            None
        }
    }
}

fn parse_bool_string(
    raw: &str,
    field: &str,
    pos: DiagnosticPosition,
    diagnostics: &mut Vec<SkillDiagnostic>,
) -> Option<bool> {
    match raw {
        "true" => Some(true),
        "false" => Some(false),
        _ => {
            diagnostics.push(invalid_extension(field, pos));
            None
        }
    }
}

fn require_paths(
    value: &Value,
    field: &str,
    pos: DiagnosticPosition,
    diagnostics: &mut Vec<SkillDiagnostic>,
) -> Option<Vec<String>> {
    match value {
        Value::String(raw) => {
            require_nonempty_len(raw, field, MAX_GROK_PATH_CHARS, pos, diagnostics).map(|s| vec![s])
        }
        Value::Sequence(items) => {
            if items.is_empty() || items.len() > MAX_GROK_PATHS {
                diagnostics.push(invalid_extension(field, pos));
                return None;
            }
            let mut paths = Vec::with_capacity(items.len());
            for item in items {
                let Some(raw) = item.as_str() else {
                    diagnostics.push(invalid_extension(field, pos));
                    return None;
                };
                let Some(path) =
                    require_nonempty_len(raw, field, MAX_GROK_PATH_CHARS, pos, diagnostics)
                else {
                    return None;
                };
                paths.push(path);
            }
            Some(paths)
        }
        _ => {
            diagnostics.push(invalid_extension(field, pos));
            None
        }
    }
}

fn invalid_extension(field: &str, pos: DiagnosticPosition) -> SkillDiagnostic {
    SkillDiagnostic::new(
        SkillDiagnosticCode::GrokExtensionInvalidValue,
        Some(field),
        "Invalid metadata.grok extension value.",
        "Use the documented type and length for this metadata.grok.* field. Values are not repaired.",
        pos,
    )
}

fn collect_warnings(
    description: &str,
    license: Option<&str>,
    compatibility: Option<&str>,
    allowed_tools: Option<&str>,
    grok: &GrokSkillExtensions,
    body: &str,
    positions: &FieldPositions,
) -> Vec<SkillAuthoringWarning> {
    let mut warnings = Vec::new();
    if description.chars().count() < 40 {
        warnings.push(SkillAuthoringWarning::new(
            SkillWarningCode::ShortDescription,
            Some("description"),
            "Description is shorter than the recommended authoring length.",
            "Describe what the skill does and when to use it in more detail.",
            positions.get("description"),
        ));
    }
    if license.is_none() {
        warnings.push(SkillAuthoringWarning::new(
            SkillWarningCode::MissingLicense,
            Some("license"),
            "License is omitted.",
            "Add a license field such as Apache-2.0 when you can.",
            positions.get("license"),
        ));
    }
    if matches!(compatibility, Some(value) if value.trim().is_empty()) {
        warnings.push(SkillAuthoringWarning::new(
            SkillWarningCode::EmptyCompatibility,
            Some("compatibility"),
            "Compatibility is empty.",
            "Omit compatibility or describe environment requirements.",
            positions.get("compatibility"),
        ));
    }
    if matches!(allowed_tools, Some(value) if value.trim().is_empty()) {
        warnings.push(SkillAuthoringWarning::new(
            SkillWarningCode::EmptyAllowedTools,
            Some("allowed-tools"),
            "allowed-tools is empty.",
            "Omit allowed-tools or write a space-separated tool string.",
            positions.get("allowed-tools"),
        ));
    }
    if grok.when_to_use.is_none() {
        warnings.push(SkillAuthoringWarning::new(
            SkillWarningCode::MissingGrokWhenToUse,
            Some("metadata.grok.when-to-use"),
            "metadata.grok.when-to-use is omitted.",
            "Add metadata.grok.when-to-use with trigger phrases for automatic invocation.",
            positions.get("metadata.grok.when-to-use"),
        ));
    }
    let body = body.trim();
    if body.is_empty() {
        warnings.push(SkillAuthoringWarning::new(
            SkillWarningCode::EmptyBody,
            None,
            "Markdown body is empty.",
            "Add instructions after the closing frontmatter marker.",
            DiagnosticPosition::FILE_START,
        ));
    } else if body.lines().count() > 500 {
        warnings.push(SkillAuthoringWarning::new(
            SkillWarningCode::LongBody,
            None,
            "Markdown body exceeds the recommended 500-line limit.",
            "Move detailed reference material into referenced files.",
            DiagnosticPosition::FILE_START,
        ));
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::skills::discovery::{
        is_valid_skill_name, normalize_skill_name, parse_skill_files, parse_skill_frontmatter,
    };
    use crate::implementations::skills::strict::{
        STRICT_VALIDATOR_RUNTIME_ENABLED, SkillInventory,
    };
    use crate::implementations::skills::types::SkillScope;

    fn validate(parent: &str, content: &str) -> StrictSkillOutcome {
        validate_strict_skill(StrictSkillInput {
            file_name: SKILL_MD_FILE_NAME,
            parent_dir_name: parent,
            content,
            scope: None,
        })
    }

    fn valid(parent: &str, content: &str) -> DiscoveredSkill {
        match validate(parent, content) {
            StrictSkillOutcome::Valid(skill) => skill,
            StrictSkillOutcome::Quarantined(row) => {
                panic!(
                    "expected valid skill, got {:?}",
                    row.diagnostics
                        .iter()
                        .map(|d| d.stable_line())
                        .collect::<Vec<_>>()
                );
            }
        }
    }

    fn codes(outcome: &StrictSkillOutcome) -> Vec<SkillDiagnosticCode> {
        outcome.diagnostics().iter().map(|d| d.code).collect()
    }

    #[test]
    fn spec_revision_is_pinned_hex_and_never_fetched() {
        assert_eq!(super::super::spec::AGENTSKILLS_SPEC_REVISION.len(), 40);
        assert!(
            super::super::spec::AGENTSKILLS_SPEC_REVISION
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        );
        assert!(STRICT_VALIDATOR_RUNTIME_ENABLED);
    }

    #[test]
    fn accepts_official_minimal_skill() {
        let skill = valid(
            "my-skill",
            "---\nname: my-skill\ndescription: A test skill used when validating fixtures.\n---\n# My Skill\n",
        );
        assert_eq!(skill.manifest.name, "my-skill");
        assert!(skill.manifest.grok.is_empty());
    }

    #[test]
    fn accepts_official_optional_fields_and_string_metadata() {
        let skill = valid(
            "pdf-processing",
            "---\nname: pdf-processing\ndescription: Extract PDF text, fill forms, merge files. Use when handling PDFs.\nlicense: Apache-2.0\ncompatibility: Requires Python 3.11+\nmetadata:\n  author: example-org\n  version: \"1.0\"\nallowed-tools: Bash(jq:*) Bash(git:*) Read\n---\nBody\n",
        );
        assert_eq!(skill.manifest.license.as_deref(), Some("Apache-2.0"));
        assert_eq!(
            skill.manifest.compatibility.as_deref(),
            Some("Requires Python 3.11+")
        );
        assert_eq!(
            skill.manifest.allowed_tools.as_deref(),
            Some("Bash(jq:*) Bash(git:*) Read")
        );
        assert_eq!(
            skill.manifest.allowed_tool_tokens(),
            vec!["Bash(jq:*)", "Bash(git:*)", "Read"]
        );
        assert_eq!(
            skill.manifest.metadata.get("author").map(String::as_str),
            Some("example-org")
        );
        assert_eq!(
            skill.manifest.metadata.get("version").map(String::as_str),
            Some("1.0")
        );
    }

    #[test]
    fn accepts_unicode_lowercase_names() {
        let chinese = valid(
            "技能",
            "---\nname: 技能\ndescription: A skill with Chinese name used in official fixtures.\n---\nBody\n",
        );
        assert_eq!(chinese.manifest.name, "技能");
        let russian = valid(
            "мой-навык",
            "---\nname: мой-навык\ndescription: A skill with Russian name used in official fixtures.\n---\nBody\n",
        );
        assert_eq!(russian.manifest.name, "мой-навык");
    }

    #[test]
    fn accepts_nfkc_directory_and_name_match() {
        let composed = "café";
        let decomposed = "cafe\u{0301}";
        let skill = valid(
            composed,
            &format!(
                "---\nname: {decomposed}\ndescription: A test skill used when validating NFKC fixtures.\n---\nBody\n"
            ),
        );
        assert_eq!(skill.manifest.name, decomposed);
    }

    #[test]
    fn rejects_official_name_failures() {
        let cases: &[(&str, &str, SkillDiagnosticCode)] = &[
            (
                "MySkill",
                "---\nname: MySkill\ndescription: A test skill\n---\nBody\n",
                SkillDiagnosticCode::NameNotLowercase,
            ),
            (
                &"a".repeat(70),
                &format!(
                    "---\nname: {}\ndescription: A test skill\n---\nBody\n",
                    "a".repeat(70)
                ),
                SkillDiagnosticCode::NameTooLong,
            ),
            (
                "-my-skill",
                "---\nname: -my-skill\ndescription: A test skill\n---\nBody\n",
                SkillDiagnosticCode::NameLeadingOrTrailingHyphen,
            ),
            (
                "my-skill-",
                "---\nname: my-skill-\ndescription: A test skill\n---\nBody\n",
                SkillDiagnosticCode::NameLeadingOrTrailingHyphen,
            ),
            (
                "my--skill",
                "---\nname: my--skill\ndescription: A test skill\n---\nBody\n",
                SkillDiagnosticCode::NameConsecutiveHyphens,
            ),
            (
                "my_skill",
                "---\nname: my_skill\ndescription: A test skill\n---\nBody\n",
                SkillDiagnosticCode::NameInvalidCharacters,
            ),
            (
                "wrong-name",
                "---\nname: correct-name\ndescription: A test skill\n---\nBody\n",
                SkillDiagnosticCode::NameDirectoryMismatch,
            ),
            (
                "НАВЫК",
                "---\nname: НАВЫК\ndescription: A skill with Russian uppercase name\n---\nBody\n",
                SkillDiagnosticCode::NameNotLowercase,
            ),
        ];
        for (parent, content, expected) in cases {
            let outcome = validate(parent, content);
            assert!(
                codes(&outcome).contains(expected),
                "parent={parent} codes={:?}",
                codes(&outcome)
            );
        }
    }

    #[test]
    fn rejects_missing_required_fields_and_frontmatter_errors() {
        assert!(
            codes(&validate(
                "my-skill",
                "---\ndescription: A test skill\n---\nBody\n"
            ))
            .contains(&SkillDiagnosticCode::MissingName)
        );
        assert!(
            codes(&validate("my-skill", "---\nname: my-skill\n---\nBody\n"))
                .contains(&SkillDiagnosticCode::MissingDescription)
        );
        assert!(
            codes(&validate("my-skill", "# No frontmatter\n"))
                .contains(&SkillDiagnosticCode::MissingFrontmatter)
        );
        assert!(
            codes(&validate(
                "my-skill",
                "---\nname: my-skill\ndescription: A test skill\n"
            ))
            .contains(&SkillDiagnosticCode::UnclosedFrontmatter)
        );
        assert!(
            codes(&validate(
                "my-skill",
                "---\nname: [invalid\ndescription: broken\n---\nBody\n"
            ))
            .contains(&SkillDiagnosticCode::InvalidYaml)
        );
        assert!(
            codes(&validate(
                "my-skill",
                "---\n- just\n- a\n- list\n---\nBody\n"
            ))
            .contains(&SkillDiagnosticCode::FrontmatterNotMapping)
        );
    }

    #[test]
    fn quarantines_empty_required_keys_as_empty_not_type_errors() {
        let empty_name = validate(
            "my-skill",
            "---\nname:\ndescription: A test skill used when validating fixtures.\n---\nBody\n",
        );
        assert!(
            codes(&empty_name).contains(&SkillDiagnosticCode::EmptyName),
            "empty name: key must be EmptyName, got {:?}",
            codes(&empty_name)
        );
        assert!(
            !codes(&empty_name).contains(&SkillDiagnosticCode::NameNotString),
            "empty name: key must not be NameNotString, got {:?}",
            codes(&empty_name)
        );
        assert!(empty_name.diagnostics().iter().any(|diag| {
            diag.code == SkillDiagnosticCode::EmptyName
                && diag.message == "Field 'name' must be a nonempty string."
                && diag.remediation == "Set 'name' to a nonempty string."
                && !diag.remediation.contains("Nested lists")
        }));

        let empty_description =
            validate("my-skill", "---\nname: my-skill\ndescription:\n---\nBody\n");
        assert!(
            codes(&empty_description).contains(&SkillDiagnosticCode::EmptyDescription),
            "empty description: key must be EmptyDescription, got {:?}",
            codes(&empty_description)
        );
        assert!(
            !codes(&empty_description).contains(&SkillDiagnosticCode::DescriptionNotString),
            "empty description: key must not be DescriptionNotString, got {:?}",
            codes(&empty_description)
        );
        assert!(empty_description.diagnostics().iter().any(|diag| {
            diag.code == SkillDiagnosticCode::EmptyDescription
                && diag.message == "Field 'description' must be a nonempty string."
                && diag.remediation == "Set 'description' to a nonempty string."
                && !diag.remediation.contains("Nested lists")
        }));

        let name_list = validate(
            "my-skill",
            "---\nname:\n  - my-skill\ndescription: A test skill used when validating fixtures.\n---\nBody\n",
        );
        assert!(codes(&name_list).contains(&SkillDiagnosticCode::NameNotString));
        assert!(!codes(&name_list).contains(&SkillDiagnosticCode::EmptyName));
        assert!(name_list.diagnostics().iter().any(|diag| {
            diag.code == SkillDiagnosticCode::NameNotString
                && diag
                    .remediation
                    .contains("Nested lists and maps are not accepted.")
        }));
    }

    #[test]
    fn rejects_unexpected_top_level_keys_including_legacy_grok_fields() {
        let outcome = validate(
            "my-skill",
            "---\nname: my-skill\ndescription: A test skill used when validating fixtures.\nwhen-to-use: commit\nunknown_field: should not be here\n---\nBody\n",
        );
        let codes = codes(&outcome);
        assert!(codes.contains(&SkillDiagnosticCode::UnexpectedTopLevelKey));
        let lines: Vec<_> = outcome
            .diagnostics()
            .iter()
            .map(SkillDiagnostic::stable_line)
            .collect();
        assert!(lines.iter().any(|l| l.contains("when-to-use")));
        assert!(lines.iter().any(|l| l.contains("unknown_field")));
        assert!(outcome.diagnostics().iter().any(|d| {
            d.field.as_deref() == Some("when-to-use")
                && d.remediation.contains("metadata.grok.when-to-use")
        }));
    }

    #[test]
    fn rejects_description_and_compatibility_over_limit() {
        let long_desc = "x".repeat(1100);
        assert!(
            codes(&validate(
                "my-skill",
                &format!("---\nname: my-skill\ndescription: {long_desc}\n---\nBody\n")
            ))
            .contains(&SkillDiagnosticCode::DescriptionTooLong)
        );
        let long_compat = "x".repeat(550);
        assert!(codes(&validate(
            "my-skill",
            &format!(
                "---\nname: my-skill\ndescription: A test skill used when validating fixtures.\ncompatibility: {long_compat}\n---\nBody\n"
            )
        ))
        .contains(&SkillDiagnosticCode::CompatibilityTooLong));
    }

    #[test]
    fn preserves_unquoted_yaml_scalars_as_strings() {
        let skill = valid(
            "my-skill",
            "---\nname: my-skill\ndescription: A test skill used when validating fixtures.\ncompatibility: 3.10\nmetadata:\n  author: example-org\n  version: 1.0\n  patch: 1.10\n  count: 123\n  enabled: true\n  quoted: \"1.0\"\n---\nBody\n",
        );
        assert_eq!(skill.manifest.compatibility.as_deref(), Some("3.10"));
        assert_eq!(
            skill.manifest.metadata.get("version").map(String::as_str),
            Some("1.0")
        );
        assert_eq!(
            skill.manifest.metadata.get("patch").map(String::as_str),
            Some("1.10")
        );
        assert_eq!(
            skill.manifest.metadata.get("count").map(String::as_str),
            Some("123")
        );
        assert_eq!(
            skill.manifest.metadata.get("enabled").map(String::as_str),
            Some("true")
        );
        assert_eq!(
            skill.manifest.metadata.get("quoted").map(String::as_str),
            Some("1.0")
        );
    }

    #[test]
    fn preserves_trailing_decimal_zero_yaml_number_lexemes() {
        let skill = valid(
            "my-skill",
            "---\nname: my-skill\ndescription: A test skill used when validating fixtures.\ncompatibility: 3.10\nlicense: 1.10\nmetadata:\n  version: 1.10 # keep trailing zero\n---\nBody\n",
        );
        assert_eq!(skill.manifest.compatibility.as_deref(), Some("3.10"));
        assert_eq!(skill.manifest.license.as_deref(), Some("1.10"));
        assert_eq!(
            skill.manifest.metadata.get("version").map(String::as_str),
            Some("1.10")
        );
        assert_ne!(
            skill.manifest.metadata.get("version").map(String::as_str),
            Some("1.1")
        );
    }

    #[test]
    fn accepts_plain_scalar_names_that_yaml_would_type() {
        let numbered = valid(
            "123",
            "---\nname: 123\ndescription: A test skill used when validating fixtures.\n---\nBody\n",
        );
        assert_eq!(numbered.manifest.name, "123");
        let flag = valid(
            "true",
            "---\nname: true\ndescription: A test skill used when validating fixtures.\n---\nBody\n",
        );
        assert_eq!(flag.manifest.name, "true");
    }

    #[test]
    fn preserves_yaml_null_lexemes_as_strings() {
        let skill = valid(
            "null",
            "---\nname: null\ndescription: A test skill used when validating YAML null lexemes.\nlicense:\nmetadata:\n  version: null\n  capital: Null\n  screaming: NULL\n  tilde: ~\n---\nBody\n",
        );
        assert_eq!(skill.manifest.name, "null");
        assert_eq!(skill.manifest.license, None);
        assert_eq!(
            skill.manifest.metadata.get("version").map(String::as_str),
            Some("null")
        );
        assert_eq!(
            skill.manifest.metadata.get("capital").map(String::as_str),
            Some("Null")
        );
        assert_eq!(
            skill.manifest.metadata.get("screaming").map(String::as_str),
            Some("NULL")
        );
        assert_eq!(
            skill.manifest.metadata.get("tilde").map(String::as_str),
            Some("~")
        );
        assert!(
            skill
                .warnings
                .iter()
                .any(|w| w.code == SkillWarningCode::MissingLicense)
        );

        let explicit = valid(
            "null",
            "---\nname: null\ndescription: A test skill used when validating YAML null lexemes.\nlicense: null\n---\nBody\n",
        );
        assert_eq!(explicit.manifest.license.as_deref(), Some("null"));
        assert!(
            !explicit
                .warnings
                .iter()
                .any(|w| w.code == SkillWarningCode::MissingLicense)
        );
    }

    #[test]
    fn rejects_capitalized_yaml_bool_name_as_not_lowercase() {
        let outcome = validate(
            "true",
            "---\nname: True\ndescription: A test skill used when validating fixtures.\n---\nBody\n",
        );
        assert!(
            codes(&outcome).contains(&SkillDiagnosticCode::NameNotLowercase),
            "name: True must keep the authored lexeme and fail lowercase, got {:?}",
            codes(&outcome)
        );
        assert!(!outcome.is_valid());

        for (parent, name) in [("true", "TRUE"), ("false", "False"), ("false", "FALSE")] {
            let outcome = validate(
                parent,
                &format!(
                    "---\nname: {name}\ndescription: A test skill used when validating fixtures.\n---\nBody\n"
                ),
            );
            assert!(
                codes(&outcome).contains(&SkillDiagnosticCode::NameNotLowercase),
                "name: {name} parent={parent} got {:?}",
                codes(&outcome)
            );
        }
    }

    #[test]
    fn rejects_capitalized_yaml_null_name_as_not_lowercase() {
        for name in ["Null", "NULL"] {
            let outcome = validate(
                "null",
                &format!(
                    "---\nname: {name}\ndescription: A test skill used when validating fixtures.\n---\nBody\n"
                ),
            );
            assert!(
                codes(&outcome).contains(&SkillDiagnosticCode::NameNotLowercase),
                "name: {name} must keep the authored lexeme and fail lowercase, got {:?}",
                codes(&outcome)
            );
            assert!(
                !codes(&outcome).contains(&SkillDiagnosticCode::NameNotString),
                "name: {name} must not be treated as a typed null, got {:?}",
                codes(&outcome)
            );
            assert!(!outcome.is_valid());
        }
    }

    #[test]
    fn quarantines_non_string_metadata_and_allowed_tools_list() {
        let meta = validate(
            "my-skill",
            "---\nname: my-skill\ndescription: A test skill used when validating fixtures.\nmetadata:\n  author: example-org\n  version:\n    nested: map\n---\nBody\n",
        );
        assert!(codes(&meta).contains(&SkillDiagnosticCode::MetadataValueNotString));
        let rendered = meta
            .diagnostics()
            .iter()
            .map(|d| format!("{} {} {}", d.stable_line(), d.message, d.remediation))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !rendered.contains("nested: map") && !rendered.contains("{"),
            "{rendered}"
        );

        let list_meta = validate(
            "my-skill",
            "---\nname: my-skill\ndescription: A test skill used when validating fixtures.\nmetadata:\n  version:\n    - first\n    - second\n---\nBody\n",
        );
        assert!(codes(&list_meta).contains(&SkillDiagnosticCode::MetadataValueNotString));
        assert!(
            !list_meta
                .diagnostics()
                .iter()
                .any(|d| d.message.contains("first") || d.stable_line().contains("first"))
        );

        let tools = validate(
            "my-skill",
            "---\nname: my-skill\ndescription: A test skill used when validating fixtures.\nallowed-tools:\n  - Bash\n  - Read\n---\nBody\n",
        );
        assert!(codes(&tools).contains(&SkillDiagnosticCode::AllowedToolsNotString));

        let name_list = validate(
            "my-skill",
            "---\nname:\n  - my-skill\ndescription: A test skill used when validating fixtures.\n---\nBody\n",
        );
        assert!(codes(&name_list).contains(&SkillDiagnosticCode::NameNotString));
    }

    #[test]
    fn accepts_nested_and_dotted_grok_extensions() {
        let nested = valid(
            "commit",
            "---\nname: commit\ndescription: Create well-formatted git commits. Use when the user wants to commit.\nmetadata:\n  author: xai\n  grok:\n    when-to-use: commit changes\n    paths:\n      - \"**/*.rs\"\n    argument-hint: commit message\n    user-invocable: true\n    disable-model-invocation: false\n---\nBody\n",
        );
        assert_eq!(
            nested.manifest.grok.when_to_use.as_deref(),
            Some("commit changes")
        );
        assert_eq!(
            nested.manifest.grok.paths.as_deref(),
            Some(["**/*.rs".to_string()].as_slice())
        );
        assert_eq!(nested.manifest.grok.user_invocable, Some(true));
        assert_eq!(
            nested.manifest.metadata.get("author").map(String::as_str),
            Some("xai")
        );
        assert!(!nested.manifest.metadata.contains_key("grok"));

        let dotted = valid(
            "commit",
            "---\nname: commit\ndescription: Create well-formatted git commits. Use when the user wants to commit.\nmetadata:\n  grok.when-to-use: commit changes\n  grok.paths: src/**/*.rs\n  grok.user-invocable: \"false\"\n---\nBody\n",
        );
        assert_eq!(
            dotted.manifest.grok.when_to_use.as_deref(),
            Some("commit changes")
        );
        assert_eq!(
            dotted.manifest.grok.paths.as_deref(),
            Some(["src/**/*.rs".to_string()].as_slice())
        );
        assert_eq!(dotted.manifest.grok.user_invocable, Some(false));

        let dotted_plain_bool = valid(
            "commit",
            "---\nname: commit\ndescription: Create well-formatted git commits. Use when the user wants to commit.\nmetadata:\n  grok.user-invocable: true\n---\nBody\n",
        );
        assert_eq!(dotted_plain_bool.manifest.grok.user_invocable, Some(true));
    }

    #[test]
    fn quarantines_invalid_grok_extension_values() {
        let bad_bool = validate(
            "commit",
            "---\nname: commit\ndescription: Create well-formatted git commits. Use when committing.\nmetadata:\n  grok:\n    user-invocable: yes\n---\nBody\n",
        );
        assert!(codes(&bad_bool).contains(&SkillDiagnosticCode::GrokExtensionInvalidValue));

        let bad_paths = validate(
            "commit",
            "---\nname: commit\ndescription: Create well-formatted git commits. Use when committing.\nmetadata:\n  grok:\n    paths:\n      nested: map\n---\nBody\n",
        );
        assert!(codes(&bad_paths).contains(&SkillDiagnosticCode::GrokExtensionInvalidValue));

        let unknown = validate(
            "commit",
            "---\nname: commit\ndescription: Create well-formatted git commits. Use when committing.\nmetadata:\n  grok.unknown-flag: true\n---\nBody\n",
        );
        assert!(codes(&unknown).contains(&SkillDiagnosticCode::GrokExtensionUnknownKey));

        let mixed = validate(
            "commit",
            "---\nname: commit\ndescription: Create well-formatted git commits. Use when committing.\nmetadata:\n  grok:\n    when-to-use: commit\n  grok.paths: src/**\n---\nBody\n",
        );
        assert!(codes(&mixed).contains(&SkillDiagnosticCode::GrokExtensionConflict));
    }

    #[test]
    fn does_not_leak_secrets_paths_or_raw_values() {
        let secret = "sk-live-SUPERSECRETVALUE";
        let outcome = validate(
            "leaky",
            &format!(
                "---\nname: leaky\ndescription: {secret}\nwhen-to-use: {secret}\nmetadata:\n  token: {secret}\n---\nBody contains {secret}\n"
            ),
        );
        let rendered = outcome
            .diagnostics()
            .iter()
            .map(|d| format!("{} {} {}", d.stable_line(), d.message, d.remediation))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!rendered.contains(secret), "{rendered}");
        assert!(!rendered.contains("SUPERSECRET"));
        assert!(!rendered.contains("/Users/"));
        assert!(!rendered.contains("home/"));
        assert!(outcome.diagnostics().iter().all(|d| d.position.line >= 1));
    }

    #[test]
    fn does_not_normalize_or_derive_from_body() {
        let spaced = validate(
            "my-cool-skill",
            "---\nname: My Cool Skill\ndescription: \"   \"\n---\nDoes a real thing.\n",
        );
        assert!(codes(&spaced).contains(&SkillDiagnosticCode::NameNotLowercase));
        assert!(codes(&spaced).contains(&SkillDiagnosticCode::EmptyDescription));

        let no_fm = validate("my-skill", "Just a body with no frontmatter.\n");
        assert!(codes(&no_fm).contains(&SkillDiagnosticCode::MissingFrontmatter));
    }

    #[test]
    fn authoring_warnings_do_not_quarantine() {
        let skill = valid("a", "---\nname: a\ndescription: short desc\n---\n");
        let warning_codes: Vec<_> = skill.warnings.iter().map(|w| w.code).collect();
        assert!(warning_codes.contains(&SkillWarningCode::ShortDescription));
        assert!(warning_codes.contains(&SkillWarningCode::EmptyBody));
        assert!(warning_codes.contains(&SkillWarningCode::MissingLicense));
        assert!(warning_codes.contains(&SkillWarningCode::MissingGrokWhenToUse));
    }

    #[test]
    fn dir_helper_rejects_missing_and_symlink_skill_md() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("missing-skill");
        std::fs::create_dir_all(&missing).unwrap();
        let outcome = validate_strict_skill_dir(&missing, Some(SkillScope::Local));
        assert!(codes(&outcome).contains(&SkillDiagnosticCode::MissingSkillMd));
        assert_eq!(
            match &outcome {
                StrictSkillOutcome::Quarantined(row) => row.identity.file_label.as_str(),
                StrictSkillOutcome::Valid(_) => "valid",
            },
            "missing-skill/SKILL.md"
        );

        let lower = tmp.path().join("lower-skill");
        std::fs::create_dir_all(&lower).unwrap();
        std::fs::write(
            lower.join("skill.md"),
            "---\nname: lower-skill\ndescription: x\n---\n",
        )
        .unwrap();
        assert!(
            codes(&validate_strict_skill_dir(&lower, None))
                .contains(&SkillDiagnosticCode::WrongSkillFileName)
        );

        #[cfg(unix)]
        {
            let linked = tmp.path().join("linked-skill");
            std::fs::create_dir_all(&linked).unwrap();
            let target = tmp.path().join("target.md");
            std::fs::write(&target, "---\nname: linked-skill\ndescription: A test skill used when validating fixtures.\n---\nBody\n").unwrap();
            std::os::unix::fs::symlink(&target, linked.join("SKILL.md")).unwrap();
            assert!(
                codes(&validate_strict_skill_dir(&linked, None))
                    .contains(&SkillDiagnosticCode::NotRegularFile)
            );
        }
    }

    #[test]
    fn inventory_collects_valid_and_quarantined_rows() {
        let good = valid(
            "ok-skill",
            "---\nname: ok-skill\ndescription: A valid official skill used in inventory fixtures.\n---\nBody\n",
        );
        let bad = match validate("bad-skill", "---\nname: bad-skill\n---\n") {
            StrictSkillOutcome::Quarantined(row) => row,
            StrictSkillOutcome::Valid(_) => panic!("expected quarantine"),
        };
        let inventory = SkillInventory::new(7, vec![good], vec![bad]);
        assert_eq!(inventory.spec_revision.len(), 40);
        assert_eq!(inventory.valid.len(), 1);
        assert_eq!(inventory.quarantined.len(), 1);
        assert!(inventory.is_stale(8, &inventory.fingerprint()));
    }

    #[test]
    fn discovery_source_routes_through_strict_ingest() {
        let src = include_str!("../discovery.rs");
        assert!(
            src.contains("ingest_skill_sources"),
            "runtime discovery must ingest through the strict validator"
        );
    }

    #[test]
    fn legacy_tolerant_parser_still_accepts_unofficial_frontmatter() {
        assert!(STRICT_VALIDATOR_RUNTIME_ENABLED);
        let parsed = parse_skill_frontmatter(
            "---\nname: My Cool Skill\ndescription: lorem ipsum: dolor\nwhen-to-use: trig\nallowed-tools:\n  - bash\n  - read_file\n---\nBody\n",
            Some("my-cool-skill"),
        )
        .unwrap();
        assert_eq!(parsed.name, "my-cool-skill");
        assert_eq!(parsed.when_to_use.as_deref(), Some("trig"));
        assert_eq!(
            parsed.allowed_tools.as_deref(),
            Some(["bash".to_string(), "read_file".to_string()].as_slice())
        );
        assert!(is_valid_skill_name(&normalize_skill_name("My Cool Skill")));

        let tmp = tempfile::tempdir().unwrap();
        let skill_dir = tmp.path().join("legacy");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "Just body, no frontmatter.\n").unwrap();
        let skills = parse_skill_files(vec![(skill_dir.join("SKILL.md"), SkillScope::Local)]);
        assert!(
            skills.is_empty(),
            "runtime ingest quarantines unofficial SKILL.md"
        );
    }

    #[test]
    fn wrong_file_name_is_rejected_without_reading_as_repair() {
        let outcome = validate_strict_skill(StrictSkillInput {
            file_name: "skill.md",
            parent_dir_name: "my-skill",
            content: "---\nname: my-skill\ndescription: A test skill used when validating fixtures.\n---\nBody\n",
            scope: None,
        });
        assert!(codes(&outcome).contains(&SkillDiagnosticCode::WrongSkillFileName));
    }
}
