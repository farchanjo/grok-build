// Provider Adapter Factory — plan-execution workflow.
// Runs the phases of the reviewed plan (docs in plan.md) as one sequential
// chain of specialist spawns. Sequential by necessity: every phase validates
// with a cargo helper against the single shared ./target-dev lock (AGENTS.md:
// one helper at a time). Halts at the first non-green phase.
//
// Launch:  /workflows  or  Workflow name="provider-factory"
// Args:    #{ repo: "/abs/path/repo", planPath: "/abs/path/plan.md" }  (optional)
//
// NOTE on skills: workflow spawns receive NO auto-primed skills — the brief is
// the spawn's only context. Domain rules the phases need are therefore inlined
// in RULES below (activation markers: run_terminal_command, nextest,
// grok-check.sh/grok-test.sh). agent_type is set per phase: rust-engineer for
// code phases, general-purpose for the validate gate, architect-reviewer for
// the final review.

let meta = #{
    name: "provider-factory",
    description: "Executes the reviewed Provider Adapter Factory plan end to end: P0 lenient reasoning-delta deserializer; P1 provider/ module (one ProviderAdapter trait, six per-kind impls, factory); P2 real Zai identity + fail-closed dialect config; P3a/P3b behavior-identical wiring of auth/route/body/pacing sites; P4 WireCodec split + reasoning echo/strip (context-overflow protection); P6a BackendNegotiator behind default-off flag; P6b per-provider sampling pools. Then workspace-wide validation, a live tmux TUI pass driving /providers field-by-field on every provider kind (render, round-trip, zero error banners/panics), and an architecture review vs plan. One phase per spawn with best-fit agent type (rust-engineer code phases, general-purpose validate, qa-expert tmux pass, architect-reviewer final), strictly sequential (shared cargo lock), halt on first failure, no commits.",
    phases: [
        #{ title: "P0 Deserializer", detail: "lenient_reasoning_delta in xai-grok-inference-types + golden serde tests" },
        #{ title: "P1 provider module", detail: "trait + ProviderPolicy + 6 adapters + factory, unwired, unit tests green" },
        #{ title: "P2 identity+config", detail: "ProviderIdentity::Zai, kind->identity 1:1, dialect TOML fail-closed" },
        #{ title: "P3a auth+route wiring", detail: "adapter consulted at auth/route/rate-limit sites, behavior byte-identical" },
        #{ title: "P3b body+pacing wiring", detail: "request_extensions/pacing/fallback sites folded into adapter" },
        #{ title: "P4 wire codec+echo", detail: "WireCodec per backend; echo/strip policy; vLLM reasoning capture" },
        #{ title: "P6a negotiation", detail: "BackendNegotiator behind provider.negotiate_backends=false default" },
        #{ title: "P6b provider pools", detail: "sampling pools via ProviderPoolKey only when tuning/http1_only set" },
        #{ title: "Validate+Review", detail: "workspace-wide checks, tmux TUI field-by-field provider validation, then architecture review vs plan" },
    ],
};

let repo = "/Users/farchanjo/dev/grok-build";
let plan = "/Users/farchanjo/.grok/sessions/%2FUsers%2Ffarchanjo%2Fdev%2Fgrok-build/01a07796-1a68-7a60-acd6-5e1f290e59d6/plan.md";
// All phase spawns run on DeepSeek V4 Flash Vision Exp (ZDR): tools + vision,
// 1M ctx, effort menu max/high/low (pinned to high via the [model."zdr:..."]
// config block — workflow spawns carry no per-spawn effort field). Insulates
// the run from the flaky self-hosted vLLM provider seen today.
let MODEL = "zdr:deepseek/deepseek-v4-flash-vision-exp";
if args != () && type_of(args) == "Object" {
    if args.repo != () { repo = args.repo; }
    if args.planPath != () { plan = args.planPath; }
}

