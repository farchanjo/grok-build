// media-understanding-dag
//
// Reusable execution workflow for the approved "Native Media Understanding for
// Text-Only Models" plan (plan.md sections 19-21). It orchestrates all 10 PRs
// of the mergeable PR DAG as dependency-level waves in ONE shared workspace.
//
// Design decisions:
//  * The main conversation is the brain/orchestrator only. Children explore,
//    implement, test, fix, and review. Every child (including every parallel
//    item) is pinned to the constant DEEPSEEK_MODEL via ctx.model.
//  * Reasoning effort: the workflow host API has NO option to set a child's
//    reasoning_effort (AgentOpts has no such field; unknown opts keys are
//    dropped). Children inherit role/persona/definition/catalog defaults. The
//    bundled "explore" role pins effort="medium", which the DeepSeek slug
//    rejects, so every child uses agent_type "general-purpose" (no
//    effort-pinning role) with an explicit capability_mode. For MAXIMUM
//    effort, configure the slug's catalog default reasoning level to "max"
//    before the run; args.confirm_max_effort=true records that confirmation
//    and skips the start-of-run verification gate.
//  * Integration model: the workflow host API has no primitive to merge an
//    isolated worktree back into the parent (isolation_worktree edits never
//    land). To produce actually-mergeable work, implementers/fixers work
//    DIRECTLY in the shared workspace with strict disjoint per-PR ownership
//    and dependency-level ordering. They leave changes uncommitted in the
//    working tree and never touch git history.
//  * Gates: local reversible edit/test/integration steps run autonomously.
//    await_user pauses are reserved for budget shortage, final-validation
//    failure, release hard blockers (legal review, macOS library validation),
//    and the final sign-off (pushes/PRs/deployment stay outside this
//    workflow). Fail-closed conditions complete with status "blocked".
//  * Smoke check: validate_only=true returns the canned host stub object
//    (stub: true) for every agent; guards accept that sentinel so the canned
//    path traverses every wave (explore -> implement -> test -> review ->
//    fix) and ends in the blocked completion. Real children cannot emit
//    "stub" because every output schema sets "additionalProperties": false.
//    For validate_only, pass args.confirm_max_effort=true so the smoke check
//    passes the start-of-run gate and traverses the full path.
//
// Budget: ~7 logical agent calls per PR per wave set. Nominal run ~40
// children; with retries and one fix round ~100-120. Launch with
// agent_budget 160+, use 256 for margin.

let meta = #{
    name: "media-understanding-dag",
    description: "Implement the 10-PR native media-understanding subsystem for text-only models using DeepSeek children in strict disjoint-ownership dependency waves",
    when_to_use: "Executing the approved media-understanding plan (PR 1-10) end to end in a shared workspace, with bounded retries, scratch handoffs, security/FFmpeg gates, and final validation.",
    phases: [
        #{ title: "Level 0 explore", detail: "PR 1 foundation seam" },
        #{ title: "Level 0 implement" },
        #{ title: "Level 0 test" },
        #{ title: "Level 0 review" },
        #{ title: "Level 0 fix" },
        #{ title: "Level 1 explore", detail: "PR 2/3/4/5" },
        #{ title: "Level 1 implement" },
        #{ title: "Level 1 test" },
        #{ title: "Level 1 review" },
        #{ title: "Level 1 fix" },
        #{ title: "Level 2 explore", detail: "PR 6 shell backend" },
        #{ title: "Level 2 implement" },
        #{ title: "Level 2 test" },
        #{ title: "Level 2 review" },
        #{ title: "Level 2 fix" },
        #{ title: "Level 3 explore", detail: "PR 7/8 tool + compaction" },
        #{ title: "Level 3 implement" },
        #{ title: "Level 3 test" },
        #{ title: "Level 3 review" },
        #{ title: "Level 3 fix" },
        #{ title: "Level 4 explore", detail: "PR 9 TUI" },
        #{ title: "Level 4 implement" },
        #{ title: "Level 4 test" },
        #{ title: "Level 4 review" },
        #{ title: "Level 4 fix" },
        #{ title: "Level 5 explore", detail: "PR 10 hardening" },
        #{ title: "Level 5 implement" },
        #{ title: "Level 5 test" },
        #{ title: "Level 5 review" },
        #{ title: "Level 5 fix" },
        #{ title: "Final validation", detail: "plan section 19 sequence" },
        #{ title: "Rollout gates", detail: "hard blockers from sections 20/21.7" },
    ],
};

// Single DeepSeek model slug for every child. Never omit and never select
// another model. Flows into ctx.model so functions (which cannot read module
// scope) still pin every child through this one constant.
const DEEPSEEK_MODEL = "openrouter:deepseek/deepseek-v4-flash-0731";

// ---------------------------------------------------------------------------
// Output schemas. Every schema rejects additional properties so real children
// can never emit the "stub" smoke-check sentinel.
// ---------------------------------------------------------------------------

let plan_schema = #{
    "type": "object",
    "required": ["prereqs_ok", "files", "steps", "risks", "gates"],
    "additionalProperties": false,
    "properties": #{
        "prereqs_ok": #{ "type": "boolean" },
        "files": #{ "type": "array", "maxItems": 40, "items": #{ "type": "string" } },
        "steps": #{ "type": "array", "maxItems": 40, "items": #{ "type": "string" } },
        "risks": #{ "type": "array", "maxItems": 20, "items": #{ "type": "string" } },
        "gates": #{ "type": "array", "maxItems": 20, "items": #{ "type": "string" } },
    },
};

let impl_schema = #{
    "type": "object",
    "required": ["files_changed", "checks_passed", "commit", "notes"],
    "additionalProperties": false,
    "properties": #{
        "files_changed": #{ "type": "array", "maxItems": 60, "items": #{ "type": "string" } },
        "checks_passed": #{ "type": "array", "maxItems": 40, "items": #{ "type": "string" } },
        "commit": #{ "type": "string" },
        "notes": #{ "type": "string" },
    },
};

let test_schema = #{
    "type": "object",
    "required": ["tests_passed", "commands", "failures"],
    "additionalProperties": false,
    "properties": #{
        "tests_passed": #{ "type": "boolean" },
        "commands": #{ "type": "array", "maxItems": 40, "items": #{ "type": "string" } },
        "failures": #{ "type": "array", "maxItems": 20, "items": #{ "type": "string" } },
    },
};

let review_schema = #{
    "type": "object",
    "required": ["approve", "blocker", "security_ok", "issues", "outside_scope"],
    "additionalProperties": false,
    "properties": #{
        "approve": #{ "type": "boolean" },
        "blocker": #{ "type": "string" },
        "security_ok": #{ "type": "boolean" },
        "issues": #{
            "type": "array",
            "maxItems": 20,
            "items": #{
                "type": "object",
                "required": ["severity", "file", "issue"],
                "additionalProperties": false,
                "properties": #{
                    "severity": #{ "type": "string" },
                    "file": #{ "type": "string" },
                    "issue": #{ "type": "string" },
                },
            },
        },
        "outside_scope": #{ "type": "array", "maxItems": 20, "items": #{ "type": "string" } },
    },
};

let rollout_schema = #{
    "type": "object",
    "required": ["decisions", "blocked"],
    "additionalProperties": false,
    "properties": #{
        "decisions": #{
            "type": "array",
            "maxItems": 10,
            "items": #{
                "type": "object",
                "required": ["item", "resolved", "evidence"],
                "additionalProperties": false,
                "properties": #{
                    "item": #{ "type": "string" },
                    "resolved": #{ "type": "boolean" },
                    "evidence": #{ "type": "string" },
                },
            },
        },
        "blocked": #{ "type": "array", "maxItems": 10, "items": #{ "type": "string" } },
    },
};

