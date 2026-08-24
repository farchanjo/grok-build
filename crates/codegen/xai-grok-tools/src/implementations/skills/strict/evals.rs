//! Bounded `evals/cases.yaml` schema and a deterministic offline local runner.
//!
//! The runner never contacts a network, embedding, reranking, or model
//! provider. Matching is local substring / path / pin evidence only. Results
//! are keyed by case id and skill identity; fingerprints mark stale state.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use super::inventory::SkillIdentity;
use super::status::SkillHealthStatus;
use crate::implementations::skills::types::SkillInfo;
use crate::util::hash::fnv1a_32;

pub const EVALS_SCHEMA_VERSION: u32 = 1;
pub const EVALS_CASES_FILE: &str = "evals/cases.yaml";
pub const MAX_CASES: usize = 32;
pub const MAX_CASE_ID_CHARS: usize = 64;
pub const MAX_QUERY_CHARS: usize = 512;
pub const MAX_RESOURCE_CHARS: usize = 256;
pub const MAX_PATH_PATTERN_CHARS: usize = 256;
pub const MAX_PEERS: usize = 8;
pub const MAX_RESULT_NOTES_CHARS: usize = 160;

/// Case kinds the local runner can evaluate offline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalCaseKind {
    ShouldTrigger,
    ShouldNotTrigger,
    ExplicitPin,
    PathTrigger,
    Resource,
    Conflict,
}

impl EvalCaseKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ShouldTrigger => "should_trigger",
            Self::ShouldNotTrigger => "should_not_trigger",
            Self::ExplicitPin => "explicit_pin",
            Self::PathTrigger => "path_trigger",
            Self::Resource => "resource",
            Self::Conflict => "conflict",
        }
    }
}

/// One bounded evaluation case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalCase {
    pub id: String,
    pub kind: EvalCaseKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub peers: Vec<String>,
}

/// Parsed `evals/cases.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalSuite {
    pub version: u32,
    pub cases: Vec<EvalCase>,
}

impl EvalSuite {
    pub fn fingerprint(&self) -> String {
        let mut buf = String::new();
        buf.push_str(&self.version.to_string());
        buf.push('\n');
        for case in &self.cases {
            buf.push_str(&case.id);
            buf.push('\t');
            buf.push_str(case.kind.as_str());
            buf.push('\t');
            buf.push_str(&case.query);
            buf.push('\t');
            if let Some(skill) = &case.skill {
                buf.push_str(skill);
            }
            buf.push('\t');
            if let Some(path) = &case.path {
                buf.push_str(path);
            }
            buf.push('\t');
            if let Some(resource) = &case.resource {
                buf.push_str(resource);
            }
            buf.push('\t');
            buf.push_str(&case.peers.join(","));
            buf.push('\n');
        }
        format!("{:08x}", fnv1a_32(buf.as_bytes()))
    }
}

/// Schema or parse failure. Messages never include raw YAML, paths, or bodies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalSchemaError {
    pub message: String,
}

impl EvalSchemaError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: bound_text(&message.into(), MAX_RESULT_NOTES_CHARS),
        }
    }
}

/// Parse and bound-check `evals/cases.yaml` bytes.
pub fn parse_eval_suite(bytes: &[u8]) -> Result<EvalSuite, EvalSchemaError> {
    if bytes.len() > 32 * 1024 {
        return Err(EvalSchemaError::new(
            "evals/cases.yaml exceeds the 32 KiB bound.",
        ));
    }
    let value: serde_yaml::Value = serde_yaml::from_slice(bytes)
        .map_err(|_| EvalSchemaError::new("evals/cases.yaml is not valid YAML."))?;
    parse_eval_suite_value(&value)
}

fn parse_eval_suite_value(value: &serde_yaml::Value) -> Result<EvalSuite, EvalSchemaError> {
    let mapping = value
        .as_mapping()
        .ok_or_else(|| EvalSchemaError::new("evals/cases.yaml root must be a mapping."))?;
    let mut seen_keys = BTreeSet::new();
    let mut version = None;
    let mut cases_value = None;
    for (key, val) in mapping {
        let Some(name) = key.as_str() else {
            return Err(EvalSchemaError::new(
                "evals/cases.yaml keys must be strings.",
            ));
        };
        if !seen_keys.insert(name.to_string()) {
            return Err(EvalSchemaError::new(
                "evals/cases.yaml contains a duplicate top-level key.",
            ));
        }
        match name {
            "version" => {
                version = Some(parse_version(val)?);
            }
            "cases" => {
                cases_value = Some(val);
            }
            _ => {
                return Err(EvalSchemaError::new(
                    "evals/cases.yaml contains an unknown top-level key.",
                ));
            }
        }
    }
    let version =
        version.ok_or_else(|| EvalSchemaError::new("evals/cases.yaml is missing version."))?;
    if version != EVALS_SCHEMA_VERSION {
        return Err(EvalSchemaError::new(
            "evals/cases.yaml version is not supported.",
        ));
    }
    let cases_value =
        cases_value.ok_or_else(|| EvalSchemaError::new("evals/cases.yaml is missing cases."))?;
    let cases = parse_cases(cases_value)?;
    Ok(EvalSuite { version, cases })
}