// ---------------------------------------------------------------- shared rules
// Inlined here because workflow spawns carry no task context and no skills:
// this block IS the project's rust + dev-env skill surface for every spawn.
let RULES = "REPOSITORY: " + repo + " (Rust 2024 workspace, grok-build fork).\n" +
"CONTEXT CONTRACT: this brief is your ONLY task context — nothing else is primed into your session (no skills, no hooks). Everything you need is below; work only with read/edit/run_terminal_command style tools.\n" +
"MANDATORY DEV ENVIRONMENT (repo AGENTS.md; your shell may inherit GROK_HOME pointing at a production profile — strip it):\n" +
"- Every validation command runs as: env -u GROK_HOME -u GROK_LEADER_SOCKET ./grok-check.sh -p <crate>  (and ./grok-test.sh -p <crate> [-- name-filter]). These helpers enforce the isolated ~/.grokdev profile.\n" +
"- Run ONE cargo helper at a time (single shared ./target-dev lock). Waiting on the cargo lock is expected; never delete locks or kill builds.\n" +
"- Bare cargo only via: bash -c 'source ./grok-dev-env.sh && cargo ...' in one command line. NEVER make, never the installed grok binary, never write to ~/.grok.\n" +
"RUST RULES (project playbook, inlined): edition 2024 (let-chains, unsafe attrs OK); pinned toolchain via rust-toolchain.toml, do not touch it; #[expect] over #[allow]; no unwrap/expect outside tests; thiserror for error enums; rustfmt-clean; en-US comments/tests; test runner is cargo-nextest via the helper scripts (filter with ./grok-test.sh -p <crate> -- <filter>).\n" +
"EDIT DISCIPLINE:\n" +
"- First read the full plan at: " + plan + " — the phase spec below is an excerpt; on conflict the plan wins.\n" +
"- Touch ONLY files your phase needs; preserve unrelated work in the tree. Do not stage, do not commit.\n" +
"- Cargo.lock, Cargo.toml dependency tables, lint tables, rust-toolchain are OFF LIMITS (no new dependencies, period).\n" +
"RETURN CONTRACT: you must return JSON matching the given schema exactly. status='ok' ONLY if every validation command listed in your spec finished green. On any failure return status='failed' with the failing command and the exact error tail in 'notes'. Never claim ok with unrun validations.";

let PHASE_SCHEMA = #{
    "type": "object",
    "required": ["status", "summary", "files_changed", "tests_run"],
    "properties": #{
        "status": #{ "type": "string", "enum": ["ok", "failed"] },
        "summary": #{ "type": "string" },
        "files_changed": #{ "type": "array", "items": #{ "type": "string" } },
        "tests_run": #{ "type": "string", "description": "exact validation commands executed and their outcomes" },
        "notes": #{ "type": "string" },
    },
};

let REVIEW_SCHEMA = #{
    "type": "object",
    "required": ["verdict", "summary"],
    "properties": #{
        "verdict": #{ "type": "string", "enum": ["ship", "fix-needed"] },
        "summary": #{ "type": "string" },
        "findings": #{ "type": "array", "items": #{ "type": "string" } },
    },
};