let final_validation_schema = #{
    "type": "object",
    "required": ["passed", "commands", "failures"],
    "additionalProperties": false,
    "properties": #{
        "passed": #{ "type": "boolean" },
        "commands": #{ "type": "array", "maxItems": 40, "items": #{ "type": "string" } },
        "failures": #{ "type": "array", "maxItems": 20, "items": #{ "type": "string" } },
    },
};

// ---------------------------------------------------------------------------
// Argument parsing. Guard args == () first; property access on unit throws.
// ---------------------------------------------------------------------------

let plan_path = if args == () { "" } else { if args.plan_path == () { "" } else { args.plan_path } };
let base_branch = if args == () { "" } else { if args.base_branch == () { "" } else { args.base_branch } };
let ffmpeg_headers = if args == () { true } else { if args.ffmpeg_headers == () { true } else { args.ffmpeg_headers } };
let only_pr = if args == () { "" } else { if args.only_pr == () { "" } else { args.only_pr } };
let start_level = if args == () { 0 } else { if args.start_level == () { 0 } else { args.start_level } };
let max_retries = if args == () { 1 } else { if args.max_retries == () { 1 } else { args.max_retries } };
let max_review_rounds = if args == () { 1 } else { if args.max_review_rounds == () { 1 } else { args.max_review_rounds } };
let sequential = if args == () { false } else { if args.sequential == () { false } else { args.sequential } };
let confirm_max_effort = if args == () { false } else { if args.confirm_max_effort == () { false } else { args.confirm_max_effort } };

// ---------------------------------------------------------------------------
// PR descriptors: id, level, primary crate, test filter, dependencies, owned
// paths, notes. Levels: 0=PR1, 1=PR2/3/4/5, 2=PR6, 3=PR7/8, 4=PR9, 5=PR10.
// Only PR 4 may intentionally change Cargo.lock.
// ---------------------------------------------------------------------------

let prs = [
    #{
        id: 1,
        level: 0,
        title: "Foundation seam",
        crate_name: "xai-grok-tools",
        test_filter: "media_",
        deps: [],
        owns: [
            "crates/codegen/xai-grok-tools/src/media/domain.rs",
            "crates/codegen/xai-grok-tools/src/media/backend.rs",
            "crates/codegen/xai-grok-tools/src/media/analyze_media.rs",
            "crates/codegen/xai-grok-tools/src/media/mod.rs",
            "crates/codegen/xai-grok-tools/src/registry/types.rs (ToolKind::AnalyzeMedia variant only, do NOT register)",
            "crates/codegen/xai-grok-agent/src/builder.rs",
            "crates/codegen/xai-grok-workspace/src/session/tool_config.rs",
            "crates/codegen/xai-grok-workspace/src/config.rs",
            "crates/codegen/xai-grok-workspace/src/handle.rs",
            "crates/codegen/xai-grok-shell/src/session/agent_rebuild.rs",
        ],
        notes: "Inference-free media domain types, MediaUnderstandingBackend trait, SessionContext backend resource, construction-site threading. Defer model-visible tool registration. No Cargo.lock changes.",
    },
    #{
        id: 2,
        level: 1,
        title: "Modality and transport metadata",
        crate_name: "xai-grok-shell",
        test_filter: "media_",
        deps: [1],
        owns: [
            "crates/codegen/xai-grok-models/default_models.json",
            "crates/codegen/xai-grok-models/src/lib.rs",
            "remote/provider catalog parsing for media modality/transport",
            "crates/codegen/xai-grok-shell/src/agent/config.rs (ModelInfo modality fields)",
            "models_cache.json schema v3 (MODELS_CAPABILITY_SCHEMA_VERSION 2 -> 3)",
            "ACP ModelInfo._meta media projection",
            "crates/codegen/xai-grok-pager model state and model-details UI additions",
        ],
        notes: "Tri-state MediaModalitySupport + MediaTransportCapabilities end to end. Unknown serde default; v2 cache compatibility; schema version bump to 3.",
    },
    #{
        id: 3,
        level: 1,
        title: "Configuration and reload",
        crate_name: "xai-grok-shell",
        test_filter: "media_",
        deps: [1],
        owns: [
            "[media_understanding] config section, three route lists, policies, limits, validation",
            "crates/codegen/xai-grok-shell/src/util/config/settings_writes.rs",
            "crates/codegen/xai-grok-shell/src/util/config/persist.rs",
            "crates/codegen/xai-grok-shell/src/config/watcher.rs",
            "crates/codegen/xai-grok-shell/src/config/reloader.rs (ConfigUpdate::MediaUnderstanding + LKG pattern)",
            "GROK_MEDIA_IMAGE_ROUTES / AUDIO / VIDEO env vars and legacy image route migration",
        ],
        notes: "Ordered routes with primary+fallbacks, no duplicates within a category, unresolved IDs preserved, process-local LKG only (no .lkg sidecar), typed settings writes, no credentials persisted.",
    },
    #{
        id: 4,
        level: 1,
        title: "Native FFmpeg layer",
        crate_name: "xai-grok-tools",
        test_filter: "ffmpeg",
        deps: [1],
        owns: [
            "crates/codegen/xai-grok-tools/build.rs",
            "crates/codegen/xai-grok-tools/ffmpeg/grok_av.h",
            "crates/codegen/xai-grok-tools/ffmpeg/grok_av.c",
            "crates/codegen/xai-grok-tools/src/media/ffmpeg/",
            "crates/codegen/xai-grok-tools/Cargo.toml (libloading; build-deps cc/pkg-config)",
            "Cargo.lock (the ONLY PR permitted to change dependency resolution)",
        ],
        notes: "FFmpeg 8 majors 60/62/62/9/6, per-context function-pointer table, lazy loader, bounded AVIO, RGB/PCM, cancellation, honest safety model, compile-out without headers. Review the intentional Cargo.lock diff with the pinned toolchain.",
    },
    #{
        id: 5,
        level: 1,
        title: "Artifact store and ledger",
        crate_name: "xai-grok-shell",
        test_filter: "media_",
        deps: [1],
        owns: [
            "crates/codegen/xai-grok-shell/src/session/media/artifacts.rs",
            "crates/codegen/xai-grok-shell/src/session/media/cache.rs",
            "crates/codegen/xai-grok-shell/src/session/media/ledger.rs",
            "assets/media/ store layout under the session dir",
            "crates/codegen/xai-grok-shell/src/session/fork.rs",
            "crates/codegen/xai-grok-shell/src/session/storage/jsonl/mod.rs",
            "session persistence/rewind/archive integration for artifacts",
        ],
        notes: "BLAKE3-addressed immutable objects, refs/checkpoints, index + append-only journals, conservative GC, fork hardlinks, replay semantic-cache hits.",
    },
    #{
        id: 6,
        level: 2,
        title: "Shell backend and invoker",
        crate_name: "xai-grok-shell",
        test_filter: "media_",
        deps: [1, 2, 3, 5],
        owns: [
            "crates/codegen/xai-grok-shell/src/session/media/routes.rs",
            "crates/codegen/xai-grok-shell/src/session/media/invoker.rs",
            "crates/codegen/xai-grok-shell/src/session/media/backend.rs",
            "crates/codegen/xai-grok-shell/src/session/media/transport.rs",
            "crates/codegen/xai-grok-shell/src/session/media/preprocess.rs",
            "crates/codegen/xai-grok-shell/src/session/media/consent.rs",
            "crates/codegen/xai-grok-shell/src/session/media/zdr.rs",
            "crates/codegen/xai-grok-shell/src/session/media/policy.rs",
            "crates/codegen/xai-grok-shell/src/session/media/mod.rs",
        ],
        notes: "Ordered route resolution, one independently constructed InferenceClient per route (never the parent session InferenceHandle), transport planner, structured validation/repair, circuit breaker, budgets, consent/ZDR gates, cache+ledger integration. Integrates PR 4 FFmpeg availability diagnostics. Does NOT activate the tool or compaction.",
    },
    #{
        id: 7,
        level: 3,
        title: "Tool and auto-enrichment",
        crate_name: "xai-grok-shell",
        test_filter: "media_",
        deps: [6],
        owns: [
            "crates/codegen/xai-grok-shell/src/session/media/auto_enrich.rs",
            "analyze_media tool registration/listing wiring (conditionally listed)",
            "xai-grok-tools permission classification for analyze_media",
            "crates/codegen/xai-grok-shell/src/session/prompt_build.rs (enrich_user_media decision point)",
            "GROK_DISABLE_MEDIA_ANALYZE / AUTO_ENRICH kill switches",
        ],
        notes: "Content-only tool args, two independent host gates (filesystem/tool permission then external-disclosure consent), active-model capability decision (Supported/Unsupported/Unknown policy), legacy image compatibility, recursion prevention.",
    },
    #{
        id: 8,
        level: 3,
        title: "Compaction preflight",
        crate_name: "xai-grok-shell",
        test_filter: "compaction_",
        deps: [6],
        owns: [
            "crates/codegen/xai-grok-shell/src/session/media/compaction.rs",
            "PreparedCompactionSource and prepare_media_semantics",
            "crates/codegen/xai-grok-shell/src/session/compaction.rs",
            "crates/codegen/xai-grok-shell/src/session/helpers/full_replace_compaction.rs",
            "crates/codegen/xai-grok-shell/src/session/rolling_compaction.rs",
            "crates/codegen/xai-grok-shell/src/session/two_pass.rs",
            "crates/codegen/xai-grok-shell/src/session/compaction_segments.rs",
        ],
        notes: "One preflight per stable job snapshot, pairing-safe provenance envelopes, best_effort/strict policy, sanitize_compaction_images stays as the final safety net.",
    },
    #{
        id: 9,
        level: 4,
        title: "TUI and live synchronization",
        crate_name: "xai-grok-pager",
        test_filter: "media_",
        deps: [2, 3, 6, 7],
        owns: [
            "crates/codegen/xai-grok-pager/src/settings/ (three route-list editors)",
            "crates/codegen/xai-grok-pager/src/app/effects/helpers.rs",
            "modality/auth/transport/source status badges",
            "consent and capability-override UX (Unknown acknowledge, forced Unsupported)",
            "consented sample-media route test",
            "watcher/reloader driven TUI refresh and optional media notification",
        ],
        notes: "Route CRUD/reorder, unresolved rows, Unknown/Unsupported flows, atomic rollback on persistence failure, leader-client UI refresh. Pager settings registry stays metadata-only.",
    },
    #{
        id: 10,
        level: 5,
        title: "Hardening and rollout",
        crate_name: "xai-grok-shell",
        test_filter: "media_",
        deps: [4, 7, 8, 9],
        owns: [
            "adversarial media corpus and tests",
            "ASan/UBSan fuzz harness",
            "telemetry/redaction audit",
            "kill-switch exercises",
            "macOS library-validation gate",
            "legal gate",
            "deliberate full-workspace validation",
        ],
        notes: "Rollout gates from sections 20/21.7 require orchestrator decisions on macOS hardened runtime, legal review, provider audio/video wire support, trusted ZDR metadata, structured-output support, catalog completeness. Pushes/PRs/deployment stay outside the workflow.",
    },
];