fn parse_version(value: &serde_yaml::Value) -> Result<u32, EvalSchemaError> {
    match value {
        serde_yaml::Value::Number(n) => n
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .ok_or_else(|| EvalSchemaError::new("version must be a positive integer.")),
        _ => Err(EvalSchemaError::new("version must be a positive integer.")),
    }
}

fn parse_cases(value: &serde_yaml::Value) -> Result<Vec<EvalCase>, EvalSchemaError> {
    let seq = value
        .as_sequence()
        .ok_or_else(|| EvalSchemaError::new("cases must be a list."))?;
    if seq.len() > MAX_CASES {
        return Err(EvalSchemaError::new("cases exceeds the 32-entry bound."));
    }
    let mut cases = Vec::with_capacity(seq.len());
    let mut ids = BTreeSet::new();
    for item in seq {
        let case = parse_case(item)?;
        if !ids.insert(case.id.clone()) {
            return Err(EvalSchemaError::new("case ids must be unique."));
        }
        cases.push(case);
    }
    Ok(cases)
}

fn parse_case(value: &serde_yaml::Value) -> Result<EvalCase, EvalSchemaError> {
    let mapping = value
        .as_mapping()
        .ok_or_else(|| EvalSchemaError::new("each case must be a mapping."))?;
    let mut id = None;
    let mut kind = None;
    let mut query = String::new();
    let mut skill = None;
    let mut path = None;
    let mut resource = None;
    let mut peers = Vec::new();
    for (key, val) in mapping {
        let Some(name) = key.as_str() else {
            return Err(EvalSchemaError::new("case keys must be strings."));
        };
        match name {
            "id" => id = Some(required_id(val)?),
            "kind" => kind = Some(required_kind(val)?),
            "query" => query = optional_bounded_string(val, MAX_QUERY_CHARS, "query")?,
            "skill" => {
                skill = Some(optional_bounded_string(
                    val,
                    super::spec::MAX_NAME_CHARS,
                    "skill",
                )?);
            }
            "path" => {
                path = Some(optional_bounded_string(
                    val,
                    MAX_PATH_PATTERN_CHARS,
                    "path",
                )?);
            }
            "resource" => {
                resource = Some(optional_bounded_string(
                    val,
                    MAX_RESOURCE_CHARS,
                    "resource",
                )?);
            }
            "peers" => peers = parse_peers(val)?,
            _ => {
                return Err(EvalSchemaError::new("case contains an unknown key."));
            }
        }
    }
    let id = id.ok_or_else(|| EvalSchemaError::new("case is missing id."))?;
    let kind = kind.ok_or_else(|| EvalSchemaError::new("case is missing kind."))?;
    if skill.as_ref().is_some_and(|s| s.is_empty()) {
        skill = None;
    }
    if path.as_ref().is_some_and(|s| s.is_empty()) {
        path = None;
    }
    if resource.as_ref().is_some_and(|s| s.is_empty()) {
        resource = None;
    }
    validate_case_shape(
        kind,
        &query,
        skill.as_deref(),
        path.as_deref(),
        resource.as_deref(),
        &peers,
    )?;
    Ok(EvalCase {
        id,
        kind,
        query,
        skill,
        path,
        resource,
        peers,
    })
}

fn validate_case_shape(
    kind: EvalCaseKind,
    query: &str,
    skill: Option<&str>,
    path: Option<&str>,
    resource: Option<&str>,
    peers: &[String],
) -> Result<(), EvalSchemaError> {
    match kind {
        EvalCaseKind::ShouldTrigger | EvalCaseKind::ShouldNotTrigger => {
            if query.is_empty() || skill.is_none() {
                return Err(EvalSchemaError::new(
                    "trigger cases require query and skill.",
                ));
            }
        }
        EvalCaseKind::ExplicitPin => {
            if skill.is_none() {
                return Err(EvalSchemaError::new("explicit_pin cases require skill."));
            }
        }
        EvalCaseKind::PathTrigger => {
            if path.is_none() || skill.is_none() {
                return Err(EvalSchemaError::new(
                    "path_trigger cases require path and skill.",
                ));
            }
        }
        EvalCaseKind::Resource => {
            if resource.is_none() || skill.is_none() {
                return Err(EvalSchemaError::new(
                    "resource cases require resource and skill.",
                ));
            }
        }
        EvalCaseKind::Conflict => {
            if query.is_empty() || peers.len() < 2 {
                return Err(EvalSchemaError::new(
                    "conflict cases require query and at least two peers.",
                ));
            }
        }
    }
    Ok(())
}