// ------------------------------------------------------------------ phase specs
// Each entry: id, agent (best-fit subagent type), effort, spec (detailed task).
let SPECS = [
#{
id: "P0-deserializer", title: "P0 Deserializer", agent: "rust-engineer", effort: "high",
spec:
"PHASE 0 — lenient reasoning-delta deserializer.\n" +
"In crates/codegen/xai-grok-inference-types: add lenient_reasoning_delta to src/serde_helpers.rs (String -> Some, empty string -> None, Unit/null -> None, Seq/Map/Bool/Number -> ignore -> None; NEVER Err). Replace the existing #[serde(alias = \"reasoning\")] on ChatChunkDelta.reasoning_content in src/types.rs (around line 696) with #[serde(default, alias = \"reasoning\", deserialize_with = \"crate::serde_helpers::lenient_reasoning_delta\")]. Keep serialization on the canonical reasoning_content name. Update/extend the existing alias test: cover key-absent, key-null, both keys present (last wins), object-shaped value, array-shaped value, empty string, and the canonical serialize-name assertion. The old test chat_chunk_delta_reasoning_alias_matches_vllm_wire must keep passing or be folded into the new table.\n" +
"VALIDATE: env -u GROK_HOME -u GROK_LEADER_SOCKET ./grok-check.sh -p xai-grok-inference-types  THEN  env -u GROK_HOME -u GROK_LEADER_SOCKET ./grok-test.sh -p xai-grok-inference-types (full suite green)."
},
#{
id: "P1-provider-module", title: "P1 provider module", agent: "rust-engineer", effort: "high",
spec:
"PHASE 1 — provider/ module in crates/codegen/xai-grok-inference, zero call-site changes.\n" +
"Create src/provider/ with: mod.rs (trait ProviderAdapter: Send + Sync + 'static — object-safe, all-sync methods per the plan section 'Two ports + one collaborator': id, auth -> AuthScheme, policy() -> &'static ProviderPolicy bundling route_kind/backend_preference(: &'static [ApiBackend], 1..=3 entries)/usage_policy/pacing_default, request_extensions, reasoning_wire -> ReasoningWire (request-side keys + echo only), rate_limit_threshold(env), classify_error, detect_fallback, and the default no-op fn shape_delta(&self, delta: &mut ChatChunkDelta). Give every trait method a sensible default body so future port growth does not churn all impls. dialect.rs: WireDialect as a #[non_exhaustive] enum (Standard, Vllm, ...) fail-closed from TOML strings. Six impl files, one per kind: xai.rs, openai.rs, compatible.rs (dialect-parameterized), anthropic.rs, openrouter.rs, zai.rs. Behavior values must be transcribed verbatim from today's branch sites listed in the plan (client.rs, retry.rs, route_context.rs, pacing.rs, agent/config.rs) — read them first; no behavior changes anywhere else. Factory: provider/mod.rs ProviderFactory::build(...) exhaustive match on kind + dialect. Register mod in lib.rs. Write thorough pure unit tests per adapter + a table test over all six (policy values, delta shaping for vLLM hoist and OpenRouter reasoning_details gating).\n" +
"VALIDATE: env -u GROK_HOME -u GROK_LEADER_SOCKET ./grok-check.sh -p xai-grok-inference  THEN  env -u GROK_HOME -u GROK_LEADER_SOCKET ./grok-test.sh -p xai-grok-inference (full suite green — module is unwired, everything existing must stay green)."
},
#{
id: "P2-identity-config", title: "P2 identity+config", agent: "rust-engineer", effort: "high",
spec:
"PHASE 2 — real Zai identity + fail-closed dialect config surface.\n" +
"(a) Add ProviderIdentity::Zai in crates/codegen/xai-grok-inference/src/config.rs (serde snake_case; Custom stays default). Audit every exhaustive match on ProviderIdentity — route_context.rs legacy_from_config gains a Zai arm with RouteAuthority::Unverified and a pinned test; retry.rs and any others get explicit arms (no wildcards). (b) In crates/codegen/xai-grok-shell/src/agent/config.rs, provider_identity_for_model (~line 6147) maps kind->identity 1:1 (Zai stops collapsing into Custom). (c) ModelProviderConfig gains optional dialect (fail-closed unknown values; default Standard); plumb to a new InferenceConfig.wire_dialect: Option field (serde-default None). (d) Note for later phases: Zai now stops inheriting non-OpenRouter defaults — that is the intended visible fix; keep Zai pacing/authority behavior conservative (Unverified, no pacing).\n" +
"VALIDATE: env -u GROK_HOME -u GROK_LEADER_SOCKET ./grok-check.sh -p xai-grok-inference && ./grok-check.sh -p xai-grok-shell (run sequentially), then ./grok-test.sh -p xai-grok-inference and ./grok-test.sh -p xai-grok-shell -- provider_identity (both green, including new Zai arm tests and serde round-trips)."
},
#{
id: "P3a-auth-route-wiring", title: "P3a auth+route wiring", agent: "rust-engineer", effort: "high",
spec:
"PHASE 3a — wire the adapter into InferenceClient at auth/identity/route sites (behavior-preserving).\n" +
"InferenceClient::new builds Arc<dyn ProviderAdapter> via ProviderFactory from the config (identity + wire_dialect). Migrate these sites to consult the adapter/policy, with byte-identical wire output: client.rs first-party header gate (~841), route metadata fields (~945-947), route_context.rs kind mapping (~312 now via adapter.route_kind()), retry.rs rate-limit resolution (~65) and its call site request_task.rs (~443) via adapter.rate_limit_threshold(env) preserving GROK_OPENROUTER_RATE_LIMIT_RETRIES. Keep is_openrouter()/is_first_party() as delegating shims (other call sites remain until P3b). Add golden tests: per identity, snapshot outgoing headers + resolved threshold old-vs-new (the pinned post_emits_single_authorization / bearer_resolver tests must stay green untouched).\n" +
"VALIDATE: env -u GROK_HOME -u GROK_LEADER_SOCKET ./grok-check.sh -p xai-grok-inference && ./grok-check.sh -p xai-grok-shell, then ./grok-test.sh -p xai-grok-inference (FULL suite green)."
},
#{
id: "P3b-body-pacing-wiring", title: "P3b body+pacing wiring", agent: "rust-engineer", effort: "high",
spec:
"PHASE 3b — body-extension + pacing + fallback sites (behavior-preserving).\n" +
"Migrate remaining provider branch sites onto the adapter: the OpenRouter fallback-detection gate in chat_completions.rs (~325) and client.rs (~2698) via adapter.detect_fallback(); pacing default in actor/pacing.rs (~273) via adapter.policy().pacing_default() (route override + openrouter_pacing flag precedence unchanged); in xai-grok-shell agent/config.rs (~6279 onward) the per-kind field filters (X-OpenRouter-Metadata header, fallback models, provider preferences, plugins, zai_tool_stream/zai_thinking) fold into adapter request_extensions contributions while the shell keeps only the identity/dialect decision. Body/headers must serialize byte-identically for all kinds: extend the golden tables to request bodies (OpenRouter reasoning object, zdr flag, preferences, zai thinking) and the pacing matrix.\n" +
"VALIDATE: env -u GROK_HOME -u GROK_LEADER_SOCKET ./grok-check.sh -p xai-grok-inference && ./grok-check.sh -p xai-grok-shell, then ./grok-test.sh -p xai-grok-inference (FULL) and ./grok-test.sh -p xai-grok-shell -- openrouter (all green)."
},
#{
id: "P4-codec-echo", title: "P4 wire codec+echo", agent: "rust-engineer", effort: "high",
spec:
"PHASE 4 — WireCodec split + reasoning echo/strip (the context-overflow protection).\n" +
"(a) Introduce a per-backend WireCodec (three impls inside stream/chat_completions.rs, stream/responses.rs, stream/messages.rs): the codec owns SSE framing, [DONE]-vs-finish-delta, the single typed from_str parse, stop-reason normalization, in-band error shapes. The chat_completions codec additionally calls adapter.reasoning_wire() and adapter.shape_delta(&mut ChatChunkDelta) per delta — keep the typed single-parse pump: NO serde_json::Value re-parse of chunks anywhere. The Anthropic messages codec stays inside the repository-owned anthropic client (its adapter supplies auth/extensions/preference only — no dead polymorphism). (b) Request-build site: apply echo policy from the adapter — ReasoningEcho::Strip drops reasoning_content from replayed assistant messages when dialect=Vllm; Echo/Details stay byte-identical to today. (c) MockInferenceServer-based tests: for each of the six kinds drive an SSE stream on the relevant endpoint through the pump and assert delta channels, usage, and echoed bodies byte-exact; include the echo/strip pair for compatible+Vllm.\n" +
"VALIDATE: env -u GROK_HOME -u GROK_LEADER_SOCKET ./grok-check.sh -p xai-grok-inference && ./grok-check.sh -p xai-grok-shell, then ./grok-test.sh -p xai-grok-inference (FULL) plus ./grok-test.sh -p xai-grok-shell -- reasoning (all green)."
},
#{
id: "P6a-negotiation", title: "P6a negotiation", agent: "rust-engineer", effort: "high",
spec:
"PHASE 6a — BackendNegotiator, behind a default-OFF flag (this phase changes behavior).\n" +
"New provider/negotiation.rs: BackendNegotiator using the repo latch pattern (AtomicU8 states UNRESOLVED/CHAT/RESPONSES/MESSAGES, compare_exchange single-writer, wait-free load, read once per stream outside the chunk loop), generation-tagged via binding_generation so model re-bind/config reload invalidates (rcu adopt newer-generation, never bare store). Behavior when provider.negotiate_backends=true: on a first-attempt 404 or backend-unsupported error AND when api_backend was NOT explicitly configured, downgrade once along adapter.policy().backend_preference filtered client-side to supported caps, cache the result, and surface an error naming BOTH attempted backends. Explicitly configured api_backend NEVER auto-downgrades and never writes the cache. Default false: byte-identical behavior to P4 end state. Tests: downgrade-once, no-downgrade-when-explicit, generation invalidation, flag-off no-op, error-message shape.\n" +
"VALIDATE: env -u GROK_HOME -u GROK_LEADER_SOCKET ./grok-check.sh -p xai-grok-inference, then ./grok-test.sh -p xai-grok-inference (FULL green, negotiation tests included)."
},
#{
id: "P6b-provider-pools", title: "P6b provider pools", agent: "rust-engineer", effort: "high",
spec:
"PHASE 6b — per-provider sampling pools (R5: multiplexing tunable per endpoint).\n" +
"In shared_http.rs: sampling keeps the single shared client as default (zero behavior change). Route sampling through the existing provider_client(ProviderPoolKey{ pool: \"sampling\", provider: <id>, connect_timeout (effective_provider_connect_timeout), http1_only }) registry ONLY when the adapter's pool_tuning() differs from default OR http1_only=true. Add http1_only: Option<bool> as one additive field to ProviderPoolTuning (plumbing from [model_providers.<id>] TOML dialect/transport surface already established in P2). Per-provider pool then multiplexes its own h2 connection set (keepalive + window knobs already in build_provider_client); the existing rcu first-build-wins registry and wait-free reads must not change. Integration-style test mirroring shared_http_wire conventions: two provider pool keys yield two clients; unconfigured provider still gets the shared client; http1_only pins its pool.\n" +
"VALIDATE: env -u GROK_HOME -u GROK_LEADER_SOCKET ./grok-check.sh -p xai-grok-inference, then ./grok-test.sh -p xai-grok-inference (FULL green) and run the shared_http wire integration binaries if present in this tree (note their env ownership: run them standalone)."
},
];

