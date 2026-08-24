//! Canonical strict Agent Skills contracts.
//!
//! This module pins an immutable revision of the Apache-2.0 Agent Skills
//! specification/`skills-ref` behavior and exposes shared manifest,
//! diagnostic, warning, discovered-skill, quarantined-skill, and inventory
//! types. Runtime discovery routes every SKILL.md source through
//! [`validate_strict_skill`]; only valid rows may feed SkillManager,
//! advertisement, invocation, preload, or Prime.
//!
//! The pinned repository is never fetched at runtime.

mod author;
mod diagnostic;
mod evals;
mod inventory;
mod management;
mod manifest;
mod publish;
mod runtime;
mod spec;
mod status;
mod validator;

pub use author::{
    AuthorCheckReport, AuthorKind, check_author_skill_content, check_author_skill_dir,
    content_has_legacy_top_level_grok_key, secret_leak_tokens, text_leaks_secrets,
};
pub use diagnostic::{
    DiagnosticPosition, SkillAuthoringWarning, SkillDiagnostic, SkillDiagnosticCode,
    SkillWarningCode,
};
pub use evals::{
    EVALS_CASES_FILE, EVALS_SCHEMA_VERSION, EvalCase, EvalCaseKind, EvalCaseResult, EvalRunReport,
    EvalSchemaError, EvalSuite, LocalSkillEvidence, live_cases_fingerprint, load_eval_report,
    load_eval_suite_from_dir, parse_eval_suite, persist_eval_report, regression_key_matches,
    regression_store_key, run_eval_suite,
};
pub use inventory::{DiscoveredSkill, QuarantinedSkill, SkillIdentity, SkillInventory};
pub use management::{
    ManagedSkillRow, SKILLS_API_VERSION, SkillRegressionSummary, SkillsListV1Response,
    SkillsPublishResponse, SkillsRegressStatusResponse, SkillsValidateResponse, SkillsVersionError,
    SkillsVersionedRequest, build_managed_rows, require_api_version,
};
pub use manifest::{GrokSkillExtensions, StrictSkillManifest};
pub use publish::{
    PublishError, PublishResult, PublishScope, dest_parent_for_scope, publish_skill_directory,
    render_skill_md, validate_complete_skill_directory,
};
pub use runtime::{
    LegacyCommand, RevalidatedSkillFile, SkillLoadError, SkillSourceReport, ingest_skill_sources,
    is_legacy_command_path, is_real_directory, is_regular_file, is_skill_md_path,
    parse_legacy_command_file, revalidate_skill_at_load, revalidate_skill_file_at_load,
    set_after_nofollow_read_hook, set_before_walk_root_open_hook, skill_info_from_discovered,
    skill_matches_toggle, skill_qualified_identity, stamp_collection_root,
};
pub use spec::{
    AGENTSKILLS_SPEC_REPOSITORY, AGENTSKILLS_SPEC_REVISION, GROK_EXTENSION_KEYS,
    GROK_EXTENSION_OBJECT_KEY, GROK_EXTENSION_PREFIX, LEGACY_GROK_TOP_LEVEL_KEYS,
    MAX_COMPATIBILITY_CHARS, MAX_DESCRIPTION_CHARS, MAX_NAME_CHARS, OFFICIAL_TOP_LEVEL_KEYS,
    SKILL_MD_FILE_NAME, STRICT_VALIDATOR_RUNTIME_ENABLED, is_official_publishable_name, nfkc,
};
pub use status::{SkillHealthStatus, SkillsHealthHeader};
pub use validator::{
    StrictSkillInput, StrictSkillOutcome, validate_strict_skill, validate_strict_skill_dir,
};

#[cfg(test)]
mod acp_fixtures;
#[cfg(test)]
mod parity;