fn required_id(value: &serde_yaml::Value) -> Result<String, EvalSchemaError> {
    let raw = value
        .as_str()
        .ok_or_else(|| EvalSchemaError::new("case id must be a nonempty string."))?;
    if raw.is_empty() || raw.chars().count() > MAX_CASE_ID_CHARS {
        return Err(EvalSchemaError::new("case id must be 1 to 64 characters."));
    }
    if !raw
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        || raw.starts_with('-')
        || raw.ends_with('-')
        || raw.contains("--")
    {
        return Err(EvalSchemaError::new(
            "case id must use lowercase letters, digits, and hyphens.",
        ));
    }
    Ok(raw.to_string())
}

fn required_kind(value: &serde_yaml::Value) -> Result<EvalCaseKind, EvalSchemaError> {
    let raw = value
        .as_str()
        .ok_or_else(|| EvalSchemaError::new("case kind must be a string."))?;
    match raw {
        "should_trigger" => Ok(EvalCaseKind::ShouldTrigger),
        "should_not_trigger" => Ok(EvalCaseKind::ShouldNotTrigger),
        "explicit_pin" => Ok(EvalCaseKind::ExplicitPin),
        "path_trigger" => Ok(EvalCaseKind::PathTrigger),
        "resource" => Ok(EvalCaseKind::Resource),
        "conflict" => Ok(EvalCaseKind::Conflict),
        _ => Err(EvalSchemaError::new("case kind is not supported.")),
    }
}

fn optional_bounded_string(
    value: &serde_yaml::Value,
    max_chars: usize,
    field: &str,
) -> Result<String, EvalSchemaError> {
    let raw = value
        .as_str()
        .ok_or_else(|| EvalSchemaError::new(format!("{field} must be a string.")))?;
    if raw.chars().count() > max_chars {
        return Err(EvalSchemaError::new(format!(
            "{field} exceeds the character bound."
        )));
    }
    Ok(raw.to_string())
}

fn parse_peers(value: &serde_yaml::Value) -> Result<Vec<String>, EvalSchemaError> {
    let seq = value
        .as_sequence()
        .ok_or_else(|| EvalSchemaError::new("peers must be a list of strings."))?;
    if seq.len() > MAX_PEERS {
        return Err(EvalSchemaError::new("peers exceeds the 8-entry bound."));
    }
    let mut peers = Vec::with_capacity(seq.len());
    for item in seq {
        let name = optional_bounded_string(item, super::spec::MAX_NAME_CHARS, "peer")?;
        if name.is_empty() {
            return Err(EvalSchemaError::new("peer names must be nonempty."));
        }
        peers.push(name);
    }
    Ok(peers)
}

/// One keyed case result. No query text, bodies, or paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalCaseResult {
    pub id: String,
    pub passed: bool,
    pub order: u32,
}

/// Bounded, persistable local-regression report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalRunReport {
    pub schema_version: u32,
    pub generation: u64,
    pub inventory_fingerprint: String,
    pub cases_fingerprint: String,
    pub identity: SkillIdentity,
    pub status: SkillHealthStatus,
    pub results: BTreeMap<String, EvalCaseResult>,
    pub cancelled: bool,
    pub stable: bool,
}

impl EvalRunReport {
    /// Stale when inventory or live cases fingerprints disagree.
    /// Process-local generation is not a durability signal.
    pub fn is_stale(&self, inventory_fingerprint: &str, cases_fingerprint: &str) -> bool {
        self.inventory_fingerprint != inventory_fingerprint
            || self.cases_fingerprint != cases_fingerprint
    }

    /// Missing, unreadable, or changed `evals/cases.yaml` is stale.
    pub fn is_stale_vs_live(
        &self,
        inventory_fingerprint: &str,
        live_cases_fingerprint: Option<&str>,
    ) -> bool {
        match live_cases_fingerprint {
            Some(fp) => self.is_stale(inventory_fingerprint, fp),
            None => true,
        }
    }
}

/// Local matching surface for one valid skill. Never includes body or path.
#[derive(Debug, Clone)]
pub struct LocalSkillEvidence {
    pub name: String,
    pub description: String,
    pub when_to_use: Option<String>,
    pub paths: Vec<String>,
    pub short_description: Option<String>,
}

impl LocalSkillEvidence {
    pub fn from_skill_info(skill: &SkillInfo) -> Self {
        Self {
            name: skill.name.clone(),
            description: skill.description.clone(),
            when_to_use: skill.when_to_use.clone(),
            paths: skill.paths.clone().unwrap_or_default(),
            short_description: skill.short_description.clone(),
        }
    }