// ------------------------------------------------------------------ execution
let results = [];
let stopped_at = "";

for spec in SPECS {
    phase(spec.title);
    let prompt = "You are executing ONE phase of a reviewed refactor plan (Provider Adapter Factory) in the grok-build Rust workspace. Work autonomously end-to-end for this phase; no questions.\n\n" +
        RULES + "\n\n" + spec.spec +
        "\n\nWhen done: re-run your VALIDATE block one final time after any last edit, then return the JSON result. tests_run must list every command you ran with pass/fail. Keep 'summary' to 2-4 sentences.";
    let opts = #{
        label: spec.id + " [effort:" + spec.effort + "]",
        agent_type: spec.agent,
        model: MODEL,
        capability_mode: "all",
        output_schema: PHASE_SCHEMA,
    };
    let r = agent(prompt, opts);
    let ok = r != () && r.success && r.output != () && r.output.status == "ok";
    if !ok {
        // Provider 502s and dropped spawns are observed on this stack: one
        // automatic retry before halting. The brief warns the retrier that
        // partial edits from the dead attempt may exist in the tree.
        log(spec.id + ": first attempt not green — retrying once");
        r = agent(prompt + "\n\nRETRY NOTE: a previous attempt at this phase died mid-run and may have left PARTIAL edits of this phase in the working tree (uncommitted). Inspect the current state of the phase's target files first, keep what is correct, finish or redo the rest, then validate.", opts);
    }
    let ok = r != () && r.success && r.output != () && r.output.status == "ok";
    let entry = #{
        phase: spec.id,
        agent: spec.agent,
        status: if ok { "ok" } else { "failed" },
        summary: if r != () && r.success && r.output != () { r.output.summary } else { "spawn failed or returned nothing" },
        files: if r != () && r.success && r.output != () && r.output.files_changed != () { r.output.files_changed } else { [] },
        tests: if r != () && r.success && r.output != () && r.output.tests_run != () { r.output.tests_run } else { "" },
    };
    results.push(entry);
    log(spec.id + " [" + spec.agent + "]: " + entry.status + " — " + entry.summary);
    if !ok {
        stopped_at = spec.id;
        log("halting: " + spec.id + " not green (phases are ordered; later phases depend on it)");
        break;
    }
}

