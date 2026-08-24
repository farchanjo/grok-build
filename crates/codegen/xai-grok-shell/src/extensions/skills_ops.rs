//! Versioned skill validate/publish/update/regress operations.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use tokio_util::sync::CancellationToken;

use serde::Deserialize;
use xai_grok_agent::prompt::skills::{CompatConfig, SkillListing, list_skill_sources_with_plugins};
use xai_grok_tools::implementations::grok_build::publish_from_fields;
use xai_grok_tools::implementations::skills::strict::{
    EvalRunReport, LocalSkillEvidence, PublishScope, SKILLS_API_VERSION, SkillHealthStatus,
    SkillIdentity, SkillRegressionSummary, SkillsListV1Response, SkillsPublishResponse,
    SkillsRegressStatusResponse, SkillsValidateResponse, SkillsVersionError, StrictSkillOutcome,
    build_managed_rows, dest_parent_for_scope, live_cases_fingerprint, load_eval_report,
    load_eval_suite_from_dir, persist_eval_report, publish_skill_directory, regression_key_matches,
    regression_store_key, require_api_version, run_eval_suite, validate_strict_skill_dir,
};
use xai_grok_tools::implementations::skills::types::{SkillInfo, SkillScope};
use xai_grok_tools::util::grok_home::grok_home;

use super::ExtResult;

static INVENTORY_GENERATION: AtomicU64 = AtomicU64::new(1);
static RUN_TOKEN: AtomicU64 = AtomicU64::new(1);

fn jobs() -> &'static Mutex<HashMap<String, RegressionJob>> {
    static JOBS: OnceLock<Mutex<HashMap<String, RegressionJob>>> = OnceLock::new();
    JOBS.get_or_init(|| Mutex::new(HashMap::new()))
}

struct RegressionJob {
    generation: u64,
    run_token: u64,
    cancel: Arc<AtomicBool>,
    running: AtomicBool,
}

/// Bidirectional job-key match so a scoped listing identity still finds a
/// provisional unscoped job registered before `load_listing` returns.
fn job_matches(key: &str, identity: &SkillIdentity) -> bool {
    if regression_key_matches(key, identity) {
        return true;
    }
    if identity.scope.is_some() {
        let unscoped = SkillIdentity::new(identity.parent_dir_name.as_str(), None);
        return key == regression_store_key(&unscoped);
    }
    false
}

struct RegisteredJob {
    key: String,
    token: u64,
}

impl Drop for RegisteredJob {
    fn drop(&mut self) {
        remove_job_if_token(&self.key, self.token);
    }
}

impl RegisteredJob {
    fn rekey(&mut self, new_key: String) {
        if new_key == self.key {
            return;
        }
        let mut guard = jobs()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(job) = guard.remove(&self.key) {
            if job.run_token == self.token {
                guard.insert(new_key.clone(), job);
                self.key = new_key;
            } else {
                guard.insert(self.key.clone(), job);
            }
        }
    }
}

#[derive(Debug)]
enum RegisterError {
    AlreadyRunning,
}

fn next_run_token() -> u64 {
    RUN_TOKEN.fetch_add(1, Ordering::Relaxed).saturating_add(1)
}

fn remove_job_if_token(key: &str, token: u64) {
    let mut guard = jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match guard.get(key) {
        Some(job) if job.run_token == token => {
            guard.remove(key);
        }
        _ => {}
    }
}

/// Insert a running job before any await so `regress/cancel` during listing
/// observes `running=true` and can set the in-flight cancel flag. Idle
/// `regress/cancel` is a no-op and must not poison the next run.
fn register_run(
    identity: &SkillIdentity,
    generation: u64,
) -> Result<(RegisteredJob, Arc<AtomicBool>), RegisterError> {
    let token = next_run_token();
    let key = regression_store_key(identity);
    let mut guard = jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if guard.iter().any(|(existing, job)| {
        job_matches(existing, identity) && job.running.load(Ordering::Relaxed)
    }) {
        return Err(RegisterError::AlreadyRunning);
    }
    let matching: Vec<String> = guard
        .keys()
        .filter(|existing| job_matches(existing, identity))
        .cloned()
        .collect();
    for existing in matching {
        guard.remove(&existing);
    }
    let cancel_flag = Arc::new(AtomicBool::new(false));
    guard.insert(
        key.clone(),
        RegressionJob {
            generation,
            run_token: token,
            cancel: Arc::clone(&cancel_flag),
            running: AtomicBool::new(true),
        },
    );
    Ok((RegisteredJob { key, token }, cancel_flag))
}