    fn matches_query(&self, query: &str) -> bool {
        let q = query.to_ascii_lowercase();
        if q.is_empty() {
            return false;
        }
        let name = self.name.to_ascii_lowercase();
        let desc = self.description.to_ascii_lowercase();
        let when = self
            .when_to_use
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase();
        let short = self
            .short_description
            .as_deref()
            .unwrap_or("")
            .to_ascii_lowercase();
        name.contains(&q) || desc.contains(&q) || when.contains(&q) || short.contains(&q)
    }

    fn matches_path(&self, path: &str) -> bool {
        self.paths
            .iter()
            .any(|pattern| path_glob_matches(pattern, path))
    }

    fn matches_resource(&self, resource: &str) -> bool {
        let r = resource.to_ascii_lowercase();
        self.description.to_ascii_lowercase().contains(&r)
            || self
                .when_to_use
                .as_deref()
                .unwrap_or("")
                .to_ascii_lowercase()
                .contains(&r)
            || self.name.to_ascii_lowercase().contains(&r)
    }
}

/// Run the suite twice for stable ordering. Cancels cooperatively.
pub fn run_eval_suite(
    suite: &EvalSuite,
    subject: &LocalSkillEvidence,
    peers: &[LocalSkillEvidence],
    identity: SkillIdentity,
    generation: u64,
    inventory_fingerprint: &str,
    cancel: &AtomicBool,
) -> EvalRunReport {
    let first = run_once(suite, subject, peers, cancel);
    let second = if cancel.load(Ordering::Relaxed) {
        first.clone()
    } else {
        run_once(suite, subject, peers, cancel)
    };
    let stable = first == second;
    let cancelled = cancel.load(Ordering::Relaxed);
    let mut results = BTreeMap::new();
    for (order, (id, passed)) in first.into_iter().enumerate() {
        results.insert(
            id.clone(),
            EvalCaseResult {
                id,
                passed,
                order: order as u32,
            },
        );
    }
    let all_passed = !cancelled && stable && results.values().all(|r| r.passed);
    let any_failed = results.values().any(|r| !r.passed);
    let status = if cancelled {
        SkillHealthStatus::Untested
    } else if results.is_empty() {
        SkillHealthStatus::Untested
    } else if !stable || any_failed {
        SkillHealthStatus::Failed
    } else if all_passed {
        SkillHealthStatus::ValidPass
    } else {
        SkillHealthStatus::Untested
    };
    EvalRunReport {
        schema_version: EVALS_SCHEMA_VERSION,
        generation,
        inventory_fingerprint: inventory_fingerprint.to_string(),
        cases_fingerprint: suite.fingerprint(),
        identity,
        status,
        results,
        cancelled,
        stable,
    }
}