// ---------------------------------------------------------------------------
// Shared context. ctx.model is the one DeepSeek constant; every child option
// map uses model: ctx.model.
// ---------------------------------------------------------------------------

let ctx = #{
    plan_path: plan_path,
    base_branch: base_branch,
    ffmpeg_headers: ffmpeg_headers,
    max_retries: max_retries,
    max_review_rounds: max_review_rounds,
    sequential: sequential,
    confirm_max_effort: confirm_max_effort,
    model: DEEPSEEK_MODEL,
    plan_schema: plan_schema,
    impl_schema: impl_schema,
    test_schema: test_schema,
    review_schema: review_schema,
    rollout_schema: rollout_schema,
    final_validation_schema: final_validation_schema,
};

// ---------------------------------------------------------------------------
// Small helpers (pure; take all data as parameters).
// ---------------------------------------------------------------------------

fn scratch_name(pr, suffix) {
    "pr" + pr.id.to_string() + "-" + suffix + ".json"
}

fn owns_text(pr) {
    let o = "";
    for path in pr.owns {
        o += "- " + path + "\n";
    }
    o
}

fn plan_ref(ctx) {
    if ctx.plan_path == "" {
        "the approved media-understanding plan (locate plan.md in the repository root, or the path passed in args.plan_path)"
    } else {
        "the approved plan at " + ctx.plan_path
    }
}

fn bool_str(b) {
    if b { "true" } else { "false" }
}

fn is_stub_result(r) {
    r != () && r.success == true && r.output != () && r.output.stub == true
}

fn plan_ok(r) {
    is_stub_result(r) || (r != () && r.success == true && r.output != () && r.output.prereqs_ok == true)
}

fn impl_ok(r) {
    is_stub_result(r) || (r != () && r.success == true && r.output != ())
}

fn test_ok(r) {
    is_stub_result(r) || (r != () && r.success == true && r.output != () && r.output.tests_passed == true)
}

fn review_ok(r) {
    is_stub_result(r) || (r != () && r.success == true && r.output != ())
}

// Best-effort budget pre-check. Pauses only when the host reports a concrete
// remaining count (unit/None means unknown, e.g. during validate_only).
fn check_budget(needed, what, ctx) {
    let b = budget();
    if b.remaining != () && b.remaining < needed {
        await_user("back_off",
            "Workflow child budget is insufficient for " + what + " (need "
            + needed.to_string() + ", remaining " + b.remaining.to_string() + "). "
            + "Resume with resume_from_run_id and a higher agent_budget (recommended 160+, margin 256).");
    }
}

fn find_pr(prs, id_str) {
    for pr in prs {
        if pr.id.to_string() == id_str {
            return pr;
        }
    }
    ()
}

fn first_blocked_at(pr_status, level_prs) {
    let i = 0;
    while i < pr_status.len() {
        if pr_status[i] == "blocked" {
            return #{ reason: "PR " + level_prs[i].id.to_string() + " blocked" };
        }
        i += 1;
    }
    ()
}