// ------------------------------------------------------- final validate+review
let workspace_ok = stopped_at == "";
if workspace_ok {
    phase("Validate+Review");
    let v = agent(
        "Final gate for the Provider Adapter Factory refactor in " + repo + ". You change NOTHING (read + run checks only).\n" + RULES +
        "\n\nRun, strictly one at a time: ./grok-check.sh -p xai-grok-inference-types, then -p xai-grok-inference, then -p xai-grok-shell (all three via env -u GROK_HOME -u GROK_LEADER_SOCKET). Then ./grok-test.sh -p xai-grok-inference-types and ./grok-test.sh -p xai-grok-inference (full nextest). Report exact totals. Also run git status --porcelain and git diff --stat and confirm the change set touches ONLY: xai-grok-inference-types (serde_helpers, types), xai-grok-inference (provider/, client.rs, config.rs, retry.rs, route_context.rs, stream/*, actor/*, shared_http.rs, lib.rs, anthropic/error.rs — the latter is a verified pre-existing test fix for the 'rate limit' keyword spelling), xai-grok-shell (agent/config.rs, agent/model_providers.rs, provider registry plumbing). Any file outside that set, any Cargo.lock diff, or any Cargo.toml dependency diff = failed.",
        #{ label: "validate [effort:medium]", agent_type: "general-purpose", model: MODEL, capability_mode: "all", output_schema: PHASE_SCHEMA });
    workspace_ok = v != () && v.success && v.output != () && v.output.status == "ok";
    results.push(#{ phase: "workspace-validate", agent: "general-purpose", status: if workspace_ok { "ok" } else { "failed" },
        summary: if v != () && v.success && v.output != () { v.output.summary } else { "validate spawn failed" },
        files: [], tests: if v != () && v.success && v.output != () && v.output.tests_run != () { v.output.tests_run } else { "" } });
    log("workspace-validate: " + (if workspace_ok { "ok" } else { "failed" }));
}