fn run_once(
    suite: &EvalSuite,
    subject: &LocalSkillEvidence,
    peers: &[LocalSkillEvidence],
    cancel: &AtomicBool,
) -> Vec<(String, bool)> {
    let mut out = Vec::with_capacity(suite.cases.len());
    for case in &suite.cases {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        out.push((case.id.clone(), eval_case(case, subject, peers)));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn eval_case(case: &EvalCase, subject: &LocalSkillEvidence, peers: &[LocalSkillEvidence]) -> bool {
    match case.kind {
        EvalCaseKind::ShouldTrigger => {
            skill_named(subject, case.skill.as_deref()) && subject.matches_query(&case.query)
        }
        EvalCaseKind::ShouldNotTrigger => {
            skill_named(subject, case.skill.as_deref()) && !subject.matches_query(&case.query)
        }
        EvalCaseKind::ExplicitPin => skill_named(subject, case.skill.as_deref()),
        EvalCaseKind::PathTrigger => {
            skill_named(subject, case.skill.as_deref())
                && case
                    .path
                    .as_deref()
                    .is_some_and(|path| subject.matches_path(path))
        }
        EvalCaseKind::Resource => {
            skill_named(subject, case.skill.as_deref())
                && case
                    .resource
                    .as_deref()
                    .is_some_and(|resource| subject.matches_resource(resource))
        }
        EvalCaseKind::Conflict => {
            let mut hits = 0u32;
            for peer_name in &case.peers {
                let evidence = if skill_named(subject, Some(peer_name)) {
                    Some(subject)
                } else {
                    peers.iter().find(|p| p.name == *peer_name)
                };
                if evidence.is_some_and(|e| e.matches_query(&case.query)) {
                    hits += 1;
                }
            }
            hits <= 1
        }
    }
}

fn skill_named(subject: &LocalSkillEvidence, expected: Option<&str>) -> bool {
    expected.is_some_and(|name| subject.name == name)
}

/// Simple glob: exact, `*`, `**`, prefix `dir/**`, and suffix `**/file`.
fn path_glob_matches(pattern: &str, path: &str) -> bool {
    if pattern == path || pattern == "**" || pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path == prefix || path.starts_with(&format!("{prefix}/"));
    }
    if let Some(suffix) = pattern.strip_prefix("**/") {
        return path == suffix || path.ends_with(&format!("/{suffix}"));
    }
    if let Some(prefix) = pattern.strip_suffix("/*") {
        return path
            .strip_prefix(&format!("{prefix}/"))
            .is_some_and(|rest| !rest.is_empty() && !rest.contains('/'));
    }
    false
}

/// Persist a bounded keyed report under `$GROK_HOME/skill-regress/`.
///
/// Never stores under `$GROK_HOME/skills/` so a skill named `regress` cannot
/// collide with result files. Writes go through a temp file + rename.
pub fn persist_eval_report(
    grok_home: &Path,
    report: &EvalRunReport,
) -> Result<(), EvalSchemaError> {
    let dir = grok_home.join("skill-regress");
    ensure_real_regress_dir(&dir)?;
    let key = regression_store_key(&report.identity);
    if key.is_empty() {
        return Err(EvalSchemaError::new("regression identity is missing."));
    }
    let path = dir.join(format!("{key}.json"));
    if let Ok(meta) = std::fs::symlink_metadata(&path)
        && (meta.file_type().is_symlink() || !meta.is_file())
    {
        return Err(EvalSchemaError::new(
            "regression results must be a regular file.",
        ));
    }
    let bytes = serde_json::to_vec(report)
        .map_err(|_| EvalSchemaError::new("regression results could not be encoded."))?;
    let tmp = dir.join(format!(
        ".{key}.tmp-{}-{}",
        std::process::id(),
        persist_seq().fetch_add(1, Ordering::Relaxed)
    ));
    if std::fs::write(&tmp, &bytes).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return Err(EvalSchemaError::new(
            "regression results could not be stored.",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    if std::fs::rename(&tmp, &path).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return Err(EvalSchemaError::new(
            "regression results could not be stored.",
        ));
    }
    Ok(())
}

/// Load a previously persisted report. Missing files are `Ok(None)`.
pub fn load_eval_report(
    grok_home: &Path,
    identity: &SkillIdentity,
) -> Result<Option<EvalRunReport>, EvalSchemaError> {
    let key = regression_store_key(identity);
    if key.is_empty() {
        return Ok(None);
    }
    let path = grok_home.join("skill-regress").join(format!("{key}.json"));
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(_) => return Ok(None),
    };
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(EvalSchemaError::new(
            "regression results must be a regular file.",
        ));
    }
    let bytes = std::fs::read(&path)
        .map_err(|_| EvalSchemaError::new("regression results could not be read."))?;
    let report = serde_json::from_slice(&bytes)
        .map_err(|_| EvalSchemaError::new("regression results are not valid JSON."))?;
    Ok(Some(report))
}

/// Bounded `{scope}-{name}` token used for result files and in-flight jobs.
pub fn regression_store_key(identity: &SkillIdentity) -> String {
    let scope = match identity.scope {
        Some(crate::implementations::skills::types::SkillScope::Local) => "local",
        Some(crate::implementations::skills::types::SkillScope::Repo) => "repo",
        Some(crate::implementations::skills::types::SkillScope::User) => "user",
        Some(crate::implementations::skills::types::SkillScope::Server) => "server",
        Some(crate::implementations::skills::types::SkillScope::Bundled) => "bundled",
        Some(crate::implementations::skills::types::SkillScope::Plugin) => "plugin",
        None => "unscoped",
    };
    let scope = sanitize_result_key(scope);
    let name = sanitize_result_key(&identity.parent_dir_name);
    if scope.is_empty() || name.is_empty() {
        String::new()
    } else {
        format!("{scope}-{name}")
    }
}

/// True when `key` is the persist/job token for `identity`.
///
/// A request without scope matches every scoped job for the same name so
/// cancel still works when the TUI sends only `skill.name`.
pub fn regression_key_matches(key: &str, identity: &SkillIdentity) -> bool {
    let exact = regression_store_key(identity);
    if exact.is_empty() {
        return false;
    }
    if key == exact {
        return true;
    }
    identity.scope.is_none() && scope_prefixed_name(key) == Some(identity.parent_dir_name.as_str())
}

fn scope_prefixed_name(key: &str) -> Option<&str> {
    const SCOPES: &[&str] = &[
        "unscoped", "bundled", "plugin", "server", "local", "repo", "user",
    ];
    for scope in SCOPES {
        if let Some(name) = key.strip_prefix(scope)
            && let Some(name) = name.strip_prefix('-')
            && !name.is_empty()
        {
            return Some(name);
        }
    }
    None
}

fn persist_seq() -> &'static std::sync::atomic::AtomicU64 {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    &SEQ
}