fn rejected_indices(pr_status, pr_reviews) {
    let out = [];
    let i = 0;
    while i < pr_status.len() {
        if pr_status[i] == "ready" && (pr_reviews[i] == () || pr_reviews[i].approve != true) {
            out.push(i);
        }
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// Prompt builders.
// ---------------------------------------------------------------------------

fn build_explore_prompt(pr, ctx) {
    let p = "You are the EXPLORER for PR " + pr.id.to_string() + " ("
        + pr.title + ") of the native media-understanding subsystem for text-only models. "
        + "Read AGENTS.md and GROK.md in full before doing anything else. Read "
        + plan_ref(ctx) + " (PR section " + pr.id.to_string() + " and section 21). "
        + "Your model is " + ctx.model + ".";
    p += " This is a READ-ONLY exploration. Inspect the ACTUAL current code in this shared workspace "
        + "with read_file and grep; never answer from memory. The integration branch is "
        + json_encode(ctx.base_branch) + ".";
    p += " Verify the prerequisite merge state: required predecessor PRs are "
        + json_encode(pr.deps) + ". Set prereqs_ok=true only if you confirmed each predecessor's "
        + "changes exist in the current workspace (level ordering: PR 1 level 0; PRs 2/3/4/5 need PR 1; "
        + "PR 6 needs PRs 1,2,3,5; PRs 7/8 need PR 6; PR 9 needs PRs 2,3,6,7; PR 10 needs PRs 4,7,8,9). "
        + "If any predecessor is missing, set prereqs_ok=false and list the exact missing items in gates.";
    p += " Owned paths (the ONLY files this PR may later modify):\n" + owns_text(pr);
    p += " Return structured { prereqs_ok, files, steps, risks, gates }. files: concrete files to change "
        + "or add. steps: ordered implementation steps. risks: correctness/security risks. "
        + "gates: conditions that must hold before implementation. An empty risks list is valid only after "
        + "reading the relevant code with read_file/grep. Do not edit any files.";
    p
}

fn build_implementer_prompt(pr, plan, ctx) {
    let p = "You are the IMPLEMENTER for PR " + pr.id.to_string() + " ("
        + pr.title + "). Read AGENTS.md and GROK.md in full. Read " + plan_ref(ctx)
        + " (PR section " + pr.id.to_string() + " and section 21). Your model is " + ctx.model + ".";
    p += " Explorer blueprint: " + json_encode(plan) + ". Treat it as informed advice; verify it "
        + "against the current code before writing.";
    p += " You work DIRECTLY in the SHARED workspace (there is no isolated worktree and no merge step). "
        + "Modify ONLY the owned paths below and nothing else:\n" + owns_text(pr);
    p += " Constraints: preserve every unrelated pre-existing dirty file and every earlier PR's changes; "
        + "never run git add, commit, stash, reset, checkout, clean, rm, or modify the git index/history "
        + "- leave your changes uncommitted in the working tree; ";
    if pr.id == 4 {
        p += "PR 4 is the ONLY PR allowed to change dependency resolution: it may extend "
            + "xai-grok-tools/Cargo.toml with libloading and build-deps cc/pkg-config and regenerate "
            + "Cargo.lock (review that diff). Support BOTH build situations: compatible FFmpeg 8 headers "
            + "present (build and test the native shim) and headers absent (compile the feature out with "
            + "a clear diagnostic). ";
    } else {
        p += "do NOT modify any Cargo.toml or Cargo.lock; keep dependency resolution untouched. ";
    }
    if pr.id == 1 {
        p += "PR 1: add the inference-free media domain types, MediaUnderstandingBackend trait, "
            + "SessionContext backend resource, and construction-site threading; add ToolKind::AnalyzeMedia "
            + "but do NOT register it model-visibly yet. ";
    }
    if pr.id == 6 {
        p += "PR 6: never reuse the parent session InferenceHandle; every resolved route owns a separately "
            + "constructed InferenceClient. Clear provider-hidden fallback routing. Concrete transport must "
            + "be known before any bytes leave. ";
    }
    if pr.id == 7 {
        p += "PR 7: content-only tool args, two independent host gates (filesystem/tool permission, then "
            + "external-disclosure consent), active-model capability decision, legacy image compatibility, "
            + "recursion prevention, and the GROK_DISABLE_MEDIA_* kill switches. ";
    }
    if pr.id == 10 {
        p += "PR 10: adversarial corpus, ASan/UBSan fuzz harness, telemetry/redaction audit, kill-switch "
            + "exercises, and rollout-gate preparation. ";
    }
    p += " Validation: run the narrowest relevant helpers ONE AT A TIME and record exact results: "
        + "./grok-check.sh -p " + pr.crate_name + " then ./grok-test.sh -p " + pr.crate_name
        + " -- " + pr.test_filter + ". Waiting on Cargo's shared ./target-dev lock is expected when the "
        + "level runs parallel PRs; never delete .cargo file locks, never kill another build, never run "
        + "installed grok, make deploy, make verify, or any production profile, and never make external "
        + "model calls from tests. The helpers enforce the isolated GROK_HOME=$HOME/.grokdev profile. "
        + "Use en-US. Be idempotent: if a previous attempt left partial edits, reconcile them first. "
        + "Return structured { files_changed, checks_passed, commit, notes }.";
    p
}

fn build_tester_prompt(pr, ctx) {
    let p = "You are the TESTER for PR " + pr.id.to_string() + " ("
        + pr.title + "). Read AGENTS.md and GROK.md in full. Read " + plan_ref(ctx)
        + " (PR section " + pr.id.to_string() + "). Your model is " + ctx.model + ".";
    p += " Verify the integrated SHARED workspace state for this PR's scope. Do not edit files. Inspect "
        + "the real working-tree diff (git diff, git status) and the relevant code first, then run the "
        + "focused repository helpers ONE AT A TIME and record exact commands and results: "
        + "./grok-check.sh -p " + pr.crate_name + " then ./grok-test.sh -p " + pr.crate_name
        + " -- " + pr.test_filter + ".";
    p += " Tests must exercise failure paths wherever the change is security-sensitive (permissions, "
        + "consent, ZDR, containment, budgets, transport). Never run installed grok, make, deployment "
        + "targets, production profiles, or external model calls from tests. If a test fails for an "
        + "environment reason, report it as a failure with the exact command output; do not 'fix' code. "
        + "Return structured { tests_passed, commands, failures }.";
    p
}

fn build_reviewer_prompt(pr, ctx) {
    let p = "You are an adversarial REVIEWER for PR " + pr.id.to_string() + " ("
        + pr.title + "). Read AGENTS.md and GROK.md in full. Read " + plan_ref(ctx)
        + " (PR section " + pr.id.to_string() + "). Your model is " + ctx.model + ".";
    p += " Independently inspect the REAL integrated working-tree diff (git diff, git status) and the "
        + "relevant code with read_file/grep; do not rely on any other agent's report. Verify: "
        + "(1) the diff touches only the owned paths for this PR; (2) Cargo.lock is unchanged unless this "
        + "is PR 4; (3) focused tests exist and would fail before the change; (4) failure paths are "
        + "covered for security-sensitive surfaces; (5) dependency boundaries hold (xai-grok-tools must "
        + "never depend on xai-grok-inference; delegate and compaction requests carry no application "
        + "tools except the private _grok_capture_semantics tool; video-to-audio recursion depth is at "
        + "most 1); (6) en-US everywhere; (7) no credentials, API keys, media bytes, semantic text, or "
        + "instructions are logged.";
    if pr.id >= 6 {
        p += " Security checklist for this PR: two host gates (filesystem/tool permission, then "
            + "external-disclosure consent keyed by (provider_identity, category, purpose)); consent "
            + "re-checked before every fallback provider transmission; ZDR fail-closed and derived only "
            + "from trusted host/provider metadata; no URL input anywhere in analyze_media; no "
            + "model-visible route/consent/budget bypass fields; content-addressed session-local artifact "
            + "lifecycle safe under fork, replay, rewind, and export. ";
    }
    if pr.id == 10 {
        p += " PR 10: confirm the adversarial corpus, fuzz harness, kill-switch exercises, and redaction "
            + "audit exist; verify the rollout-blocker list is accurate. ";
    }
    p += " Set approve=true only with concrete code evidence. An empty issues list is valid only after "
        + "reading every changed relevant file. Return structured { approve, blocker, security_ok, "
        + "issues, outside_scope }.";
    p
}

fn build_fixer_prompt(pr, review, ctx) {
    let p = "You are the FIXER for PR " + pr.id.to_string() + " ("
        + pr.title + "). Read AGENTS.md and GROK.md in full. Read " + plan_ref(ctx)
        + " (PR section " + pr.id.to_string() + "). Your model is " + ctx.model + ".";
    p += " The independent reviewer flagged or rejected this PR. Reviewer output: "
        + json_encode(review) + ". Verify every claim against the actual code before acting; fix only "
        + "confirmed issues, add or strengthen the focused regression tests that prove each fix, and "
        + "leave false positives untouched with a note.";
    p += " Work DIRECTLY in the SHARED workspace; modify ONLY these owned paths:\n" + owns_text(pr);
    p += " Same constraints as the implementer: preserve unrelated changes, no git write commands, no "
        + "Cargo.lock changes unless PR 4, run the narrowest helpers one at a time, en-US, idempotent. "
        + "Return structured { files_changed, checks_passed, commit, notes }.";
    p
}

fn build_rollout_prompt(ctx) {
    let p = "You are the ROLLOUT AUDITOR for the media-understanding subsystem. Read AGENTS.md and "
        + "GROK.md in full. Read " + plan_ref(ctx) + " (sections 19, 20, and 21.7). "
        + "Your model is " + ctx.model + ".";
    p += " READ-ONLY. Inspect the integrated workspace. For each rollout blocker record resolved=true "
        + "only with concrete in-repo evidence, otherwise resolved=false with what remains: "
        + "(1) macOS hardened-runtime/library-validation compatibility for dlopen of user-installed "
        + "FFmpeg; (2) legal review of runtime FFmpeg interoperation and notice language; "
        + "(3) provider audio/video wire support; (4) trusted ZDR metadata availability; "
        + "(5) structured-output support (native JSON schema or the single private schema-capture tool); "
        + "(6) catalog completeness/source reporting. FFmpeg headers present at build time: "
        + bool_str(ctx.ffmpeg_headers) + ". Verify the native shim compiles out cleanly when headers are "
        + "absent and produces a clear diagnostic. Return structured { decisions, blocked }.";
    p
}

fn build_final_validation_prompt(ctx) {
    let p = "You are the FINAL VALIDATION agent. Read AGENTS.md and GROK.md in full. Your model is "
        + ctx.model + ".";
    p += " Run the plan's section 19 validation sequence on the integrated shared workspace, ONE helper "
        + "at a time, in order: ./grok-check.sh -p xai-grok-tools; ./grok-test.sh -p xai-grok-tools -- "
        + "media_; ./grok-check.sh -p xai-grok-shell; ./grok-test.sh -p xai-grok-shell -- media_; "
        + "./grok-test.sh -p xai-grok-shell -- compaction_; ./grok-check.sh -p xai-grok-workspace; "
        + "./grok-test.sh -p xai-grok-workspace -- session_toolset; ./grok-check.sh -p xai-grok-pager; "
        + "./grok-test.sh -p xai-grok-pager -- media_; ./grok-check.sh -p xai-grok-pager-bin; then "
        + "source ./grok-dev-env.sh and run cargo fmt --all --check and cargo check --workspace --locked.";
    p += " When compatible FFmpeg headers were present at build time, also run "
        + "./grok-test.sh -p xai-grok-tools -- ffmpeg. Never run installed grok, make deploy, make "
        + "verify, or production profiles. Record every command and its result; if any command fails, "
        + "record the exact failure. Return structured { passed, commands, failures }.";
    p
}

// ---------------------------------------------------------------------------
// Per-role runners (bounded retries; all output_schema pins the DeepSeek slug).
// ---------------------------------------------------------------------------

fn run_explorer_with_retry(pr, ctx) {
    let prompt = build_explore_prompt(pr, ctx);
    let r = agent(prompt, #{
        label: "explore:pr" + pr.id.to_string(),
        capability_mode: "read-only",
        agent_type: "general-purpose",
        model: ctx.model,
        output_schema: ctx.plan_schema,
    });
    let ok = plan_ok(r);
    let attempts = 0;
    while !ok && attempts < ctx.max_retries {
        attempts += 1;
        let retry_prompt = prompt + " This is retry " + attempts.to_string()
            + ": the previous explorer attempt failed or reported prereqs_ok != true. Re-inspect the "
            + "actual workspace carefully with read_file and grep. If a prerequisite is genuinely absent, "
            + "keep prereqs_ok=false and list the exact missing items in gates.";
        let r2 = agent(retry_prompt, #{
            label: "explore:pr" + pr.id.to_string() + "-retry" + attempts.to_string(),
            capability_mode: "read-only",
            agent_type: "general-purpose",
            model: ctx.model,
            output_schema: ctx.plan_schema,
        });
        r = r2;
        ok = plan_ok(r);
    }
    if !ok {
        write_scratch_file(scratch_name(pr, "plan-failed"), json_encode(#{ prereqs_ok: false, reason: "explorer failed or prerequisites missing after retries" }));
        ()
    } else {
        write_scratch_file(scratch_name(pr, "plan"), json_encode(r.output));
        r.output
    }
}

fn run_implementer_with_retry(pr, plan, ctx) {
    let prompt = build_implementer_prompt(pr, plan, ctx);
    let r = agent(prompt, #{
        label: "impl:pr" + pr.id.to_string(),
        capability_mode: "all",
        agent_type: "general-purpose",
        model: ctx.model,
        output_schema: ctx.impl_schema,
    });
    let ok = impl_ok(r);
    let attempts = 0;
    while !ok && attempts < ctx.max_retries {
        attempts += 1;
        let retry_prompt = prompt + " This is retry " + attempts.to_string()
            + ": the previous implementer attempt failed. Be idempotent: reconcile any partial edits "
            + "already present in the workspace, then complete the change and its validation.";
        let r2 = agent(retry_prompt, #{
            label: "impl:pr" + pr.id.to_string() + "-retry" + attempts.to_string(),
            capability_mode: "all",
            agent_type: "general-purpose",
            model: ctx.model,
            output_schema: ctx.impl_schema,
        });
        r = r2;
        ok = impl_ok(r);
    }
    if !ok {
        write_scratch_file(scratch_name(pr, "impl-failed"), json_encode(#{ success: false, reason: "implementer failed after retries" }));
        ()
    } else {
        write_scratch_file(scratch_name(pr, "impl"), json_encode(r.output));
        r.output
    }
}

fn run_tester_with_retry(pr, ctx) {
    let prompt = build_tester_prompt(pr, ctx);
    let r = agent(prompt, #{
        label: "test:pr" + pr.id.to_string(),
        capability_mode: "execute",
        agent_type: "general-purpose",
        model: ctx.model,
        output_schema: ctx.test_schema,
    });
    let ok = test_ok(r);
    let attempts = 0;
    while !ok && attempts < ctx.max_retries {
        attempts += 1;
        let retry_prompt = prompt + " This is retry " + attempts.to_string()
            + ": the previous tester attempt failed or reported tests_passed=false. Re-run the focused "
            + "helpers and report exact results; do not edit code.";
        let r2 = agent(retry_prompt, #{
            label: "test:pr" + pr.id.to_string() + "-retry" + attempts.to_string(),
            capability_mode: "execute",
            agent_type: "general-purpose",
            model: ctx.model,
            output_schema: ctx.test_schema,
        });
        r = r2;
        ok = test_ok(r);
    }
    if !ok {
        let failed = if r != () && r.output != () { r.output } else { #{ tests_passed: false, commands: [], failures: ["tester failed after retry"] } };
        write_scratch_file(scratch_name(pr, "test"), json_encode(failed));
        ()
    } else {
        write_scratch_file(scratch_name(pr, "test"), json_encode(r.output));
        r.output
    }
}

fn run_reviewer(pr, ctx) {
    let prompt = build_reviewer_prompt(pr, ctx);
    let r = agent(prompt, #{
        label: "review:pr" + pr.id.to_string(),
        capability_mode: "read-only",
        agent_type: "general-purpose",
        model: ctx.model,
        output_schema: ctx.review_schema,
    });
    if review_ok(r) {
        write_scratch_file(scratch_name(pr, "review"), json_encode(r.output));
        r.output
    } else {
        write_scratch_file(scratch_name(pr, "review"), json_encode(#{ approve: false, blocker: "reviewer agent failed", security_ok: false, issues: [], outside_scope: [] }));
        ()
    }
}

fn run_fixer_with_retry(pr, review, ctx) {
    let prompt = build_fixer_prompt(pr, review, ctx);
    let r = agent(prompt, #{
        label: "fix:pr" + pr.id.to_string(),
        capability_mode: "all",
        agent_type: "general-purpose",
        model: ctx.model,
        output_schema: ctx.impl_schema,
    });
    let ok = impl_ok(r);
    let attempts = 0;
    while !ok && attempts < ctx.max_retries {
        attempts += 1;
        let retry_prompt = prompt + " This is retry " + attempts.to_string()
            + ": the previous fixer attempt failed. Verify the reviewer's claims, reconcile partial "
            + "edits, fix confirmed issues, and rerun the narrowest helpers.";
        let r2 = agent(retry_prompt, #{
            label: "fix:pr" + pr.id.to_string() + "-retry" + attempts.to_string(),
            capability_mode: "all",
            agent_type: "general-purpose",
            model: ctx.model,
            output_schema: ctx.impl_schema,
        });
        r = r2;
        ok = impl_ok(r);
    }
    if !ok {
        write_scratch_file(scratch_name(pr, "fix-failed"), json_encode(#{ success: false, reason: "fixer failed after retries" }));
        ()
    } else {
        write_scratch_file(scratch_name(pr, "fix"), json_encode(r.output));
        r.output
    }
}

fn run_final_validation(ctx) {
    let prompt = build_final_validation_prompt(ctx);
    let r = agent(prompt, #{
        label: "final-validation",
        phase: "Final validation",
        capability_mode: "execute",
        agent_type: "general-purpose",
        model: ctx.model,
        output_schema: ctx.final_validation_schema,
    });
    if r != () && r.success == true && r.output != () {
        r.output
    } else {
        ()
    }
}

fn run_rollout_audit(ctx) {
    let prompt = build_rollout_prompt(ctx);
    let r = agent(prompt, #{
        label: "rollout-audit",
        phase: "Rollout gates",
        capability_mode: "read-only",
        agent_type: "general-purpose",
        model: ctx.model,
        output_schema: ctx.rollout_schema,
    });
    if review_ok(r) {
        r.output
    } else {
        ()
    }
}