/// Set cancel only on matching in-flight jobs. Idle cancel (no running job)
/// is a no-op and must not record a durable cancel-intent placeholder.
fn request_cancel(identity: &SkillIdentity) {
    let guard = jobs()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for (key, job) in guard.iter() {
        if job_matches(key, identity) && job.running.load(Ordering::Relaxed) {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }
}

fn current_generation() -> u64 {
    INVENTORY_GENERATION.load(Ordering::Relaxed)
}

fn bump_generation() -> u64 {
    INVENTORY_GENERATION
        .fetch_add(1, Ordering::Relaxed)
        .saturating_add(1)
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionedCwdRequest {
    #[serde(default)]
    pub api_version: Option<u32>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub expected_generation: Option<u64>,
    /// When true, configured-profile regression may use the shipped
    /// embedding route. Default false: local-only, never contact the route.
    #[serde(default)]
    pub confirm_configured_profile: bool,
}

fn version_error(err: SkillsVersionError) -> ExtResult {
    super::to_ext_response(Err::<serde_json::Value, _>(anyhow::anyhow!(err.message())))
}

pub fn mixed_version_error(method: &str, api_version: Option<u32>) -> Option<ExtResult> {
    if method == "x.ai/skills/list" {
        return None;
    }
    match require_api_version(api_version) {
        Ok(_) => None,
        Err(err) => Some(version_error(err)),
    }
}

pub async fn list_v1(
    cwd: &str,
    plugin_registry: Option<&xai_grok_agent::plugins::PluginRegistry>,
    compat: CompatConfig,
) -> SkillsListV1Response {
    let listing = load_listing(cwd, plugin_registry, compat).await;
    let grok = grok_home();
    let generation = current_generation().max(listing.inventory.generation);
    let inventory_fp = listing.inventory.fingerprint();
    let rows = build_managed_rows(&listing.inventory, &listing.skills, &|identity| {
        load_eval_report(&grok, identity)
            .ok()
            .flatten()
            .map(|report| {
                regression_summary_from_report(
                    &report,
                    &inventory_fp,
                    skill_dir_for(&listing, identity).as_deref(),
                )
            })
    });
    SkillsListV1Response::from_rows(generation, inventory_fp, rows)
}

/// Profile id and `[prime.skills].min_score` consumer floor for Smart search.
/// Profile `min_score` stays inside `stricter_threshold`; callers must not
/// hardcode `0.0`.
fn smart_search_profile_and_consumer_floor(
    service: Option<&crate::retrieval::RetrievalService>,
) -> (Option<String>, f32) {
    match service {
        Some(service) => smart_search_profile_and_consumer_floor_from_skills(
            &service.load_snapshot().prime.skills,
        ),
        None => (None, 0.0),
    }
}

fn smart_search_profile_and_consumer_floor_from_skills(
    skills: &xai_grok_config_types::SkillPrimeConfig,
) -> (Option<String>, f32) {
    (skills.retrieval_profile.clone(), skills.min_score)
}

/// Bounded workspace inventory for Smart search, matching prime's
/// `PrimeInput.inventory` path. A failed walk still records `cwd` as root
/// so path-glob scoring is not pointed at an empty default.
fn workspace_inventory_from_cwd(
    cwd: &Path,
) -> crate::session::prime::inventory::WorkspaceInventory {
    match crate::session::prime::inventory::build_inventory(
        cwd,
        crate::session::prime::inventory::InventoryLimits::default(),
    ) {
        Ok(inv) => inv,
        Err(_) => crate::session::prime::inventory::WorkspaceInventory {
            root: cwd.to_path_buf(),
            ..Default::default()
        },
    }
}

async fn workspace_inventory_for_search(
    cwd: &Path,
) -> crate::session::prime::inventory::WorkspaceInventory {
    let root = cwd.to_path_buf();
    tokio::task::spawn_blocking(move || workspace_inventory_from_cwd(&root))
        .await
        .unwrap_or_else(|_| crate::session::prime::inventory::WorkspaceInventory {
            root: cwd.to_path_buf(),
            ..Default::default()
        })
}

/// Caller deadline for `/skills` Smart search. Never a never-cancelled token:
/// backfill and query embed must observe this bound.
fn smart_search_cancel(service: Option<&crate::retrieval::RetrievalService>) -> CancellationToken {
    let deadline_ms = service
        .map(|s| s.load_snapshot().prime.skills.deadline_ms)
        .filter(|&n| n > 0)
        .unwrap_or(3_000);
    crate::session::prime::bounded_cancel(deadline_ms)
}

fn local_skill_matches(query_lower: &str, skill: &SkillInfo) -> bool {
    skill.name.to_ascii_lowercase().contains(query_lower)
        || skill.description.to_ascii_lowercase().contains(query_lower)
        || skill
            .when_to_use
            .as_deref()
            .is_some_and(|w| w.to_ascii_lowercase().contains(query_lower))
}

/// Local-only `/skills` ranking used for Local mode and Smart fallback.
fn local_search_names(query: &str, skills: &[SkillInfo], exact: Vec<String>) -> Vec<String> {
    let mut names = exact;
    if query.is_empty() {
        return names;
    }
    let q = query.to_ascii_lowercase();
    for skill in skills {
        if !names.iter().any(|n| n == &skill.name) && local_skill_matches(&q, skill) {
            names.push(skill.name.clone());
        }
    }
    names
}

fn finish_search(
    query: &str,
    skills: &[SkillInfo],
    exact: Vec<String>,
    ranked: Option<Vec<String>>,
) -> super::skills::SkillsSearchResponse {
    match ranked {
        Some(mut names) => {
            for name in exact.into_iter().rev() {
                names.retain(|n| n != &name);
                names.insert(0, name);
            }
            super::skills::SkillsSearchResponse {
                api_version: SKILLS_API_VERSION,
                names,
                degraded: false,
            }
        }
        None => super::skills::SkillsSearchResponse {
            api_version: SKILLS_API_VERSION,
            names: local_search_names(query, skills, exact),
            degraded: true,
        },
    }
}

pub async fn search(
    req: super::skills::SkillsSearchRequest,
    plugin_registry: Option<&xai_grok_agent::plugins::PluginRegistry>,
    compat: CompatConfig,
) -> ExtResult {
    let listing = load_listing(&req.cwd, plugin_registry, compat).await;
    let query = req.query.trim();
    let mut exact: Vec<String> = Vec::new();
    if !query.is_empty() {
        for skill in &listing.skills {
            if skill.name.eq_ignore_ascii_case(query) {
                exact.push(skill.name.clone());
            }
        }
    }
    let want_smart = req
        .mode
        .as_deref()
        .is_some_and(|m| m.eq_ignore_ascii_case("smart"));
    if !want_smart || query.is_empty() {
        return super::to_ext_response(Ok(finish_search(query, &listing.skills, exact, None)));
    }

    let home = grok_home();
    let cwd = PathBuf::from(&req.cwd);
    let inventory = workspace_inventory_for_search(&cwd).await;
    let service = crate::retrieval::registry_for_home(&home).map(|r| r.service());
    let (profile, consumer_min_score) = smart_search_profile_and_consumer_floor(service.as_ref());
    let ranked = crate::session::prime::smart_search_names(
        service.as_ref(),
        profile.as_deref(),
        query,
        &listing.skills,
        Some(&home),
        &cwd,
        &inventory,
        consumer_min_score,
        smart_search_cancel(service.as_ref()),
    )
    .await;
    super::to_ext_response(Ok(finish_search(query, &listing.skills, exact, ranked)))
}

pub async fn validate(
    req: VersionedCwdRequest,
    plugin_registry: Option<&xai_grok_agent::plugins::PluginRegistry>,
    compat: CompatConfig,
) -> ExtResult {
    if let Some(err) = mixed_version_error("x.ai/skills/validate", req.api_version) {
        return err;
    }
    let cwd = req.cwd.as_deref().unwrap_or(".");
    let Some(path) = req.path.as_deref() else {
        return super::to_ext_response(Err::<SkillsValidateResponse, _>(anyhow::anyhow!(
            "path is required"
        )));
    };
    let resolved = PathBuf::from(path);
    let outcome = if resolved.is_dir() {
        validate_strict_skill_dir(&resolved, None)
    } else {
        validate_strict_skill_dir(resolved.parent().unwrap_or(&resolved), None)
    };
    let listing = load_listing(cwd, plugin_registry, compat).await;
    let (status, identity, diagnostics) = match outcome {
        StrictSkillOutcome::Valid(discovered) => {
            (SkillHealthStatus::Untested, discovered.identity, Vec::new())
        }
        StrictSkillOutcome::Quarantined(row) => (
            SkillHealthStatus::Quarantined,
            row.identity,
            row.diagnostics,
        ),
    };
    super::to_ext_response(Ok(SkillsValidateResponse {
        api_version: SKILLS_API_VERSION,
        generation: current_generation().max(listing.inventory.generation),
        status,
        identity,
        diagnostics,
    }))
}

pub async fn publish_or_update(req: VersionedCwdRequest, is_update: bool) -> ExtResult {
    let method = if is_update {
        "x.ai/skills/update"
    } else {
        "x.ai/skills/publish"
    };
    if let Some(err) = mixed_version_error(method, req.api_version) {
        return err;
    }
    let cwd = PathBuf::from(req.cwd.as_deref().unwrap_or("."));
    let generation = current_generation();
    let result = if let Some(path) = req.path.as_deref() {
        let source = PathBuf::from(path);
        let scope = PublishScope::parse(req.scope.as_deref().unwrap_or("project"))
            .map_err(|e| anyhow::anyhow!(e.message()));
        let scope = match scope {
            Ok(s) => s,
            Err(e) => return super::to_ext_response(Err::<SkillsPublishResponse, _>(e)),
        };
        let dest_parent = match dest_parent_for_scope(scope, &cwd, &grok_home()) {
            Ok(p) => p,
            Err(e) => {
                return super::to_ext_response(Err::<SkillsPublishResponse, _>(anyhow::anyhow!(
                    e.message()
                )));
            }
        };
        publish_skill_directory(
            &source,
            &dest_parent,
            scope,
            req.expected_generation,
            generation,
        )
        .map(|r| SkillsPublishResponse {
            api_version: SKILLS_API_VERSION,
            generation: r.generation,
            identity: r.identity,
            created: r.created,
            status: r.status,
        })
        .map_err(|e| anyhow::anyhow!(e.message()))
    } else {
        let name = req.name.as_deref().unwrap_or("");
        let description = req.description.as_deref().unwrap_or("");
        publish_from_fields(
            &cwd,
            name,
            description,
            req.body.as_deref().unwrap_or(""),
            req.scope.as_deref().unwrap_or("project"),
            req.expected_generation,
            generation,
        )
        .map(|r| SkillsPublishResponse {
            api_version: SKILLS_API_VERSION,
            generation: r.generation,
            identity: SkillIdentity::new(&r.name, None),
            created: r.created,
            status: SkillHealthStatus::Untested,
        })
        .map_err(|e| anyhow::anyhow!(e.message()))
    };
    match result {
        Ok(resp) => {
            INVENTORY_GENERATION.store(resp.generation, Ordering::Relaxed);
            super::to_ext_response(Ok(resp))
        }
        Err(e) => super::to_ext_response(Err::<SkillsPublishResponse, _>(e)),
    }
}

pub async fn regress_status(
    req: VersionedCwdRequest,
    plugin_registry: Option<&xai_grok_agent::plugins::PluginRegistry>,
    compat: CompatConfig,
) -> ExtResult {
    if let Some(err) = mixed_version_error("x.ai/skills/regress/status", req.api_version) {
        return err;
    }
    let cwd = req.cwd.as_deref().unwrap_or(".");
    let listing = load_listing(cwd, plugin_registry, compat).await;
    let identity = resolve_identity(&listing, &req);
    let running = identity
        .as_ref()
        .map(|id| {
            let guard = jobs()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard
                .iter()
                .any(|(key, job)| job_matches(key, id) && job.running.load(Ordering::Relaxed))
        })
        .unwrap_or(false);
    let summary = identity.as_ref().and_then(|id| {
        load_eval_report(&grok_home(), id)
            .ok()
            .flatten()
            .map(|report| {
                regression_summary_from_report(
                    &report,
                    &listing.inventory.fingerprint(),
                    skill_dir_for(&listing, id).as_deref(),
                )
            })
    });
    super::to_ext_response(Ok(SkillsRegressStatusResponse {
        api_version: SKILLS_API_VERSION,
        generation: current_generation(),
        running,
        summary,
    }))
}

pub async fn regress_run(
    req: VersionedCwdRequest,
    plugin_registry: Option<&xai_grok_agent::plugins::PluginRegistry>,
    compat: CompatConfig,
) -> ExtResult {
    if let Some(err) = mixed_version_error("x.ai/skills/regress/run", req.api_version) {
        return err;
    }
    let Some(identity) = identity_from_req(&req) else {
        return super::to_ext_response(Err::<EvalRunReport, _>(anyhow::anyhow!(
            "skill not found or quarantined"
        )));
    };
    let generation = current_generation();
    if let Some(expected) = req.expected_generation
        && expected != generation
    {
        return super::to_ext_response(Err::<EvalRunReport, _>(anyhow::anyhow!(
            "generation-mismatch"
        )));
    }
    let (mut registered, cancel) = match register_run(&identity, generation) {
        Ok(job) => job,
        Err(RegisterError::AlreadyRunning) => {
            return super::to_ext_response(Err::<EvalRunReport, _>(anyhow::anyhow!(
                "regression already running"
            )));
        }
    };
    let cwd = req.cwd.as_deref().unwrap_or(".");
    let listing = load_listing(cwd, plugin_registry, compat).await;
    let Some(skill) = select_skill(&listing, &req) else {
        return super::to_ext_response(Err::<EvalRunReport, _>(anyhow::anyhow!(
            "skill not found or quarantined"
        )));
    };
    let listed_identity = SkillIdentity::new(&skill.name, Some(skill.scope));
    registered.rekey(regression_store_key(&listed_identity));
    let skill_dir = Path::new(&skill.path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(&skill.path));
    let suite = match load_eval_suite_from_dir(&skill_dir) {
        Ok(Some(suite)) => suite,
        Ok(None) => {
            return super::to_ext_response(Err::<EvalRunReport, _>(anyhow::anyhow!(
                "evals/cases.yaml is missing"
            )));
        }
        Err(err) => {
            return super::to_ext_response(Err::<EvalRunReport, _>(anyhow::anyhow!(err.message)));
        }
    };
    let peers: Vec<LocalSkillEvidence> = listing
        .skills
        .iter()
        .filter(|s| s.name != skill.name)
        .map(LocalSkillEvidence::from_skill_info)
        .collect();
    let subject = LocalSkillEvidence::from_skill_info(skill);
    let inventory_fp = listing.inventory.fingerprint();
    let confirm_profile = req.confirm_configured_profile;
    let quality_queries: Vec<String> = if confirm_profile {
        suite
            .cases
            .iter()
            .map(|c| c.query.clone())
            .filter(|q| !q.is_empty())
            .collect()
    } else {
        Vec::new()
    };
    let quality_skills = listing.skills.clone();
    let quality_cwd = PathBuf::from(cwd);
    // Run off the ACP LocalSet so `regress/cancel` and `regress/status`
    // can be processed while the suite is in flight. A sync suite inside
    // `ext_method` would block the current-thread runtime until it finished.
    let cancel_for_run = Arc::clone(&cancel);
    let report = match tokio::task::spawn_blocking(move || {
        run_eval_suite(
            &suite,
            &subject,
            &peers,
            listed_identity,
            generation,
            &inventory_fp,
            cancel_for_run.as_ref(),
        )
    })
    .await
    {
        Ok(report) => report,
        Err(_) => {
            return super::to_ext_response(Err::<EvalRunReport, _>(anyhow::anyhow!(
                "regression runner failed"
            )));
        }
    };
    persist_job_report(&grok_home(), &report, generation, cancel.as_ref());
    if quality_pass_is_current(confirm_profile, &report, generation, cancel.as_ref()) {
        // Configured-profile quality pass: metadata only, never bodies.
        // Cancelled and generation-mismatched suites must not contact the
        // configured route. `smart_search_cancel` is a deadline token, so
        // the regression cancel flag is checked between queries.
        let home = grok_home();
        let service = crate::retrieval::registry_for_home(&home).map(|r| r.service());
        let (profile, consumer_min_score) =
            smart_search_profile_and_consumer_floor(service.as_ref());
        let inventory = workspace_inventory_for_search(&quality_cwd).await;
        for query in quality_queries_until_cancel(&quality_queries, cancel.as_ref()) {
            if generation != current_generation() {
                break;
            }
            let _ = crate::session::prime::smart_search_names(
                service.as_ref(),
                profile.as_deref(),
                query,
                &quality_skills,
                Some(&home),
                &quality_cwd,
                &inventory,
                consumer_min_score,
                smart_search_cancel(service.as_ref()),
            )
            .await;
        }
    }
    super::to_ext_response(Ok(report))
}

pub fn regress_cancel(req: VersionedCwdRequest) -> ExtResult {
    if let Some(err) = mixed_version_error("x.ai/skills/regress/cancel", req.api_version) {
        return err;
    }
    if let Some(identity) = identity_from_req(&req) {
        request_cancel(&identity);
    }
    super::to_ext_response(Ok(serde_json::json!({
        "apiVersion": SKILLS_API_VERSION,
        "cancelled": true,
    })))
}

pub async fn regress_update(
    req: VersionedCwdRequest,
    plugin_registry: Option<&xai_grok_agent::plugins::PluginRegistry>,
    compat: CompatConfig,
) -> ExtResult {
    if let Some(err) = mixed_version_error("x.ai/skills/regress/update", req.api_version) {
        return err;
    }
    // Refresh stale flags only. Never starts a run.
    regress_status(req, plugin_registry, compat).await
}

async fn load_listing(
    cwd: &str,
    plugin_registry: Option<&xai_grok_agent::plugins::PluginRegistry>,
    compat: CompatConfig,
) -> SkillListing {
    let config = crate::util::config::load_config().await.skills;
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        list_skill_sources_with_plugins(Some(cwd), &config, plugin_registry, compat),
    )
    .await
    {
        Ok(listing) => listing,
        Err(_) => SkillListing {
            skills: Vec::new(),
            commands: Vec::new(),
            inventory: Default::default(),
        },
    }
}

fn persist_complete_report(home: &Path, report: &EvalRunReport) {
    if !report.cancelled {
        let _ = persist_eval_report(home, report);
    }
}

/// Persist only a finished, current-generation report. Cancelled and
/// generation-mismatched runs must not overwrite stored results.
fn persist_job_report(
    home: &Path,
    report: &EvalRunReport,
    job_generation: u64,
    cancel: &AtomicBool,
) {
    if !run_is_current(report, job_generation, cancel) {
        return;
    }
    persist_complete_report(home, report);
}

fn run_is_current(report: &EvalRunReport, job_generation: u64, cancel: &AtomicBool) -> bool {
    !report.cancelled && !cancel.load(Ordering::Relaxed) && job_generation == current_generation()
}

/// Confirmed-profile quality contact is allowed only for a live, current
/// generation run that has not been cancelled.
fn quality_pass_is_current(
    confirm_profile: bool,
    report: &EvalRunReport,
    job_generation: u64,
    cancel: &AtomicBool,
) -> bool {
    confirm_profile && run_is_current(report, job_generation, cancel)
}

/// Remaining quality queries, stopping when cancel is set. Used so the
/// confirmed-profile loop cannot keep contacting the route after cancel.
fn quality_queries_until_cancel<'a>(
    queries: &'a [String],
    cancel: &'a AtomicBool,
) -> impl Iterator<Item = &'a String> {
    queries
        .iter()
        .take_while(move |_| !cancel.load(Ordering::Relaxed))
}