// ---- live TUI validation via detached tmux (AGENTS.md sanctioned procedure) ----
let tmux_ok = workspace_ok;
if workspace_ok {
    let t = agent(
        "LIVE TUI VALIDATION of the provider surface after the Provider Adapter Factory refactor in " + repo + ". You change NO source files (config under the dev profile may be written by the UI itself).\n" +
        RULES + "\n\n" +
        "TUI PROCEDURE (mandatory, follow exactly):\n" +
        "1. Build first: bash -c 'source ./grok-dev-env.sh && cargo build -p xai-grok-pager-bin' (one helper at a time; the shared lock wait is fine).\n" +
        "2. Create a detached tmux session with the isolated dev profile INLINE (a detached session does not inherit your exports):\n" +
        "   tmux new-session -d -s grok-pf -x 200 -y 50 \"export GROK_HOME=\\\"\\$HOME/.grokdev\\\" GROK_LEADER_SOCKET=\\\"\\$HOME/.grokdev/leader.sock\\\" GROK_DISABLE_AUTOUPDATER=1 GROK_CURSOR_SKILLS_ENABLED=0 GROK_CLAUDE_SKILLS_ENABLED=0 GROK_CODEX_SKILLS_ENABLED=0 GROK_CURSOR_RULES_ENABLED=0 GROK_CLAUDE_RULES_ENABLED=0 GROK_CODEX_RULES_ENABLED=0; ./target-dev/debug/xai-grok-pager --no-leader --no-auto-update; exec zsh\"\n" +
        "3. Observe ONLY via tmux capture-pane -p -t grok-pf (never attach; wait ~1s after each send-keys so async render settles). Interact via tmux send-keys -t grok-pf '<keys>' Enter / named keys (Escape, Down, Tab...). Mouse: send SGR bytes with send-keys -H (column from capture via awk index()).\n" +
        "4. Drive: type '/providers' Enter to open the providers modal. If tmux is missing, report status=failed with 'tmux missing' — do NOT install software.\n\n" +
        "VALIDATION MATRIX — for EVERY provider kind row visible (xai, openai/openai_compatible, openrouter, anthropic, zai, plus any custom): open its editor and inspect EVERY field row with Down + capture-pane: DisplayName, Kind, BaseUrl, AdminBaseUrl, DefaultBackend, AuthScheme, EnvKey, AdminEnvKey, CapabilityMode, CatalogTtl, RequestTimeout, PoolMaxIdle, PoolIdleTimeout, PoolConnectTimeout (pool fields are the P6b plumbing — they must render and round-trip), Enabled, headers summary, capability toggles, and the OpenRouter options block (fallbacks, sort, data collection, pacing, zdr, require params) when kind=openrouter. For each field verify: (a) label+value render on one row without truncation/overlap/garbage glyphs; (b) edit round-trip: enter a benign value, save (Save action), reopen, value persisted; (c) NO error banner, red/panic text, or '[Error'/'panicked' anywhere in capture-pane -S -200 scrollback; (d) the new kind=dialect plumbing does not break the Kind field: set an openai_compatible provider's kind, verify choices still list cleanly. Also exercise: ToggleEnabled off/on, RefreshCatalog (network-free providers may show a graceful error — graceful = ok, panic = failed), add-provider wizard for one throwaway openai_compatible entry (id pf-tui-test, base url http://127.0.0.1:9/v1), then remove it (ConfirmTypedId flow). A field-level or provider-level failure is recorded but keep walking the rest so the report is complete.\n" +
        "5. Finish: Escape back to chat, type '/quit' Enter (or C-c C-q), then tmux kill-session -t grok-pf. Leave the dev profile providers config as the UI left it.\n\n" +
        "STATUS: ok only if all providers' full field matrices passed with zero panics/error banners; any render defect, failed round-trip, or panic = failed, and 'notes' MUST contain the offending capture-pane excerpt (trimmed) and the exact provider+field where it happened. 'tests_run' must list every capture/interaction step you actually performed.",
        #{ label: "tui-tmux-validation [effort:high]", agent_type: "qa-expert", model: MODEL, capability_mode: "all", output_schema: PHASE_SCHEMA });
    tmux_ok = t != () && t.success && t.output != () && t.output.status == "ok";
    results.push(#{ phase: "tui-tmux-validation", agent: "qa-expert", status: if tmux_ok { "ok" } else { "failed" },
        summary: if t != () && t.success && t.output != () { t.output.summary } else { "tui spawn failed or returned nothing" },
        files: [], tests: if t != () && t.success && t.output != () && t.output.tests_run != () { t.output.tests_run } else { "" } });
    log("tui-tmux-validation: " + (if tmux_ok { "ok" } else { "failed" }));
    workspace_ok = tmux_ok;
}