// ---------------------------------------------------------------------------
// Single-PR pipeline (sequential mode and only_pr reruns).
// ---------------------------------------------------------------------------

fn run_single_pr(pr, ctx) {
    phase("Level " + pr.level.to_string() + " explore");
    let plan = run_explorer_with_retry(pr, ctx);
    if plan == () {
        return #{ status: "blocked", id: pr.id, reason: "explorer failed or prerequisites missing after retries" };
    }

    phase("Level " + pr.level.to_string() + " implement");
    let impl = run_implementer_with_retry(pr, plan, ctx);
    if impl == () {
        return #{ status: "blocked", id: pr.id, reason: "implementer failed after retries" };
    }

    phase("Level " + pr.level.to_string() + " test");
    let test = run_tester_with_retry(pr, ctx);
    if test == () {
        return #{ status: "blocked", id: pr.id, reason: "tester failed after retry" };
    }

    phase("Level " + pr.level.to_string() + " review");
    let review_out = run_reviewer(pr, ctx);

    let round = 0;
    while (review_out == () || review_out.approve != true) && round < ctx.max_review_rounds {
        round += 1;
        phase("Level " + pr.level.to_string() + " fix");
        let fix = run_fixer_with_retry(pr, review_out, ctx);
        if fix == () {
            return #{ status: "blocked", id: pr.id, reason: "fixer failed after retries" };
        }
        phase("Level " + pr.level.to_string() + " test");
        let test2 = run_tester_with_retry(pr, ctx);
        if test2 == () {
            return #{ status: "blocked", id: pr.id, reason: "tester failed after fix round" };
        }
        phase("Level " + pr.level.to_string() + " review");
        review_out = run_reviewer(pr, ctx);
    }

    if review_out == () || review_out.approve != true {
        return #{ status: "blocked", id: pr.id, reason: "reviewer rejection after bounded fix rounds" };
    }

    write_scratch_file(scratch_name(pr, "done"), json_encode(#{ status: "ok", id: pr.id, plan: plan, impl: impl, test: test, review: review_out }));
    #{ status: "ok", id: pr.id }
}

fn run_seq_level(level, level_prs, ctx) {
    let outcomes = [];
    for pr in level_prs {
        let outcome = run_single_pr(pr, ctx);
        outcomes.push(outcome);
        if outcome.status == "blocked" {
            return #{ status: "blocked", reason: outcome.reason };
        }
    }
    #{ status: "ok", outcomes: outcomes }
}

// ---------------------------------------------------------------------------
// Wave-based parallel level: explorer wave -> implementer wave -> tester wave
// -> reviewer wave -> bounded fix rounds. Shared workspace, disjoint ownership.
// ---------------------------------------------------------------------------

fn run_level(level, level_prs, ctx) {
    let pr_status = [];
    let pr_plans = [];
    let pr_impls = [];
    let pr_tests = [];
    let pr_reviews = [];
    let n = 0;
    while n < level_prs.len() {
        pr_status.push("pending");
        pr_plans.push(());
        pr_impls.push(());
        pr_tests.push(());
        pr_reviews.push(());
        n += 1;
    }

    // Explorer wave.
    phase("Level " + level.to_string() + " explore");
    let explore_jobs = [];
    let i = 0;
    while i < level_prs.len() {
        let pr = level_prs[i];
        explore_jobs.push(#{
            prompt: build_explore_prompt(pr, ctx),
            label: "explore:pr" + pr.id.to_string(),
            phase: "Level " + level.to_string() + " explore",
            capability_mode: "read-only",
            agent_type: "general-purpose",
            model: ctx.model,
            output_schema: ctx.plan_schema,
        });
        i += 1;
    }
    check_budget(explore_jobs.len(), "level " + level.to_string() + " explorers", ctx);
    let explore_results = parallel(explore_jobs);
    let idx = 0;
    for r in explore_results {
        let pr = level_prs[idx];
        if plan_ok(r) {
            write_scratch_file(scratch_name(pr, "plan"), json_encode(r.output));
            pr_status[idx] = "ready";
            pr_plans[idx] = r.output;
        } else {
            let plan = run_explorer_with_retry(pr, ctx);
            if plan == () {
                pr_status[idx] = "blocked";
            } else {
                pr_status[idx] = "ready";
                pr_plans[idx] = plan;
            }
        }
        idx += 1;
    }
    let first_blocked = ();
    first_blocked = first_blocked_at(pr_status, level_prs);
    if first_blocked != () {
        log("Level " + level.to_string() + " blocked: " + first_blocked.reason);
        return #{ status: "blocked", reason: first_blocked.reason };
    }

    // Implementer wave.
    phase("Level " + level.to_string() + " implement");
    let impl_jobs = [];
    let impl_of = [];
    let j = 0;
    while j < level_prs.len() {
        if pr_status[j] == "ready" {
            let pr = level_prs[j];
            impl_jobs.push(#{
                prompt: build_implementer_prompt(pr, pr_plans[j], ctx),
                label: "impl:pr" + pr.id.to_string(),
                phase: "Level " + level.to_string() + " implement",
                capability_mode: "all",
                agent_type: "general-purpose",
                model: ctx.model,
                output_schema: ctx.impl_schema,
            });
            impl_of.push(j);
        }
        j += 1;
    }
    if impl_jobs.len() > 0 {
        check_budget(impl_jobs.len(), "level " + level.to_string() + " implementers", ctx);
        let impl_results = parallel(impl_jobs);
        let jj = 0;
        for r in impl_results {
            let idx = impl_of[jj];
            let pr = level_prs[idx];
            if impl_ok(r) {
                write_scratch_file(scratch_name(pr, "impl"), json_encode(r.output));
                pr_impls[idx] = r.output;
            } else {
                let impl = run_implementer_with_retry(pr, pr_plans[idx], ctx);
                if impl == () {
                    pr_status[idx] = "blocked";
                } else {
                    pr_impls[idx] = impl;
                }
            }
            jj += 1;
        }
    }
    first_blocked = first_blocked_at(pr_status, level_prs);
    if first_blocked != () {
        log("Level " + level.to_string() + " blocked: " + first_blocked.reason);
        return #{ status: "blocked", reason: first_blocked.reason };
    }

    // Tester wave.
    phase("Level " + level.to_string() + " test");
    let test_jobs = [];
    let test_of = [];
    j = 0;
    while j < level_prs.len() {
        if pr_status[j] == "ready" {
            let pr = level_prs[j];
            test_jobs.push(#{
                prompt: build_tester_prompt(pr, ctx),
                label: "test:pr" + pr.id.to_string(),
                phase: "Level " + level.to_string() + " test",
                capability_mode: "execute",
                agent_type: "general-purpose",
                model: ctx.model,
                output_schema: ctx.test_schema,
            });
            test_of.push(j);
        }
        j += 1;
    }
    if test_jobs.len() > 0 {
        check_budget(test_jobs.len(), "level " + level.to_string() + " testers", ctx);
        let test_results = parallel(test_jobs);
        let tt = 0;
        for r in test_results {
            let idx = test_of[tt];
            let pr = level_prs[idx];
            if test_ok(r) {
                write_scratch_file(scratch_name(pr, "test"), json_encode(r.output));
                pr_tests[idx] = r.output;
            } else {
                let test = run_tester_with_retry(pr, ctx);
                if test == () {
                    pr_status[idx] = "blocked";
                } else {
                    pr_tests[idx] = test;
                }
            }
            tt += 1;
        }
    }
    first_blocked = first_blocked_at(pr_status, level_prs);
    if first_blocked != () {
        log("Level " + level.to_string() + " blocked: " + first_blocked.reason);
        return #{ status: "blocked", reason: first_blocked.reason };
    }

    // Reviewer wave.
    phase("Level " + level.to_string() + " review");
    let review_jobs = [];
    let review_of = [];
    j = 0;
    while j < level_prs.len() {
        if pr_status[j] == "ready" {
            let pr = level_prs[j];
            review_jobs.push(#{
                prompt: build_reviewer_prompt(pr, ctx),
                label: "review:pr" + pr.id.to_string(),
                phase: "Level " + level.to_string() + " review",
                capability_mode: "read-only",
                agent_type: "general-purpose",
                model: ctx.model,
                output_schema: ctx.review_schema,
            });
            review_of.push(j);
        }
        j += 1;
    }
    if review_jobs.len() > 0 {
        check_budget(review_jobs.len(), "level " + level.to_string() + " reviewers", ctx);
        let review_results = parallel(review_jobs);
        let vv = 0;
        for r in review_results {
            let idx = review_of[vv];
            let pr = level_prs[idx];
            if review_ok(r) {
                write_scratch_file(scratch_name(pr, "review"), json_encode(r.output));
                pr_reviews[idx] = r.output;
            } else {
                write_scratch_file(scratch_name(pr, "review"), json_encode(#{ approve: false, blocker: "reviewer agent failed", security_ok: false, issues: [], outside_scope: [] }));
            }
            vv += 1;
        }
    }

    // Bounded fix rounds.
    let rejected = [];
    rejected = rejected_indices(pr_status, pr_reviews);
    let round = 0;
    while rejected.len() > 0 && round < ctx.max_review_rounds {
        round += 1;
        phase("Level " + level.to_string() + " fix");

        let fix_jobs = [];
        let fix_of = [];
        let fj = 0;
        while fj < rejected.len() {
            let idx = rejected[fj];
            let pr = level_prs[idx];
            fix_jobs.push(#{
                prompt: build_fixer_prompt(pr, pr_reviews[idx], ctx),
                label: "fix:pr" + pr.id.to_string() + "-r" + round.to_string(),
                phase: "Level " + level.to_string() + " fix",
                capability_mode: "all",
                agent_type: "general-purpose",
                model: ctx.model,
                output_schema: ctx.impl_schema,
            });
            fix_of.push(idx);
            fj += 1;
        }
        if fix_jobs.len() > 0 {
            check_budget(fix_jobs.len(), "level " + level.to_string() + " fixers", ctx);
            let fix_results = parallel(fix_jobs);
            let f2 = 0;
            for r in fix_results {
                let idx = fix_of[f2];
                let pr = level_prs[idx];
                if impl_ok(r) {
                    write_scratch_file(scratch_name(pr, "fix"), json_encode(r.output));
                } else {
                    let fix = run_fixer_with_retry(pr, pr_reviews[idx], ctx);
                    if fix == () {
                        pr_status[idx] = "blocked";
                    }
                }
                f2 += 1;
            }
        }
        first_blocked = first_blocked_at(pr_status, level_prs);
        if first_blocked != () {
            log("Level " + level.to_string() + " blocked: " + first_blocked.reason);
            return #{ status: "blocked", reason: first_blocked.reason };
        }

        // Re-test wave for the rejected PRs.
        phase("Level " + level.to_string() + " test");
        let retest_jobs = [];
        let retest_of = [];
        let tj = 0;
        while tj < rejected.len() {
            let idx = rejected[tj];
            if pr_status[idx] == "ready" {
                let pr = level_prs[idx];
                retest_jobs.push(#{
                    prompt: build_tester_prompt(pr, ctx),
                    label: "test:pr" + pr.id.to_string() + "-r" + round.to_string(),
                    phase: "Level " + level.to_string() + " test",
                    capability_mode: "execute",
                    agent_type: "general-purpose",
                    model: ctx.model,
                    output_schema: ctx.test_schema,
                });
                retest_of.push(idx);
            }
            tj += 1;
        }
        if retest_jobs.len() > 0 {
            check_budget(retest_jobs.len(), "level " + level.to_string() + " re-testers", ctx);
            let retest_results = parallel(retest_jobs);
            let t2 = 0;
            for r in retest_results {
                let idx = retest_of[t2];
                let pr = level_prs[idx];
                if test_ok(r) {
                    write_scratch_file(scratch_name(pr, "test"), json_encode(r.output));
                } else {
                    let test = run_tester_with_retry(pr, ctx);
                    if test == () {
                        pr_status[idx] = "blocked";
                    }
                }
                t2 += 1;
            }
        }
        first_blocked = first_blocked_at(pr_status, level_prs);
        if first_blocked != () {
            log("Level " + level.to_string() + " blocked: " + first_blocked.reason);
            return #{ status: "blocked", reason: first_blocked.reason };
        }

        // Re-review wave for the rejected PRs.
        phase("Level " + level.to_string() + " review");
        let rereview_jobs = [];
        let rereview_of = [];
        let rj = 0;
        while rj < rejected.len() {
            let idx = rejected[rj];
            if pr_status[idx] == "ready" {
                let pr = level_prs[idx];
                rereview_jobs.push(#{
                    prompt: build_reviewer_prompt(pr, ctx),
                    label: "review:pr" + pr.id.to_string() + "-r" + round.to_string(),
                    phase: "Level " + level.to_string() + " review",
                    capability_mode: "read-only",
                    agent_type: "general-purpose",
                    model: ctx.model,
                    output_schema: ctx.review_schema,
                });
                rereview_of.push(idx);
            }
            rj += 1;
        }
        if rereview_jobs.len() > 0 {
            check_budget(rereview_jobs.len(), "level " + level.to_string() + " re-reviewers", ctx);
            let rereview_results = parallel(rereview_jobs);
            let r2 = 0;
            for r in rereview_results {
                let idx = rereview_of[r2];
                let pr = level_prs[idx];
                if review_ok(r) {
                    write_scratch_file(scratch_name(pr, "review"), json_encode(r.output));
                    pr_reviews[idx] = r.output;
                } else {
                    write_scratch_file(scratch_name(pr, "review"), json_encode(#{ approve: false, blocker: "reviewer agent failed in fix round", security_ok: false, issues: [], outside_scope: [] }));
                }
                r2 += 1;
            }
        }

        rejected = rejected_indices(pr_status, pr_reviews);
    }

    first_blocked = first_blocked_at(pr_status, level_prs);
    if first_blocked != () {
        log("Level " + level.to_string() + " blocked: " + first_blocked.reason);
        return #{ status: "blocked", reason: first_blocked.reason };
    }

    rejected = rejected_indices(pr_status, pr_reviews);
    if rejected.len() > 0 {
        let pr = level_prs[rejected[0]];
        log("Level " + level.to_string() + " blocked: PR " + pr.id.to_string() + " rejected after bounded fix rounds.");
        return #{ status: "blocked", reason: "reviewer rejection after bounded fix rounds for PR " + pr.id.to_string() };
    }

    log("Level " + level.to_string() + " signed off: " + level_prs.len().to_string() + " PR(s) implemented, tested, and reviewed.");
    write_scratch_file("level-" + level.to_string() + "-signoff.json", json_encode(#{ status: "ok", level: level, prs: level_prs.len() }));
    #{ status: "ok", level: level }
}

// ---------------------------------------------------------------------------
// Main flow.
// ---------------------------------------------------------------------------

log("media-understanding-dag starting. Child model pinned to " + ctx.model
    + "; reasoning effort is inherited (workflow API has no effort option) - see header note.");

if confirm_max_effort != true {
    await_user("verification",
        "The DeepSeek slug openrouter:deepseek/deepseek-v4-flash-0731 accepts only reasoning_effort "
        + "max|high|low, and the workflow API has no option to set child reasoning effort (children "
        + "inherit role/persona/definition/catalog defaults). The workflow avoids any effort-pinning "
        + "role (agent_type general-purpose everywhere). Before the run, configure the slug's catalog "
        + "default reasoning level to 'max' so children inherit maximum effort, then resume; or pass "
        + "args.confirm_max_effort=true to confirm this precondition and skip the gate.");
}

if only_pr != "" {
    let pr = find_pr(prs, only_pr);
    if pr == () {
        complete(#{ status: "blocked", reason: "only_pr not found: " + only_pr, summary: "single-PR rerun failed" });
    }
    let outcome = run_single_pr(pr, ctx);
    if outcome.status == "blocked" {
        complete(#{ status: "blocked", reason: outcome.reason, mode: "only_pr", pr: only_pr, summary: "single-PR rerun blocked" });
    }
    log("PR " + only_pr + " completed (single-PR rerun).");
    write_scratch_file("run-summary.json", json_encode(#{ status: "ok", mode: "only_pr", pr: only_pr }));
    complete(#{ status: "ok", mode: "only_pr", pr: only_pr, summary: "PR " + only_pr + " implemented, tested, reviewed, and integrated in the shared workspace." });
}

// Full run: dependency levels.
let level = start_level;
let all_level_results = [];
while level <= 5 {
    let level_prs = [];
    for pr in prs {
        if pr.level == level {
            level_prs.push(pr);
        }
    }
    if level_prs.len() > 0 {
        log("Starting level " + level.to_string() + " with " + level_prs.len().to_string() + " PR(s).");
        let outcome = if sequential { run_seq_level(level, level_prs, ctx) } else { run_level(level, level_prs, ctx) };
        write_scratch_file("level-summary-" + level.to_string() + ".json", json_encode(outcome));
        all_level_results.push(outcome);
        if outcome.status == "blocked" {
            log("Run blocked at level " + level.to_string() + ": " + outcome.reason);
            complete(#{ status: "blocked", reason: outcome.reason, level: level, levels: all_level_results, summary: "run blocked at level " + level.to_string() });
        }
    }
    level += 1;
}

// Final validation (plan section 19).
phase("Final validation");
let final_validation = run_final_validation(ctx);
write_scratch_file("final-validation.json", json_encode(final_validation));
if final_validation == () || final_validation.passed != true {
    await_user("verification",
        "Final validation did not pass. Inspect scratch/final-validation.json, fix the workspace, then "
        + "resume to re-run it; the workflow will block if it still fails.");
    let final2 = run_final_validation(ctx);
    write_scratch_file("final-validation.json", json_encode(final2));
    if final2 == () || final2.passed != true {
        complete(#{ status: "blocked", reason: "final validation still failing after orchestrator correction", levels: all_level_results, summary: "blocked at final validation" });
    }
}

// Rollout hard blockers (plan sections 20 and 21.7).
phase("Rollout gates");
let rollout = run_rollout_audit(ctx);
write_scratch_file("pr10-rollout.json", json_encode(rollout));
let pending = [];
if rollout != () && rollout.decisions != () {
    for d in rollout.decisions {
        if d.resolved != true {
            pending.push(d.item);
        }
    }
}
if rollout == () || pending.len() > 0 {
    await_user("verification",
        "Release hard blockers need decisions before rollout. Pending: " + json_encode(pending)
        + ". The orchestrator must record resolutions for macOS hardened-runtime/library validation, "
        + "legal review of runtime FFmpeg interoperation, provider audio/video wire support, trusted "
        + "ZDR metadata, structured-output support, and catalog completeness/source reporting. Resume "
        + "after decisions are recorded; updates can be written to scratch/pr10-rollout.json.");
}

// Final sign-off gate: pushes/PRs/deployment are outside the workflow.
await_user("user",
    "All PRs are implemented, tested, reviewed, and validated in the shared workspace. Resuming "
    + "finalizes the run result. Pushing, opening PRs, deployment, and modifying shared infrastructure "
    + "are OUTSIDE this workflow and require separate explicit user approval.");

log("Run complete. Review scratch/level-summary-*.json, scratch/pr*-{plan,impl,fix,test,review}.json, and scratch/final-validation.json.");
write_scratch_file("run-summary.json", json_encode(#{ status: "ok", levels: all_level_results, final_validation: final_validation, rollout: rollout }));
complete(#{ status: "ok", levels: all_level_results, final_validation: final_validation, rollout: rollout, summary: "media-understanding subsystem implemented across all levels in the shared workspace; pushes/PRs/deployment require separate approval." });