fn regression_summary_from_report(
    report: &EvalRunReport,
    inventory_fp: &str,
    skill_dir: Option<&Path>,
) -> SkillRegressionSummary {
    let mut summary = SkillRegressionSummary::from_report(report);
    let live = skill_dir.and_then(live_cases_fingerprint);
    if report.is_stale_vs_live(inventory_fp, live.as_deref()) {
        summary.status = SkillHealthStatus::Stale;
    }
    summary
}

fn skill_dir_for(listing: &SkillListing, identity: &SkillIdentity) -> Option<PathBuf> {
    listing
        .skills
        .iter()
        .find(|skill| {
            skill.name == identity.parent_dir_name
                && identity.scope.is_none_or(|scope| scope == skill.scope)
        })
        .and_then(|skill| Path::new(&skill.path).parent().map(Path::to_path_buf))
}

fn parse_skill_scope(raw: Option<&str>) -> Option<SkillScope> {
    match raw? {
        "user" => Some(SkillScope::User),
        "repo" | "project" => Some(SkillScope::Repo),
        "local" => Some(SkillScope::Local),
        "server" => Some(SkillScope::Server),
        "bundled" => Some(SkillScope::Bundled),
        "plugin" => Some(SkillScope::Plugin),
        _ => None,
    }
}