let review = #{ verdict: "skipped", summary: "not run (a phase failed or was skipped)", findings: [] };
if workspace_ok {
    let rev = agent(
        "Architecture review, read-only, of the finished Provider Adapter Factory refactor in " + repo + " against its plan at " + plan + ".\n" +
        "CONTEXT CONTRACT: this brief is your ONLY context; you likely have NO terminal — use read_file/grep/list_dir on the changed files (find them under crates/codegen/xai-grok-inference*/src; the plan lists exact paths). If a terminal IS available, cross-check with git diff. Check the implementation and the new provider/ module against the plan's non-negotiables: (1) one ProviderAdapter trait, exactly six impls keyed on config kind, dialects as data inside compatible.rs; (2) trait object-safe, sync, defaults on methods; (3) chunk hot path still a single typed parse — no serde_json::Value per chunk; (4) anthropic/client.rs still the sole x-api-key/anthropic-version emitter and its codec lives in its own client; (5) Custom/openai_compatible wire behavior byte-identical unless dialect=vllm; (6) negotiation behind provider.negotiate_backends default OFF, downgrade-once, generation-tagged; (7) sampling default path still the shared client; (8) no new dependencies, Cargo.lock untouched; (9) all six adapters table-tested. Verdict 'ship' only if every check passes; otherwise list concrete findings with file:line. Change NOTHING.",
        #{ label: "arch-review [effort:high]", agent_type: "architect-reviewer", model: MODEL, capability_mode: "read-write", output_schema: REVIEW_SCHEMA });
    if rev != () && rev.success && rev.output != () {
        review = #{ verdict: rev.output.verdict, summary: rev.output.summary,
            findings: if rev.output.findings != () { rev.output.findings } else { [] } };
    }
    log("arch-review: " + review.verdict + " — " + review.summary);
}

complete(#{
    stopped_at: stopped_at,
    phases: results,
    arch_review: review,
    deployed: false,
    next: "Review the working tree, then commit phase-by-phase and run make deploy + make verify explicitly (fcustom flow).",
});