fn ensure_real_regress_dir(dir: &Path) -> Result<(), EvalSchemaError> {
    match std::fs::symlink_metadata(dir) {
        Ok(meta) if meta.file_type().is_symlink() => Err(EvalSchemaError::new(
            "regression results could not be stored.",
        )),
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(_) => Err(EvalSchemaError::new(
            "regression results could not be stored.",
        )),
        Err(_) => {
            std::fs::create_dir_all(dir)
                .map_err(|_| EvalSchemaError::new("regression results could not be stored."))?;
            let meta = std::fs::symlink_metadata(dir)
                .map_err(|_| EvalSchemaError::new("regression results could not be stored."))?;
            if meta.file_type().is_symlink() || !meta.is_dir() {
                return Err(EvalSchemaError::new(
                    "regression results could not be stored.",
                ));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
            }
            Ok(())
        }
    }
}

fn sanitize_result_key(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
        .take(64)
        .collect()
}

/// Fingerprint of the live `evals/cases.yaml`. Missing or unreadable → `None`.
pub fn live_cases_fingerprint(skill_dir: &Path) -> Option<String> {
    match load_eval_suite_from_dir(skill_dir) {
        Ok(Some(suite)) => Some(suite.fingerprint()),
        _ => None,
    }
}

/// Load `evals/cases.yaml` from a skill directory without following the file
/// or an intermediate `evals` directory symlink.
pub fn load_eval_suite_from_dir(dir: &Path) -> Result<Option<EvalSuite>, EvalSchemaError> {
    let evals_dir = dir.join("evals");
    let evals_meta = match std::fs::symlink_metadata(&evals_dir) {
        Ok(meta) => meta,
        Err(_) => return Ok(None),
    };
    if evals_meta.file_type().is_symlink() || !evals_meta.is_dir() {
        return Err(EvalSchemaError::new(
            "evals must be a regular directory. Symlinks are rejected.",
        ));
    }
    let path = evals_dir.join("cases.yaml");
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(_) => return Ok(None),
    };
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(EvalSchemaError::new(
            "evals/cases.yaml must be a regular file. Symlinks are rejected.",
        ));
    }
    let bytes = std::fs::read(&path)
        .map_err(|_| EvalSchemaError::new("evals/cases.yaml could not be read."))?;
    parse_eval_suite(&bytes).map(Some)
}