/// Directory identity for a skill path. A `SKILL.md` leaf uses the parent
/// directory name so register/cancel/rekey share `{scope}-{dir}` instead of
/// colliding on `unscoped-md`.
fn identity_name_from_path(path: &str) -> Option<&str> {
    let path = Path::new(path);
    let file_name = path.file_name().and_then(|name| name.to_str())?;
    if file_name.eq_ignore_ascii_case("SKILL.md") {
        path.parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty() && *name != "." && *name != "..")
    } else {
        Some(file_name)
    }
}

fn identity_from_req(req: &VersionedCwdRequest) -> Option<SkillIdentity> {
    req.name
        .as_deref()
        .or_else(|| req.path.as_deref().and_then(identity_name_from_path))
        .map(|name| SkillIdentity::new(name, parse_skill_scope(req.scope.as_deref())))
}

fn resolve_identity(listing: &SkillListing, req: &VersionedCwdRequest) -> Option<SkillIdentity> {
    if let Some(skill) = select_skill(listing, req) {
        return Some(SkillIdentity::new(&skill.name, Some(skill.scope)));
    }
    identity_from_req(req)
}

/// Component-wise path match: exact SKILL.md path, its parent directory, or a
/// unique `Path::starts_with` prefix. String prefixes such as `commit` vs
/// `commit-msg` must not match.
fn skill_path_matches(skill_path: &str, requested: &str) -> bool {
    let skill = Path::new(skill_path);
    let requested = Path::new(requested);
    skill == requested || skill.parent() == Some(requested) || skill.starts_with(requested)
}