fn bound_text(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (i, ch) in text.chars().enumerate() {
        if i >= max_chars {
            break;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::skills::types::SkillScope;

    fn suite(yaml: &str) -> EvalSuite {
        parse_eval_suite(yaml.as_bytes()).expect("suite")
    }

    fn subject(name: &str, description: &str) -> LocalSkillEvidence {
        LocalSkillEvidence {
            name: name.to_string(),
            description: description.to_string(),
            when_to_use: Some(format!("Use when running {name}")),
            paths: vec!["src/**".into(), "Cargo.toml".into()],
            short_description: Some(name.to_string()),
        }
    }

    #[test]
    fn rejects_unknown_kind_and_duplicate_ids() {
        let err = parse_eval_suite(
            b"version: 1\ncases:\n  - id: a\n    kind: network\n    query: x\n    skill: a\n",
        )
        .unwrap_err();
        assert!(err.message.contains("kind"));
        let err = parse_eval_suite(
            b"version: 1\ncases:\n  - id: a\n    kind: explicit_pin\n    skill: a\n  - id: a\n    kind: explicit_pin\n    skill: a\n",
        )
        .unwrap_err();
        assert!(err.message.contains("unique"));
    }

    #[test]
    fn rejects_unknown_top_level_key() {
        let err = parse_eval_suite(b"version: 1\ncases: []\nprompt: secret\n").unwrap_err();
        assert!(err.message.contains("unknown"));
        assert!(!err.message.contains("secret"));
    }

    #[test]
    fn trigger_and_pin_and_path_and_resource_and_conflict() {
        let yaml = r#"
version: 1
cases:
  - id: trigger-commit
    kind: should_trigger
    query: commit
    skill: commit
  - id: no-deploy
    kind: should_not_trigger
    query: deploy production
    skill: commit
  - id: pin
    kind: explicit_pin
    skill: commit
  - id: cargo-path
    kind: path_trigger
    skill: commit
    path: Cargo.toml
  - id: git-resource
    kind: resource
    skill: commit
    resource: git
  - id: no-conflict
    kind: conflict
    query: commit
    peers: [commit, deploy]
"#;
        let suite = suite(yaml);
        let deploy = subject("deploy", "Ship the service");
        let report = run_eval_suite(
            &suite,
            &subject("commit", "Create well-formatted git commits"),
            &[deploy],
            SkillIdentity::new("commit", Some(SkillScope::Local)),
            7,
            "abc",
            &AtomicBool::new(false),
        );
        assert!(report.stable);
        assert!(!report.cancelled);
        let failed: Vec<&str> = report
            .results
            .values()
            .filter(|r| !r.passed)
            .map(|r| r.id.as_str())
            .collect();
        assert!(
            failed.is_empty(),
            "unexpected failures: {failed:?} status={:?}",
            report.status
        );
        assert_eq!(report.status, SkillHealthStatus::ValidPass);
        assert_eq!(report.results.len(), 6);
        assert!(report.results.values().all(|r| r.passed));
        let order: Vec<&str> = report
            .results
            .values()
            .map(|r| (r.order, r.id.as_str()))
            .fold(vec![""; 6], |mut acc, (order, id)| {
                acc[order as usize] = id;
                acc
            });
        let mut expected = order.clone();
        expected.sort();
        assert_eq!(order, expected, "results are stored in stable id order");
    }

    #[test]
    fn negative_trigger_and_conflict_failures() {
        let yaml = r#"
version: 1
cases:
  - id: should-not
    kind: should_not_trigger
    query: commit
    skill: commit
  - id: clash
    kind: conflict
    query: commit
    peers: [commit, git-commit]
"#;
        let suite = suite(yaml);
        let peer = subject("git-commit", "Also handles commit messages");
        let report = run_eval_suite(
            &suite,
            &subject("commit", "Create git commits"),
            &[peer],
            SkillIdentity::new("commit", None),
            1,
            "fp",
            &AtomicBool::new(false),
        );
        assert_eq!(report.status, SkillHealthStatus::Failed);
        assert!(!report.results["should-not"].passed);
        assert!(!report.results["clash"].passed);
    }

    #[test]
    fn cancel_marks_untested_and_is_non_destructive() {
        let yaml = r#"
version: 1
cases:
  - id: pin
    kind: explicit_pin
    skill: commit
"#;
        let suite = suite(yaml);
        let cancel = AtomicBool::new(true);
        let report = run_eval_suite(
            &suite,
            &subject("commit", "Create git commits"),
            &[],
            SkillIdentity::new("commit", None),
            1,
            "fp",
            &cancel,
        );
        assert!(report.cancelled);
        assert_eq!(report.status, SkillHealthStatus::Untested);
    }

    #[test]
    fn stale_when_any_fingerprint_or_generation_changes() {
        let suite =
            suite("version: 1\ncases:\n  - id: pin\n    kind: explicit_pin\n    skill: commit\n");
        let report = run_eval_suite(
            &suite,
            &subject("commit", "d"),
            &[],
            SkillIdentity::new("commit", None),
            3,
            "inv-a",
            &AtomicBool::new(false),
        );
        assert!(report.is_stale("inv-b", &suite.fingerprint()));
        assert!(report.is_stale("inv-a", "other"));
        assert!(!report.is_stale("inv-a", &suite.fingerprint()));
        assert!(
            !report.is_stale_vs_live("inv-a", Some(&suite.fingerprint())),
            "process-local generation must not mark a matching report stale"
        );
        assert!(report.is_stale_vs_live("inv-a", None));
        assert!(!serde_json::to_string(&report).unwrap().contains("Create"));
        assert!(!serde_json::to_string(&report).unwrap().contains("/Users"));
    }

    #[test]
    fn persist_and_reload_keyed_report_without_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let suite =
            suite("version: 1\ncases:\n  - id: pin\n    kind: explicit_pin\n    skill: commit\n");
        let report = run_eval_suite(
            &suite,
            &subject("commit", "d"),
            &[],
            SkillIdentity::new("commit", None),
            9,
            "inv",
            &AtomicBool::new(false),
        );
        persist_eval_report(tmp.path(), &report).unwrap();
        let loaded = load_eval_report(tmp.path(), &SkillIdentity::new("commit", None))
            .unwrap()
            .unwrap();
        assert_eq!(loaded.generation, 9);
        assert_eq!(loaded.status, SkillHealthStatus::ValidPass);
        let json = serde_json::to_string(&loaded).unwrap();
        assert!(!json.contains(tmp.path().to_string_lossy().as_ref()));
        assert!(
            tmp.path()
                .join("skill-regress/unscoped-commit.json")
                .is_file()
        );
        assert!(!tmp.path().join("skills/regress").exists());
    }

    #[test]
    fn persist_keys_user_and_repo_reports_independently() {
        let tmp = tempfile::tempdir().unwrap();
        let suite =
            suite("version: 1\ncases:\n  - id: pin\n    kind: explicit_pin\n    skill: commit\n");
        let user = run_eval_suite(
            &suite,
            &subject("commit", "d"),
            &[],
            SkillIdentity::new("commit", Some(SkillScope::User)),
            2,
            "inv-user",
            &AtomicBool::new(false),
        );
        let mut repo = user.clone();
        repo.identity = SkillIdentity::new("commit", Some(SkillScope::Repo));
        repo.inventory_fingerprint = "inv-repo".into();
        persist_eval_report(tmp.path(), &user).unwrap();
        persist_eval_report(tmp.path(), &repo).unwrap();
        let loaded_user = load_eval_report(
            tmp.path(),
            &SkillIdentity::new("commit", Some(SkillScope::User)),
        )
        .unwrap()
        .unwrap();
        let loaded_repo = load_eval_report(
            tmp.path(),
            &SkillIdentity::new("commit", Some(SkillScope::Repo)),
        )
        .unwrap()
        .unwrap();
        assert_eq!(loaded_user.inventory_fingerprint, "inv-user");
        assert_eq!(loaded_repo.inventory_fingerprint, "inv-repo");
        assert!(tmp.path().join("skill-regress/user-commit.json").is_file());
        assert!(tmp.path().join("skill-regress/repo-commit.json").is_file());
        assert!(!tmp.path().join("skills").exists());
    }

    #[test]
    fn live_cases_fingerprint_marks_missing_and_changed_stale() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("commit");
        std::fs::create_dir_all(dir.join("evals")).unwrap();
        let yaml = "version: 1\ncases:\n  - id: pin\n    kind: explicit_pin\n    skill: commit\n";
        std::fs::write(dir.join("evals/cases.yaml"), yaml).unwrap();
        let suite = suite(yaml);
        let report = run_eval_suite(
            &suite,
            &subject("commit", "d"),
            &[],
            SkillIdentity::new("commit", Some(SkillScope::User)),
            4,
            "inv",
            &AtomicBool::new(false),
        );
        let live = live_cases_fingerprint(&dir).unwrap();
        assert!(!report.is_stale_vs_live("inv", Some(&live)));
        std::fs::write(
            dir.join("evals/cases.yaml"),
            "version: 1\ncases:\n  - id: pin2\n    kind: explicit_pin\n    skill: commit\n",
        )
        .unwrap();
        let changed = live_cases_fingerprint(&dir).unwrap();
        assert_ne!(live, changed);
        assert!(report.is_stale_vs_live("inv", Some(&changed)));
        std::fs::remove_file(dir.join("evals/cases.yaml")).unwrap();
        assert!(live_cases_fingerprint(&dir).is_none());
        assert!(report.is_stale_vs_live("inv", live_cases_fingerprint(&dir).as_deref()));
    }

    #[test]
    fn regression_store_key_includes_scope() {
        let user = SkillIdentity::new("commit", Some(SkillScope::User));
        let repo = SkillIdentity::new("commit", Some(SkillScope::Repo));
        assert_eq!(regression_store_key(&user), "user-commit");
        assert_eq!(regression_store_key(&repo), "repo-commit");
        assert_ne!(regression_store_key(&user), regression_store_key(&repo));
        assert!(regression_key_matches("user-commit", &user));
        assert!(!regression_key_matches("repo-commit", &user));
        let unscoped = SkillIdentity::new("commit", None);
        assert!(regression_key_matches("user-commit", &unscoped));
        assert!(regression_key_matches("repo-commit", &unscoped));
        assert!(!regression_key_matches("user-deploy", &unscoped));
    }

    #[test]
    fn load_rejects_symlink_cases_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("commit");
        std::fs::create_dir_all(dir.join("evals")).unwrap();
        std::fs::write(tmp.path().join("secret.yaml"), "version: 1\ncases: []\n").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                tmp.path().join("secret.yaml"),
                dir.join("evals/cases.yaml"),
            )
            .unwrap();
            let err = load_eval_suite_from_dir(&dir).unwrap_err();
            assert!(err.message.contains("regular file"));
            assert!(!err.message.contains("secret"));
        }
    }

    #[test]
    fn load_missing_evals_dir_is_no_suite() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("commit");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(load_eval_suite_from_dir(&dir).unwrap().is_none());
    }

    #[test]
    fn load_rejects_evals_non_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("commit");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("evals"), "not-a-directory").unwrap();
        let err = load_eval_suite_from_dir(&dir).unwrap_err();
        assert!(err.message.contains("directory"));
        assert!(err.message.len() <= MAX_RESULT_NOTES_CHARS);
        assert!(!err.message.contains("not-a-directory"));
    }

    #[cfg(unix)]
    #[test]
    fn load_rejects_evals_dir_symlink_without_parsing_outside_cases() {
        let tmp = tempfile::tempdir().unwrap();
        let skill_dir = tmp.path().join("commit");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let outside = tmp.path().join("outside-evals");
        std::fs::create_dir_all(&outside).unwrap();
        let secret_yaml = "version: 1\ncases:\n  - id: secret-outside-pin\n    kind: explicit_pin\n    skill: leak\n";
        std::fs::write(outside.join("cases.yaml"), secret_yaml).unwrap();
        std::os::unix::fs::symlink(&outside, skill_dir.join("evals")).unwrap();
        let err = load_eval_suite_from_dir(&skill_dir).unwrap_err();
        assert!(err.message.contains("directory"));
        assert!(err.message.len() <= MAX_RESULT_NOTES_CHARS);
        assert!(!err.message.contains("secret"));
        assert!(!err.message.contains("outside"));
        assert!(!err.message.contains("leak"));
    }
}