fn select_skill<'a>(
    listing: &'a SkillListing,
    req: &VersionedCwdRequest,
) -> Option<&'a xai_grok_tools::implementations::skills::types::SkillInfo> {
    if let Some(name) = req.name.as_deref() {
        return listing.skills.iter().find(|s| s.name == name);
    }
    if let Some(path) = req.path.as_deref() {
        let mut matched = listing
            .skills
            .iter()
            .filter(|s| skill_path_matches(&s.path, path));
        let first = matched.next()?;
        if matched.next().is_some() {
            return None;
        }
        return Some(first);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_version_new_methods_fail_closed() {
        assert!(mixed_version_error("x.ai/skills/validate", None).is_some());
        assert!(mixed_version_error("x.ai/skills/publish", Some(0)).is_some());
        assert!(mixed_version_error("x.ai/skills/regress/run", Some(2)).is_some());
        assert!(mixed_version_error("x.ai/skills/search", None).is_some());
        assert!(mixed_version_error("x.ai/skills/list", None).is_none());
        assert!(mixed_version_error("x.ai/skills/validate", Some(1)).is_none());
    }

    #[test]
    fn configured_profile_regression_defaults_unconfirmed() {
        let req: VersionedCwdRequest = serde_json::from_str(r#"{"apiVersion":1}"#).unwrap();
        assert!(
            !req.confirm_configured_profile,
            "configured-profile regression must stay local until explicitly confirmed"
        );
    }

    #[test]
    fn workspace_inventory_from_cwd_records_name_evidence() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("commit.rs"), "fn main() {}\n").unwrap();
        let inv = workspace_inventory_from_cwd(tmp.path());
        assert!(
            !inv.lowered_rels.is_empty(),
            "Smart search must walk cwd instead of WorkspaceInventory::default()"
        );
        assert!(
            inv.lowered_rels
                .iter()
                .any(|(rel, seg)| rel.contains("commit") || seg.contains("commit")),
            "inventory must keep local name evidence, rels={:?}",
            inv.lowered_rels
        );
        assert!(
            !inv.root.as_os_str().is_empty(),
            "path-glob scoring needs a real inventory root"
        );
    }

    #[tokio::test]
    async fn smart_search_cancel_is_bounded() {
        let token = smart_search_cancel(None);
        assert!(
            !token.is_cancelled(),
            "default Smart search deadline must not fire immediately"
        );
        let zero = crate::session::prime::bounded_cancel(0);
        assert!(
            zero.is_cancelled(),
            "deadline 0 must cancel so Smart search is never unbounded"
        );
    }

    #[test]
    fn smart_search_uses_prime_skills_consumer_min_score() {
        let mut skills = xai_grok_config_types::SkillPrimeConfig::default();
        skills.retrieval_profile = Some("shipped".into());
        skills.min_score = 0.5;
        let (profile, floor) = smart_search_profile_and_consumer_floor_from_skills(&skills);
        assert_eq!(profile.as_deref(), Some("shipped"));
        assert!(
            (floor - 0.5).abs() < f32::EPSILON,
            "Smart search must use [prime.skills].min_score, got {floor}"
        );
        assert!(
            floor > 0.0,
            "raised consumer min_score must not collapse to the hardcoded 0.0 floor"
        );

        let default_skills = xai_grok_config_types::SkillPrimeConfig::default();
        let (_, default_floor) =
            smart_search_profile_and_consumer_floor_from_skills(&default_skills);
        assert_eq!(default_floor, 0.0);

        let (missing_profile, missing_floor) = smart_search_profile_and_consumer_floor(None);
        assert!(missing_profile.is_none());
        assert_eq!(missing_floor, 0.0);
    }

    #[test]
    fn smart_search_embed_miss_falls_back_to_local_alpha_degraded() {
        let skills = vec![skill_at("alpha", "skills/alpha/SKILL.md")];
        let none = finish_search("alp", &skills, Vec::new(), None);
        assert_eq!(none.names, vec!["alpha".to_string()]);
        assert!(
            none.degraded,
            "embed/index miss must run local fallback with degraded:true"
        );

        let empty_ranked = finish_search("alp", &skills, Vec::new(), Some(Vec::new()));
        assert!(
            empty_ranked.names.is_empty(),
            "Some([]) is a successful empty Smart hit list, not local fallback: {:?}",
            empty_ranked.names
        );
        assert!(!empty_ranked.degraded);
    }

    #[test]
    fn generation_bumps_monotonically() {
        let a = bump_generation();
        let b = bump_generation();
        assert!(b > a);
    }

    #[test]
    fn cancelled_report_does_not_overwrite_persisted_results() {
        let tmp = tempfile::tempdir().unwrap();
        let identity = SkillIdentity::new("commit", None);
        let keep = EvalRunReport {
            schema_version: 1,
            generation: 3,
            inventory_fingerprint: "inv".into(),
            cases_fingerprint: "cases".into(),
            identity: identity.clone(),
            status: SkillHealthStatus::ValidPass,
            results: Default::default(),
            cancelled: false,
            stable: true,
        };
        persist_complete_report(tmp.path(), &keep);
        let cancelled = EvalRunReport {
            cancelled: true,
            status: SkillHealthStatus::Untested,
            ..keep.clone()
        };
        persist_complete_report(tmp.path(), &cancelled);
        let loaded = load_eval_report(tmp.path(), &identity).unwrap().unwrap();
        assert_eq!(loaded.status, SkillHealthStatus::ValidPass);
        assert!(!loaded.cancelled);
        assert!(
            tmp.path()
                .join("skill-regress/unscoped-commit.json")
                .is_file()
        );
        assert!(!tmp.path().join("skills/regress").exists());
    }

    #[test]
    fn persisted_report_is_not_stale_when_only_process_generation_differs() {
        let report = EvalRunReport {
            schema_version: 1,
            generation: 4,
            inventory_fingerprint: "inv".into(),
            cases_fingerprint: "cases".into(),
            identity: SkillIdentity::new("commit", None),
            status: SkillHealthStatus::ValidPass,
            results: Default::default(),
            cancelled: false,
            stable: true,
        };
        assert!(!report.is_stale("inv", "cases"));
        assert!(!report.is_stale_vs_live("inv", Some("cases")));
        let summary = regression_summary_from_report(&report, "inv", None);
        assert_eq!(summary.status, SkillHealthStatus::Stale);
        let summary = regression_summary_from_report(&report, "other", None);
        assert_eq!(summary.status, SkillHealthStatus::Stale);
    }

    #[test]
    fn cancel_matches_scoped_jobs_when_request_omits_scope() {
        let user = SkillIdentity::new(
            "commit",
            Some(xai_grok_tools::implementations::skills::types::SkillScope::User),
        );
        let repo = SkillIdentity::new(
            "commit",
            Some(xai_grok_tools::implementations::skills::types::SkillScope::Repo),
        );
        let user_key = regression_store_key(&user);
        let repo_key = regression_store_key(&repo);
        assert_ne!(user_key, repo_key);
        {
            let mut guard = jobs().lock().unwrap();
            guard.insert(
                user_key.clone(),
                RegressionJob {
                    generation: 1,
                    run_token: next_run_token(),
                    cancel: Arc::new(AtomicBool::new(false)),
                    running: AtomicBool::new(true),
                },
            );
            guard.insert(
                repo_key.clone(),
                RegressionJob {
                    generation: 1,
                    run_token: next_run_token(),
                    cancel: Arc::new(AtomicBool::new(false)),
                    running: AtomicBool::new(true),
                },
            );
        }
        let named = identity_from_req(&VersionedCwdRequest {
            api_version: Some(1),
            name: Some("commit".into()),
            ..VersionedCwdRequest::default()
        })
        .unwrap();
        if let Ok(guard) = jobs().lock() {
            for (key, job) in guard.iter() {
                if regression_key_matches(key, &named) {
                    job.cancel.store(true, Ordering::Relaxed);
                }
            }
        }
        {
            let guard = jobs().lock().unwrap();
            assert!(guard.get(&user_key).unwrap().cancel.load(Ordering::Relaxed));
            assert!(guard.get(&repo_key).unwrap().cancel.load(Ordering::Relaxed));
        }
        let user_only = identity_from_req(&VersionedCwdRequest {
            api_version: Some(1),
            name: Some("commit".into()),
            scope: Some("user".into()),
            ..VersionedCwdRequest::default()
        })
        .unwrap();
        {
            let mut guard = jobs().lock().unwrap();
            if let Some(job) = guard.get(&user_key) {
                job.cancel.store(false, Ordering::Relaxed);
            }
            if let Some(job) = guard.get(&repo_key) {
                job.cancel.store(false, Ordering::Relaxed);
            }
            for (key, job) in guard.iter() {
                if regression_key_matches(key, &user_only) {
                    job.cancel.store(true, Ordering::Relaxed);
                }
            }
            assert!(guard.get(&user_key).unwrap().cancel.load(Ordering::Relaxed));
            assert!(!guard.get(&repo_key).unwrap().cancel.load(Ordering::Relaxed));
            guard.remove(&user_key);
            guard.remove(&repo_key);
        }
    }

    fn sample_report(generation: u64, cancelled: bool) -> EvalRunReport {
        EvalRunReport {
            schema_version: 1,
            generation,
            inventory_fingerprint: "inv".into(),
            cases_fingerprint: "cases".into(),
            identity: SkillIdentity::new("commit", None),
            status: if cancelled {
                SkillHealthStatus::Untested
            } else {
                SkillHealthStatus::ValidPass
            },
            results: Default::default(),
            cancelled,
            stable: true,
        }
    }

    #[test]
    fn persist_skips_cancelled_or_mismatched_generation() {
        let tmp = tempfile::tempdir().unwrap();
        let identity = SkillIdentity::new("commit", None);
        let generation = current_generation();
        let keep = sample_report(generation, false);
        persist_job_report(tmp.path(), &keep, generation, &AtomicBool::new(false));
        assert!(
            load_eval_report(tmp.path(), &identity)
                .unwrap()
                .is_some_and(|loaded| loaded.status == SkillHealthStatus::ValidPass)
        );

        persist_job_report(
            tmp.path(),
            &sample_report(generation, true),
            generation,
            &AtomicBool::new(false),
        );
        persist_job_report(
            tmp.path(),
            &sample_report(generation, false),
            generation,
            &AtomicBool::new(true),
        );
        persist_job_report(
            tmp.path(),
            &sample_report(generation.saturating_add(1), false),
            generation.saturating_add(1),
            &AtomicBool::new(false),
        );
        let loaded = load_eval_report(tmp.path(), &identity).unwrap().unwrap();
        assert_eq!(loaded.status, SkillHealthStatus::ValidPass);
        assert!(!loaded.cancelled);
        assert_eq!(loaded.generation, generation);
    }

    #[test]
    fn quality_pass_skips_cancelled_or_generation_mismatch() {
        let generation = current_generation();
        let live = sample_report(generation, false);
        assert!(quality_pass_is_current(
            true,
            &live,
            generation,
            &AtomicBool::new(false)
        ));
        assert!(
            !quality_pass_is_current(false, &live, generation, &AtomicBool::new(false)),
            "unconfirmed profile must not contact the configured route"
        );
        assert!(!quality_pass_is_current(
            true,
            &sample_report(generation, true),
            generation,
            &AtomicBool::new(false)
        ));
        assert!(!quality_pass_is_current(
            true,
            &live,
            generation,
            &AtomicBool::new(true)
        ));
        assert!(!quality_pass_is_current(
            true,
            &live,
            generation.saturating_add(1),
            &AtomicBool::new(false)
        ));
    }

    #[test]
    fn quality_query_loop_aborts_when_cancel_is_set() {
        let queries = vec!["alpha".into(), "beta".into(), "gamma".into()];
        let cancel = AtomicBool::new(false);
        let mut seen = Vec::new();
        for query in quality_queries_until_cancel(&queries, &cancel) {
            seen.push(query.clone());
            if query == "alpha" {
                cancel.store(true, Ordering::Relaxed);
            }
        }
        assert_eq!(seen, vec!["alpha".to_string()]);
        cancel.store(true, Ordering::Relaxed);
        let none: Vec<&String> = quality_queries_until_cancel(&queries, &cancel).collect();
        assert!(none.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cancel_reaches_job_while_eval_runs_off_ext_method() {
        let identity = SkillIdentity::new("slow-commit", None);
        let job_key = regression_store_key(&identity);
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut guard = jobs().lock().unwrap();
            guard.insert(
                job_key.clone(),
                RegressionJob {
                    generation: current_generation(),
                    run_token: next_run_token(),
                    cancel: Arc::clone(&cancel),
                    running: AtomicBool::new(true),
                },
            );
        }
        let started = Arc::new(AtomicBool::new(false));
        let started_flag = Arc::clone(&started);
        let cancel_for_run = Arc::clone(&cancel);
        let run = tokio::task::spawn_blocking(move || {
            started_flag.store(true, Ordering::SeqCst);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            while !cancel_for_run.load(Ordering::Relaxed) {
                if std::time::Instant::now() > deadline {
                    return false;
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            true
        });
        let wait_deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !started.load(Ordering::SeqCst) {
            if std::time::Instant::now() > wait_deadline {
                let mut guard = jobs().lock().unwrap();
                guard.remove(&job_key);
                panic!("blocking eval task did not start");
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        let _ = regress_cancel(VersionedCwdRequest {
            api_version: Some(1),
            name: Some("slow-commit".into()),
            ..VersionedCwdRequest::default()
        });
        assert!(
            run.await.expect("blocking eval join"),
            "cancel must be visible to the spawned eval before it finishes"
        );
        assert!(cancel.load(Ordering::Relaxed));
        if let Ok(mut guard) = jobs().lock() {
            guard.remove(&job_key);
        }
    }

    #[test]
    fn idle_cancel_then_register_run_persists_valid_pass() {
        let tmp = tempfile::tempdir().unwrap();
        let identity = SkillIdentity::new("idle-cancel-then-run", None);
        let key = regression_store_key(&identity);
        {
            let mut guard = jobs().lock().unwrap();
            guard.remove(&key);
        }
        let _ = regress_cancel(VersionedCwdRequest {
            api_version: Some(1),
            name: Some("idle-cancel-then-run".into()),
            ..VersionedCwdRequest::default()
        });
        {
            let guard = jobs().lock().unwrap();
            assert!(
                guard.get(&key).is_none(),
                "idle cancel must not insert a cancel-intent placeholder"
            );
        }
        let generation = current_generation();
        let (registered, cancel) =
            register_run(&identity, generation).expect("register after idle cancel");
        assert!(
            !cancel.load(Ordering::Relaxed),
            "the next run must not inherit an idle cancel"
        );
        let mut report = sample_report(generation, false);
        report.identity = identity.clone();
        persist_job_report(tmp.path(), &report, generation, cancel.as_ref());
        let loaded = load_eval_report(tmp.path(), &identity)
            .unwrap()
            .expect("idle cancel then run must persist");
        assert_eq!(loaded.status, SkillHealthStatus::ValidPass);
        assert!(!loaded.cancelled);
        drop(registered);
        {
            let mut guard = jobs().lock().unwrap();
            guard.remove(&key);
        }
    }

    fn listing_with(
        skills: Vec<xai_grok_tools::implementations::skills::types::SkillInfo>,
    ) -> SkillListing {
        SkillListing {
            skills,
            commands: Vec::new(),
            inventory: Default::default(),
        }
    }

    fn skill_at(
        name: &str,
        path: &str,
    ) -> xai_grok_tools::implementations::skills::types::SkillInfo {
        xai_grok_tools::implementations::skills::types::SkillInfo {
            name: name.to_string(),
            path: path.to_string(),
            ..xai_grok_tools::implementations::skills::types::SkillInfo::default()
        }
    }

    #[test]
    fn path_select_does_not_match_string_prefix_of_sibling_skill() {
        // commit-msg is first so a string-prefix find() would pick it for
        // `/repo/.grok/skills/commit`.
        let listing = listing_with(vec![
            skill_at("commit-msg", "/repo/.grok/skills/commit-msg/SKILL.md"),
            skill_at("commit", "/repo/.grok/skills/commit/SKILL.md"),
        ]);
        let commit_dir = VersionedCwdRequest {
            path: Some("/repo/.grok/skills/commit".into()),
            ..VersionedCwdRequest::default()
        };
        let selected = select_skill(&listing, &commit_dir).expect("commit dir is unique");
        assert_eq!(selected.name, "commit");
        assert_eq!(selected.path, "/repo/.grok/skills/commit/SKILL.md");

        let commit_file = VersionedCwdRequest {
            path: Some("/repo/.grok/skills/commit/SKILL.md".into()),
            ..VersionedCwdRequest::default()
        };
        let selected = select_skill(&listing, &commit_file).expect("exact SKILL.md is unique");
        assert_eq!(selected.name, "commit");

        let sibling = VersionedCwdRequest {
            path: Some("/repo/.grok/skills/commit-msg".into()),
            ..VersionedCwdRequest::default()
        };
        let selected = select_skill(&listing, &sibling).expect("commit-msg dir is unique");
        assert_eq!(selected.name, "commit-msg");
    }

    #[test]
    fn path_select_rejects_ambiguous_or_missing_matches() {
        let listing = listing_with(vec![
            skill_at("commit-msg", "/repo/.grok/skills/commit-msg/SKILL.md"),
            skill_at("commit", "/repo/.grok/skills/commit/SKILL.md"),
        ]);
        let parent = VersionedCwdRequest {
            path: Some("/repo/.grok/skills".into()),
            ..VersionedCwdRequest::default()
        };
        assert!(
            select_skill(&listing, &parent).is_none(),
            "a parent that matches more than one skill must be rejected"
        );
        let missing = VersionedCwdRequest {
            path: Some("/repo/.grok/skills/other".into()),
            ..VersionedCwdRequest::default()
        };
        assert!(select_skill(&listing, &missing).is_none());
    }

    #[test]
    fn second_regress_run_is_rejected_while_token_is_running() {
        let identity = SkillIdentity::new("live-token-run", None);
        let key = regression_store_key(&identity);
        {
            let mut guard = jobs().lock().unwrap();
            guard.remove(&key);
        }
        let generation = current_generation();
        let (first, _) = register_run(&identity, generation).expect("first run");
        assert!(matches!(
            register_run(&identity, generation),
            Err(RegisterError::AlreadyRunning)
        ));
        let first_token = first.token;
        remove_job_if_token(&key, first_token.wrapping_add(99));
        {
            let guard = jobs().lock().unwrap();
            let job = guard
                .get(&key)
                .expect("live job must survive a stale remove");
            assert_eq!(job.run_token, first_token);
            assert!(job.running.load(Ordering::Relaxed));
        }
        drop(first);
        let (second, _) =
            register_run(&identity, generation).expect("run after the live token is gone");
        assert_ne!(second.token, first_token);
        drop(second);
        {
            let mut guard = jobs().lock().unwrap();
            guard.remove(&key);
        }
    }

    fn path_req(path: &str) -> VersionedCwdRequest {
        VersionedCwdRequest {
            api_version: Some(1),
            path: Some(path.into()),
            ..VersionedCwdRequest::default()
        }
    }

    #[test]
    fn skill_md_path_identity_uses_parent_directory_name() {
        let file = identity_from_req(&path_req("/repo/.grok/skills/commit/SKILL.md"))
            .expect("SKILL.md path has a parent directory");
        assert_eq!(file.parent_dir_name, "commit");
        assert_eq!(regression_store_key(&file), "unscoped-commit");

        let mixed = identity_from_req(&path_req("/repo/.grok/skills/commit/skill.md"))
            .expect("case-insensitive SKILL.md leaf");
        assert_eq!(mixed.parent_dir_name, "commit");

        let dir = identity_from_req(&path_req("/repo/.grok/skills/commit")).unwrap();
        assert_eq!(dir.parent_dir_name, "commit");

        let sibling =
            identity_from_req(&path_req("/repo/.grok/skills/commit-msg/SKILL.md")).unwrap();
        assert_eq!(sibling.parent_dir_name, "commit-msg");
        assert_ne!(
            regression_store_key(&file),
            regression_store_key(&sibling),
            "commit vs commit-msg must not share a job key"
        );
        assert_ne!(regression_store_key(&file), "unscoped-md");
        assert_ne!(regression_store_key(&sibling), "unscoped-md");
    }

    #[test]
    fn skill_md_path_cancel_matches_after_rekey_to_scoped_job() {
        let req = path_req("/repo/.grok/skills/commit/SKILL.md");
        let identity = identity_from_req(&req).unwrap();
        let key = regression_store_key(&identity);
        {
            let mut guard = jobs().lock().unwrap();
            guard.remove(&key);
            guard.remove("repo-commit");
        }
        let generation = current_generation();
        let (mut registered, cancel) = register_run(&identity, generation).expect("register");
        let listed = SkillIdentity::new(
            "commit",
            Some(xai_grok_tools::implementations::skills::types::SkillScope::Repo),
        );
        registered.rekey(regression_store_key(&listed));
        request_cancel(&identity_from_req(&req).unwrap());
        assert!(
            cancel.load(Ordering::Relaxed),
            "cancel with the same SKILL.md path must match the rekeyed scoped job"
        );
        drop(registered);
        {
            let mut guard = jobs().lock().unwrap();
            guard.remove(&key);
            guard.remove("repo-commit");
        }
    }

    #[test]
    fn concurrent_skill_md_paths_do_not_share_unscoped_md_job() {
        let commit_req = path_req("/repo/.grok/skills/commit/SKILL.md");
        let msg_req = path_req("/repo/.grok/skills/commit-msg/SKILL.md");
        let commit = identity_from_req(&commit_req).unwrap();
        let msg = identity_from_req(&msg_req).unwrap();
        let commit_key = regression_store_key(&commit);
        let msg_key = regression_store_key(&msg);
        assert_eq!(commit_key, "unscoped-commit");
        assert_eq!(msg_key, "unscoped-commit-msg");
        assert_ne!(commit_key, "unscoped-md");
        assert_ne!(msg_key, "unscoped-md");
        {
            let mut guard = jobs().lock().unwrap();
            guard.remove(&commit_key);
            guard.remove(&msg_key);
        }
        let generation = current_generation();
        let (commit_job, commit_cancel) =
            register_run(&commit, generation).expect("commit SKILL.md job");
        let (msg_job, msg_cancel) =
            register_run(&msg, generation).expect("commit-msg SKILL.md must not collide");
        assert_ne!(commit_job.key, msg_job.key);
        request_cancel(&commit);
        assert!(commit_cancel.load(Ordering::Relaxed));
        assert!(
            !msg_cancel.load(Ordering::Relaxed),
            "cancelling .../commit/SKILL.md must not cancel commit-msg"
        );
        drop(commit_job);
        drop(msg_job);
        {
            let mut guard = jobs().lock().unwrap();
            guard.remove(&commit_key);
            guard.remove(&msg_key);
        }
    }
}
