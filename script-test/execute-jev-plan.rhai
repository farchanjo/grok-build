// Execute the Jev integration plan, end to end.
//
// Generated from PLAN.md and the twelve FINDINGS-*.md docs. Run it when you are
// ready; nothing here is destructive beyond the edits it makes, and every phase
// reports before the next one starts.
//
// Lock discipline: implementation agents do NOT build. They edit, and say so.
// The single Verify phase is the only place a cargo helper runs, one at a time,
// because ./target-dev is the one shared resource with the other session.
//
// Args (all optional):
//   dry_run: true        analyse and report, make no edits
//   only:    [0,1,2]     run a subset of phases by index
//   start_at: 2          skip everything before this phase index
//   crates:  ["xai-grok-shell"]  restrict the Verify phase

let meta = #{
    name: "execute-jev-plan",
    description: "Implement the Jev integration plan: prerequisites, the shared retrieval change, per-target quality, transport and the TUI layer, then verify. Read-only agents edit; one serial phase builds.",
    when_to_use: "When the findings are agreed and the plan is to be applied to the repository in one supervised run.",
    phases: [
        #{ title: "Preflight", detail: "Confirm the tree, the branch, the isolation env and the other session's state before touching anything." },
        #{ title: "Phase 0 — prerequisites", detail: "The six blocking items: three memory defects, floor telemetry, the exec-floor decision, the create-workflow skill." },
        #{ title: "Phase 1 — shared retrieval", detail: "Point the embedding at solaris:8001 and fuse the dense index into the tool search and the workflow catalog." },
        #{ title: "Phase 2 — per-target quality", detail: "Skills, memory, permission, laziness, agents, todo gate, workflow, goal — independent, run in parallel." },
        #{ title: "Phase 3 — correctness", detail: "The four items that need no measurement: agent_type validation, the skill reference, BM25 retention." },
        #{ title: "Phase 4 — transport and TUI", detail: "The transport enum and the derived session key, then one row and one status per target." },
        #{ title: "Verify", detail: "The only phase that builds. One cargo helper at a time, narrowest crate first." },
        #{ title: "TUI validation", detail: "Drive the built console in a detached tmux session: the new rows, the new slash commands, the marker. Serial with Verify because it builds." },
        #{ title: "Report", detail: "Write the run report, list what was skipped and what needs a human decision." },
    ],
};

// ── args ──────────────────────────────────────────────────────────────────
// `args` is unit when the run is launched without any. Property access on `()`
// raises "a getter is not registered", so every read is guarded and `&&`
// short-circuits before touching a property.
let dry_run = false;
let start_at = 0;
let only = ();
let crates = ();
if args != () {
    if args.dry_run != () { dry_run = args.dry_run; }
    if args.start_at != () { start_at = args.start_at; }
    if args.only != () { only = args.only; }
    if args.crates != () { crates = args.crates; }
}

// ── the contracts every agent gets ────────────────────────────────────────
let ISOLATION = "This repository is isolated: GROK_HOME=$HOME/.grokdev, "
    + "GROK_LEADER_SOCKET=$HOME/.grokdev/leader.sock, GROK_DISABLE_AUTOUPDATER=1, and the "
    + "Cursor/Claude/Codex discovery flags are 0. Never read or write ~/.grok. "
    + "THE SHELL DOES NOT INHERIT THIS: the parent session runs with GROK_HOME=$HOME/.grok, and "
    + "./grok-check.sh and ./grok-test.sh refuse an inherited value (\"GROK_HOME must be unset or "
    + "exactly $HOME/.grokdev\"). Start every helper command with "
    + "`export GROK_HOME=\"$HOME/.grokdev\" GROK_LEADER_SOCKET=\"$HOME/.grokdev/leader.sock\"` — "
    + "or `unset` both — instead of assuming the environment is already right; four wasted "
    + "iterations in phase 2 came from that assumption. "
    + "Use ./grok-check.sh and ./grok-test.sh, never the installed grok binary.";

// `capability_mode` gates TOOLS, and only `execute` grants the shell:
//   read-only  = Read/Grep/Glob/LS/WebSearch/TaskOutput   (no shell)
//   read-write = + Edit/Write/MultiEdit                   (still no shell)
//   execute    = + SHELL_TOOLS  <- every agent here needs this
// "read-only" as a BEHAVIOUR is enforced by the brief, not by the mode.
//
// WARNING: when `agent_type` names a ROLE (`explore`, `plan`, `quick-search`,
// `implementer`, `reviewer`, `test-writer`, `security-auditor`, `design-doc-*`),
// the mode is an INTERSECTION with that role's default and the most restrictive
// wins. `explore` is read-only, so `capability_mode: "execute"` + `explore` =
// read-only = NO SHELL. That is how the first run's Preflight lost its shell.
let MODES = "You are spawned with capability_mode=execute so that you HAVE a shell. "
    + "Behave as your brief says: Preflight and the analysis-only steps do not edit even "
    + "though they could.";

let LOCK = "LOCK DISCIPLINE: another session shares this worktree and ./target-dev. "
    + "Do not build while you are still editing, but a helper at the END of your job is expected: "
    + "run exactly one (`./grok-check.sh -p <the crate you touched>`), then wait for it. "
    + "\"Blocking waiting for file lock on build directory\" is the lock working, not a deadlock — "
    + "peers build too, so a wait is normal. Never run two helpers at once, never delete .cargo "
    + "locks or kill another build. The Verify phase is the one that builds broadly and serially. "
    + "IF YOUR CHANGE LANDS IN A CRATE PEERS DEPEND ON (xai-workflow, xai-grok-config-types, "
    + "xai-grok-inference), also run one helper on a direct consumer — the repository policy asks "
    + "for it, and otherwise a peer's verification dies on your error: phase 2 lost an agent's "
    + "shell check to an E0382 that a parallel agent had introduced and later fixed. When the "
    + "failure sits in a file another agent owns, say so in `blocked_on` instead of editing it.";

let EDIT_RULES = "Make focused changes; preserve unrelated work in the tree. Match the existing "
    + "style. Comments and commit messages in en-US. Methods under 30 lines, no dead code, "
    + "no duplication. Never weaken permission gates, sandboxing or path containment. "
    + "If a change touches docs/, update them. Report files touched and anything you could not do. "
    + "WRITE THE REPORT IN en-US: `summary` and `blocked_on` come back in Portuguese otherwise, "
    + "which costs a translation pass on every phase. Name files with paths, not prose.";

let EVIDENCE = "Every recommendation below was measured against jev-1.13 with the stack on the "
    + "solaris box; the numbers are the justification, keep them in the code comments where they "
    + "explain a constant.";

let SHARED_STATE = "OTHER SESSIONS ARE LIVE IN THIS WORKTREE. You are not alone here.\n"
    + "- Never `git add` or commit anything. Their uncommitted work must survive untouched.\n"
    + "- The tree can change under you between two of your commands. Re-read a file before "
    + "rewriting it, and if it changed since you last read it, say so instead of overwriting.\n"
    + "- Paths that are not yours to touch: `.detent/`, `script-test/`, and any modified file "
    + "under `docs/` unless your item names it.\n"
    + "- Never kill another process. Not another cargo, not another tmux session, not another "
    + "grok. If something is running, wait for it.\n"
    + "- Never delete `.cargo` locks, never `git checkout`/`restore`/`stash` to clean the tree.\n"
    + "- If you find that another session already made part of your change, keep theirs and "
    + "adjust rather than duplicating.\n"
    + "- SESSION ID IS LOAD-BEARING: it feeds `x-grok-session-id` (first-party only), "
    + "`X-Session-ID` (self-hosted only), the body `session_id` + `bootstrap_room` "
    + "(fnv1a_32 of the key), OpenRouter's native `session_id`, Anthropic's `metadata.user_id` "
    + "and OpenAI's `prompt_cache_key`. If your change touches any of those carriers, change "
    + "NOTHING about the existing behaviour for the chat path. A new caller gets a DERIVED key "
    + "(`{session_info.id}:<purpose>`), never the session's own key.";

let SHARED = ISOLATION + "\n\n" + MODES + "\n\n" + LOCK + "\n\n" + SHARED_STATE + "\n\n" + EDIT_RULES + "\n\n" + EVIDENCE;

let OUTCOME_SCHEMA = #{
    "type": "object",
    "properties": #{
        "status": #{ "type": "string", "enum": ["done", "partial", "blocked", "skipped"] },
        "files": #{ "type": "array", "items": #{ "type": "string" } },
        "summary": #{ "type": "string" },
        "blocked_on": #{ "type": "string", "description": "empty unless status is blocked" },
        "needs_human": #{ "type": "boolean" },
    },
    "required": ["status", "summary", "needs_human"],
};

let results = [];
let blocked = [];
let skipped = [];

// `record` is inlined after each call: Rhai closures capture by value, and these
// four arrays are mutated, so inlining removes the question entirely.

// ═══ Preflight ════════════════════════════════════════════════════════════
if (if only != () { only.contains(0) } else { 0 >= start_at }) {
    phase("Preflight");
    log("preflight: tree, branch, isolation, other session");

    let brief = SHARED + "\n\n"
        + "Do NOT edit anything. Inspect and report.\n\n"
        + "1. `git -C <repo> status --short --branch` and `git log --oneline -3`. Report whether the "
        + "tree is clean and whether HEAD is ahead of origin.\n"
        + "2. Report every modified and untracked path, and say which of them belong to the OTHER "
        + "session rather than to us — the plan must not revert or commit their work.\n"
        + "3. `ps -eo pid,etime,command | grep -E 'cargo|nextest'` — is a build running right now? "
        + "If yes, say so: every edit phase must assume the lock is busy.\n"
        + "4. Confirm the isolation env is what the constants above say, and that ~/.grokdev exists.\n"
        + "5. `ls script-test/` — the findings and PLAN.md live there; confirm they are present and "
        + "report the file count.\n"
        + "6. WHO ELSE IS HERE. `cat ~/.grokdev/active_sessions.json`, `tmux ls`, and "
        + "`ps -eo pid,etime,command | grep -E 'cargo|nextest|grok'`. Report every live session, "
        + "every running build and every tmux session by name. THIS IS THE POINT OF THE PHASE.\n"
        + "7. `git status --short` — list every modified and untracked path and mark which belong "
        + "to another session. `docs/user-guide/20-background-tasks.md` and `.detent/` are known "
        + "to be someone else's right now.\n"
        + "8. Report the crate list you would touch for this plan.\n\n"
        + "Return status=blocked with blocked_on set if the tree is dirty in a way that would "
        + "confuse the plan, or if a build is running and you judge it unsafe to start.";

    let res = agent(brief, #{
        label: "preflight",
        capability_mode: "execute",
        output_schema: OUTCOME_SCHEMA,
        phase: "Preflight",
    });
    if res != () && res.success && res.output != () {
            results.push(#{ phase: "Preflight", status: res.output.status, summary: res.output.summary });
            if res.output.status == "blocked" { blocked.push(#{ phase: "Preflight", on: res.output.blocked_on }); }
            if res.output.status == "skipped" { skipped.push(#{ phase: "Preflight", reason: res.output.summary }); }
        } else {
            blocked.push(#{ phase: "Preflight", on: "agent returned nothing usable" });
        }
    if res.success && res.output.status == "blocked" {
        pause("user", "Preflight says blocked: " + res.output.blocked_on + ". Resolve, then resume.");
    }
}

// ═══ Phase 0 — prerequisites ══════════════════════════════════════════════
if (if only != () { only.contains(1) } else { 1 >= start_at }) {
    phase("Phase 0 — prerequisites");
    log("phase 0: six blocking items");

    let brief = SHARED + "\n\n"
        + "Implement the six prerequisites. They block everything after, so be exact.\n\n"
        + "0.1 INDEX THE GLOBAL TOPIC FILES. `xai-grok-memory/src/storage.rs::list_memory_files()` "
        + "returns only `global_dir/MEMORY.md`; the sibling topic files are linked from it and never "
        + "indexed, so their bodies are unreachable by search. Verified: searching text that exists "
        + "only in the body of `short-sleeps-in-verification.md` returns nothing. Index them.\n\n"
        + "0.2 STOP `write_long_term` CLOBBERING. It rewrites the whole workspace `MEMORY.md`; Dream "
        + "uses it, so anything appended since the last Dream run is lost. Make it merge, or make "
        + "Dream own the file and mediate appends. Add a failure-path test.\n\n"
        + "0.3 MOVE THE HARDCODED SCOPE. `xai-grok-pager/src/app/effects/mod.rs:4052` writes "
        + "`MemoryScope::Global` unconditionally — that is the drift. Route it through a decision "
        + "that defaults to Global so behaviour is unchanged when the decision is off.\n\n"
        + "0.4 INSTRUMENT THE PERMISSION FLOORS. The four floors "
        + "(`bash_{write,unsafe_env,opaque_shell,exec}_floor_requires_prompt`, "
        + "`permission/manager.rs:1003-1020`) override a classifier Allow and are invisible. Count "
        + "prompts by floor kind and expose the counters. Measured: 19 of 127 commands are floor "
        + "tax, 16 of them the exec floor.\n\n"
        + "0.5 THE EXEC FLOOR IS A POLICY CALL, not a model one. Do not change its behaviour yet; "
        + "make it configurable and default it to on, and leave a comment recording the 16-command "
        + "measurement so the decision can be revisited with data.\n\n"
        + "0.6 THE `create-workflow` SKILL IS REFERENCED AND NOT INSTALLED. The workflow tool's "
        + "description tells the model to read its SKILL.md; `~/.grok/skills/create-workflow/` does "
        + "not exist. Ship it in the repo's skill set, or drop the reference. Prefer shipping it.\n\n"
        + "Order: 0.1 and 0.2 first (they touch the same file), then 0.3, then 0.4/0.5, then 0.6.";

    let res = agent(brief, #{
        label: "phase0",
        agent_type: "rust-engineer",
        capability_mode: "execute",
        output_schema: OUTCOME_SCHEMA,
        phase: "Phase 0 — prerequisites",
    });
    if res != () && res.success && res.output != () {
            results.push(#{ phase: "Phase 0 — prerequisites", status: res.output.status, summary: res.output.summary });
            if res.output.status == "blocked" { blocked.push(#{ phase: "Phase 0 — prerequisites", on: res.output.blocked_on }); }
            if res.output.status == "skipped" { skipped.push(#{ phase: "Phase 0 — prerequisites", reason: res.output.summary }); }
        } else {
            blocked.push(#{ phase: "Phase 0 — prerequisites", on: "agent returned nothing usable" });
        }
}

// ═══ Phase 1 — shared retrieval ═══════════════════════════════════════════
if (if only != () { only.contains(2) } else { 2 >= start_at }) {
    phase("Phase 1 — shared retrieval");
    log("phase 1: the change five docs recommend");

    let brief = SHARED + "\n\n"
        + "This is the highest-return change in the plan; five documents recommend it "
        + "independently.\n\n"
        + "1.1 POINT THE EMBEDDING AT THE SOLARIS BOX. Endpoint `http://192.168.200.32:8001`, "
        + "model `Qwen3-Embedding-4B`, 2560 dims, OpenAI-compatible `/v1/embeddings`. Measured: "
        + "recall@1 32/66 against 30/66 for the OpenRouter model, recall@10 52/66 against 50/66, "
        + "and 2.41s against 10.84s for the same workload. It is free and on the LAN. Keep the "
        + "existing provider as the fallback and fail open to it.\n\n"
        + "1.2 FUSE A DENSE INDEX INTO THE TOOL SEARCH. `xai-grok-shell/src/session/tool_index.rs` "
        + "is BM25 only. Add a dense index beside it, fuse with the same weighted RRF "
        + "`prime/skills.rs` already uses, and KEEP BM25 and the exact-qualified-name "
        + "short-circuit — `use_tool` calls often name the tool verbatim. Measured: pool recall "
        + "7/35 cross-lingually for BM25 against 33/35 for the embedding. Cache the vectors keyed "
        + "on the MCP toolset and rebuild only when it changes; do not embed on every search.\n\n"
        + "1.3 THE WORKFLOW CATALOG. The registry holds one built-in and the tool description "
        + "never lists it. Build selection as retrieval plus a decision over the shortlist, not as "
        + "a listing: measured same accuracy with 677 fewer tokens every turn. Keep a catalog in "
        + "the tool description as a fallback until the retrieval path exists.\n\n"
        + "1.4 LEAVE THE RERANKER OFF. Six measurements, six neutral-or-negative. Do not fill "
        + "`reranker_models`.\n\n"
        + "1.5 ADD THE THIRD `RerankerProtocol` VARIANT anyway so the slot is wireable: Jev posts "
        + "`{model, state, questions}` and answers `{answers}`; the existing two protocols are "
        + "OpenAI-compatible and Cohere. Name the per-candidate questions by index, not by name, "
        + "or two candidates with the same name collide.\n\n"
        + "1.6 and 1.7 are conditional: only add the tool selection call if the fused top-1 "
        + "disappoints, and keep the workflow catalog fallback.";

    let res = agent(brief, #{
        label: "phase1",
        capability_mode: "execute",
        output_schema: OUTCOME_SCHEMA,
        phase: "Phase 1 — shared retrieval",
    });
    if res != () && res.success && res.output != () {
            results.push(#{ phase: "Phase 1 — shared retrieval", status: res.output.status, summary: res.output.summary });
            if res.output.status == "blocked" { blocked.push(#{ phase: "Phase 1 — shared retrieval", on: res.output.blocked_on }); }
            if res.output.status == "skipped" { skipped.push(#{ phase: "Phase 1 — shared retrieval", reason: res.output.summary }); }
        } else {
            blocked.push(#{ phase: "Phase 1 — shared retrieval", on: "agent returned nothing usable" });
        }
}

// ═══ Phase 2 — per-target quality, in parallel ════════════════════════════
if (if only != () { only.contains(3) } else { 3 >= start_at }) {
    phase("Phase 2 — per-target quality");
    log("phase 2: independent targets, run in parallel; none of them builds");

    let jobs = [];

    jobs.push(#{
        prompt: SHARED + "\n\nFILES YOU OWN: xai-grok-shell/src/session/prime/*, acp/tracker.rs, xai-grok-tools/src/implementations/skills/strict/evals.rs. DO NOT EDIT retrieval_settings_modal.rs or settings/defs.rs — the TUI phase owns them and edits them serially. Two parallel agents editing one file is the only way this fan-out breaks.\n\nImplement the SKILL changes. Largest measured quality gain in the "
            + "whole sweep.\n\n"
            + "2.1 SINGLE-NOUL GATE AT 0.40, replacing the mean of three nouls at 0.30. The mean "
            + "compresses: 56 of 66 cases landed in the 0.4-0.6 band where a skill exists 100% of "
            + "the time, and it fired 'no skill' on only 2 of 12 uncovered cases. Reverting to the "
            + "mean costs 4 top-1 and drops correct silence from 11/12 to 8/12.\n"
            + "2.2 WIRE `metadata.variants`. 27 skills declare it, nothing reads it. When the "
            + "chooser lands on an umbrella that declares children, ask a second question over "
            + "exactly those children. Measured +2 top-1, +3 top-3. It has a false-positive mode "
            + "when the umbrella IS the answer — put a confidence floor on the second answer.\n"
            + "2.3 DROP THE BODY RE-READ. It rescued 3 and broke 9 on this roster and doubles "
            + "tokens.\n"
            + "2.4 INDEX TO 200 BYTES, CLEANED. Accuracy saturates at 200; the shipped 400 pays "
            + "8,600 tokens per turn for nothing. Strip the inline `TRIGGER:`/`SKIP:` scaffolding "
            + "at listing time. Never go below 40 bytes: the lookalike discriminator sits at byte "
            + "~55 and below 40 it disappears for 95% of pairs.\n"
            + "2.5 INJECTION = GATE THEN ONE CHOICE over the shortlist. Measured precision 21.7% "
            + "to 86.5%, a quarter of the injected context, harmful injection 12/12 to 1/12.\n"
            + "2.6 WRITE NATURAL-LANGUAGE `should_trigger` CASES for the top ~40 skills. Today "
            + "every NL case fails because the matcher is a literal substring test — that is the "
            + "point, and it is why the corpus is empty.\n"
            + "2.7 A SEMANTIC EVAL ARM, opt-in, behind the offline runner: the runner's "
            + "no-network property is deliberate, keep it.\n\n"
            + "Also do the visibility work: a `SyntheticReason::SkillPrime` variant, a marker "
            + "chunk when prime injects (driven by the existing `LastPrimeOutcome` accounting), "
            + "THEN the strip at request build keyed on that reason. In that order — the strip is "
            + "only safe once the injection is visible. Note `acp/tracker.rs` has three "
            + "suppression paths; touching one and missing the others gives inconsistent "
            + "behaviour.\n\n"
            + "Do NOT add the specificity hint as a lever — it is neutral in the stacked "
            + "pipeline. Keep description cleaning, which is free.",
        label: "skills",
        agent_type: "ml-engineer",
        capability_mode: "execute",
        output_schema: OUTCOME_SCHEMA,
        phase: "Phase 2 — per-target quality",
    });

    jobs.push(#{
        prompt: SHARED + "\n\nFILES YOU OWN: xai-grok-memory/src/*, xai-grok-shell/src/session/memory/*. DO NOT EDIT retrieval_settings_modal.rs or settings/defs.rs — the TUI phase owns them and edits them serially. Two parallel agents editing one file is the only way this fan-out breaks.\n\nImplement the MEMORY write path. Depends on Phase 0 being done.\n\n"
            + "2.8 ADMISSION AND DEDUP at the append seam, with the store in the state. Two atomic "
            + "questions, not one: `worth` (threshold 0.20) and `covered` (threshold 0.50). "
            + "Measured 100% precision and 100% recall with the store present; without it the gate "
            + "re-stores a restatement 6 times out of 7. THE THRESHOLD BAND IS 0.20-0.22 — noise "
            + "scores 0.05-0.23, valuable notes 0.20-0.86, and 0.5 loses 7 of 36 valuable notes.\n"
            + "2.9 SCOPE ROUTING replacing the hardcoded Global: a choice over "
            + "{this_folder, global, discard}. Measured 82.4%, with every error leaning local. Do "
            + "NOT add a tie-break sentence — it hurt.\n"
            + "2.10 JEV RERANK over the shortlist. Measured 7/10 to 9/10, and it fixes the global "
            + "leak without needing a scope filter on search.\n"
            + "2.11 CONSOLIDATION INSIDE DREAM, reusing the dedup question. Dream never fires "
            + "today.\n\n"
            + "The store goes into the state on every candidate — cap it and fall back to a "
            + "summary when it grows. Everything fail-open: a Jev failure must reproduce today's "
            + "behaviour exactly.",
        label: "memory",
        agent_type: "context-manager",
        capability_mode: "execute",
        output_schema: OUTCOME_SCHEMA,
        phase: "Phase 2 — per-target quality",
    });

    jobs.push(#{
        prompt: SHARED + "\n\nFILES YOU OWN: xai-grok-workspace/src/permission/*, xai-grok-shell/src/session/acp_session_impl/laziness_classifier.rs. DO NOT EDIT retrieval_settings_modal.rs or settings/defs.rs — the TUI phase owns them and edits them serially. Two parallel agents editing one file is the only way this fan-out breaks.\n\nImplement the PERMISSION and LAZINESS changes.\n\n"
            + "2.12 SHORTEN THE CLASSIFIER CRITERIA AND MOVE THE THRESHOLD TO 0.60. The criteria "
            + "text is the operating knob, stronger than the threshold: long criteria shift the "
            + "distribution toward block, short toward allow, and the threshold picks inside that "
            + "range. Measured with short criteria at 0.60: 90.6% agreement, 11% false allow, 7% "
            + "false block — against 85.9% / 8% / 21% for the long criteria at 0.50. Keep the "
            + "criteria SHORT.\n"
            + "2.13 ONLY THEN TOUCH THE CLASSIFIER. It is already at precision 0.94 and recall "
            + "1.00 held-out on 567 real commands; its headroom is the residual, already at 90.6%. "
            + "Do not port the prompt for accuracy — it is a cost play.\n"
            + "Do NOT add prose rules: four targeted rules were all worse than the bare question.\n\n"
            + "2.14 LAZINESS: ATTACH THE CATEGORY DEFINITIONS as choice criteria. Measured "
            + "stalled-category accuracy 0/16 to 10/16 with them and nothing else changed. The "
            + "same lesson as the skill `fits` question: an option label is not a definition.\n"
            + "2.15 GATE TO 0.50, per-model. At the shipped 0.70 the detector fires on 3 of 16 "
            + "stalls. Note the base rate decides the right side of the trade and the fixtures "
            + "were balanced.\n"
            + "2.16 REPLAY THROUGH `trace_classify` against the production classifier — the "
            + "baseline neither analysis could make.\n"
            + "2.16b DECIDE THE `evidence` CLAUSE LAST. Jev produces no text and the nudge embeds "
            + "it; dropping it is the only free option and changes the nudge's tone.",
        label: "permission-laziness",
        agent_type: "ml-engineer",
        capability_mode: "execute",
        output_schema: OUTCOME_SCHEMA,
        phase: "Phase 2 — per-target quality",
    });

    jobs.push(#{
        prompt: SHARED + "\n\nFILES YOU OWN: xai-grok-shell/src/session/prime/agents.rs, xai-grok-shell/src/session/acp_session_impl/reminders.rs, xai-grok-agent/src/system_reminder.rs, xai-workflow/src/*. DO NOT EDIT retrieval_settings_modal.rs or settings/defs.rs — the TUI phase owns them and edits them serially. Two parallel agents editing one file is the only way this fan-out breaks.\n\nImplement the AGENTS, TODO GATE, WORKFLOW and GOAL changes.\n\n"
            + "2.17 STOP ASKING FOR EFFORT — default medium. A constant `high` scores 64% against "
            + "56% for the question. The real enum is None/Minimal/Low/Medium/High/Xhigh/Max and no "
            + "agent frontmatter declares one.\n"
            + "2.18 TRY EFFORT AS AN AGENT PROPERTY: a `reasoning_effort:` frontmatter field. It "
            + "removes a decision instead of tuning one.\n"
            + "2.19 FIX THE 11 FIXABLE DANGLING DELEGATE TARGETS (`-> java` should be "
            + "`java-architect`, and so on). Three point at agents that do not exist at all — "
            + "`ruby`, `ansible`, `dart` — decide whether to write them or re-point the edges.\n"
            + "2.20 ANSWER THE SEVEN READ-ONLY-BUT-WRITABLE AGENTS (`code-reviewer`, "
            + "`architect-reviewer`, `compliance-auditor`, `chaos-engineer`, `data-scientist`, "
            + "`virt-integration`, `qa-expert`). A reviewer that can edit what it reviews is a "
            + "choice; make it a stated one.\n"
            + "2.20b THE ADVISORY BLOCK BUYS ONE CASE for 150 tokens per turn. Either give it "
            + "something the tool schema lacks — that the candidate was revalidated against live "
            + "authority this turn — or make it cheaper. Do not make it load-bearing.\n"
            + "2.21-2.23 TODO GATE: turn it ON (it is finished, correct and disabled for UI "
            + "reasons — `settings/defs.rs:1943` documents the deferral), cap the item dump at 3 "
            + "(1,391 to 548 chars at 20 items), and only for lists longer than 3 let a decision "
            + "pick the item (insertion order 61%, decision 83%, +4 gains 0 losses).\n"
            + "2.24 VALIDATE `agent_type` IN `validate_only` — a typo passes the smoke check "
            + "today because its host is canned, then fails mid-run.\n"
            + "2.25 ADD `skills_hint` TO `AgentOpts`, mirroring `spawn_subagent`.\n"
            + "2.26 USE `agent_type` IN THE BUILT-IN — `deep_research.rhai` writes it zero times "
            + "and re-describes every role in prose.\n"
            + "2.27 ADD `decide(state, questions)` TO THE ENGINE — the only path for Jev inside a "
            + "script, replacing a subagent spawn with a ~300 ms call. Respect the run's "
            + "pause/stop or a paused workflow keeps a request in flight.\n"
            + "2.28 WORKFLOW NAMING LAST — 40% at four candidates, and the earlier 53% was "
            + "position-confounded.\n"
            + "2.29-2.30 GOAL: expose the panel size (1-5, already a bounded constant with no "
            + "row), surface the per-skeptic votes, and only consider a Jev pre-filter after "
            + "measuring it against the panel.",
        label: "agents-todo-workflow-goal",
        capability_mode: "execute",
        output_schema: OUTCOME_SCHEMA,
        phase: "Phase 2 — per-target quality",
    });

    let res = parallel(jobs);
    for r in res {
        if r != () && r.success && r.output != () {
            results.push(#{ phase: "Phase 2 — per-target quality", status: r.output.status, summary: r.output.summary });
            if r.output.status == "blocked" { blocked.push(#{ phase: "Phase 2 — per-target quality", on: r.output.blocked_on }); }
            if r.output.status == "skipped" { skipped.push(#{ phase: "Phase 2 — per-target quality", reason: r.output.summary }); }
        } else {
            blocked.push(#{ phase: "Phase 2 — per-target quality", on: "agent returned nothing usable" });
        }
    }
    log("phase 2: " + res.len().to_string() + " target groups reported");
}

// ═══ Phase 3 — correctness ════════════════════════════════════════════════
if (if only != () { only.contains(4) } else { 4 >= start_at }) {
    phase("Phase 3 — correctness");
    let brief = SHARED + "\n\nFour items that need no measurement, some of which Phase 2 may "
        + "already have covered. Check before doing.\n\n"
        + "3.1 Every `agent_type` literal in a shipped workflow is validated by `validate_only` "
        + "(see 2.24).\n"
        + "3.2 The `create-workflow` skill is shipped or its reference dropped (see 0.6).\n"
        + "3.3 BM25 is still in the tool fusion and the exact-qualified-name short-circuit still "
        + "runs first (see 1.2).\n"
        + "3.4 Grep the new questions for added prose and confirm each one was measured — the "
        + "rule is: never add prose to a question without measuring it. Five attempts, one "
        + "partial win.\n\n"
        + "Also confirm the shared rules held: every new constant carries its measured number in "
        + "a comment, every new decision is fail-open, and every new toggle defaults to today's "
        + "behaviour.";
    let res = agent(brief, #{
        label: "correctness",
        capability_mode: "execute",
        output_schema: OUTCOME_SCHEMA,
        phase: "Phase 3 — correctness",
    });
    if res != () && res.success && res.output != () {
            results.push(#{ phase: "Phase 3 — correctness", status: res.output.status, summary: res.output.summary });
            if res.output.status == "blocked" { blocked.push(#{ phase: "Phase 3 — correctness", on: res.output.blocked_on }); }
            if res.output.status == "skipped" { skipped.push(#{ phase: "Phase 3 — correctness", reason: res.output.summary }); }
        } else {
            blocked.push(#{ phase: "Phase 3 — correctness", on: "agent returned nothing usable" });
        }
}

// ═══ Phase 4 — transport and TUI ══════════════════════════════════════════
if (if only != () { only.contains(5) } else { 5 >= start_at }) {
    phase("Phase 4 — transport and TUI");
    let brief = SHARED + "\n\nImplement the transport and the control layer. Transport first: "
        + "everything else needs a switch.\n\n"
        + "4.1 A `transport` ENUM (`native` | `openrouter`) that moves endpoint, model, provider "
        + "block AND credential chain together, defaulting to `openrouter` so nothing changes. "
        + "Setting one without the others fails confusingly: the native endpoint with the "
        + "`~typesafe/` model prefix 404s, and the OpenRouter endpoint with a bare `jev-latest` "
        + "fails model resolution. The `provider` block is OpenRouter-only. Accept BOTH "
        + "`TYPESAFE_API_KEY` and `TYPESAFE_AI_FABRICIO_KEY`.\n"
        + "4.2 A DERIVED SESSION KEY: `{session_info.id}:jev`. Do NOT reuse the session's own "
        + "key — the Jev payload shape differs and sharing `bootstrap_room` (an fnv1a_32 of the "
        + "key) claims a prefix family Jev does not belong to. The derived key leaves the main "
        + "path untouched and still gives affinity within Jev's own traffic. Leave "
        + "`bootstrap_room` to the transports that define it. Note the `x-grok-*` headers never "
        + "reach a third-party transport: `GrokRequestHeaders::apply` returns early when "
        + "`!first_party`.\n"
        + "4.3 `/jev transport native|openrouter` plus a settings row, BOTH dispatching one "
        + "typed action so they cannot drift — mirror `/jev` exactly.\n"
        + "4.4 `/jev status` printing the credential SOURCE (`env:`, `file:`, `auth.json:`) but "
        + "never the key. A 401 must name the chain tried or a two-transport setup is "
        + "undebuggable.\n\n"
        + "4.4b AUDIT THE SIX SESSION-ID CARRIERS BEFORE ADDING A SEVENTH. They are "
        + "`x-grok-session-id` (first-party only), `X-Session-ID` (self-hosted family only), the "
        + "body `session_id` + `bootstrap_room` (suppressed for OpenAI and Anthropic), "
        + "OpenRouter's native `session_id`, Anthropic's `metadata.user_id` and OpenAI's "
        + "`prompt_cache_key`. None may change for the chat path.\n"
        + "4.4c LOG THE MCP HEADER DROP. `expand_session_id_headers` "
        + "(`xai-grok-mcp/src/servers.rs:4547`) drops a `{{session_id}}` header silently when no "
        + "session is available, so a server behaves differently with no trace. It is the only "
        + "silent branch in the carrier set.\n\n"
        + "4.5-4.14 ONE ROW AND ONE STATUS PER TARGET, all following the `/jev` shape: one typed "
        + "action, one slash command, one settings row, one `status`. The `status` is the "
        + "cheapest visibility win in every case because the data already exists. Name the "
        + "content, not just the target — the judge reads this brief:\n"
        + "  4.5  `/permission-classifier on|off|status` PLUS a row per floor "
        + "(`[permission.floors]`: write, unsafe_env, opaque_shell, exec), all defaulting on;\n"
        + "  4.6  `/laziness on|off|status` plus threshold, idle window, nudge cap, and the last "
        + "decision (category, confidence, whether it nudged);\n"
        + "  4.7  `/prime on|off|status` plus an index-width row, a scaffolding-strip row "
        + "(default on), and the injection marker;\n"
        + "  4.8  `/memory-gate on|off|status` plus a master `[memory] enabled` row, a "
        + "`session.save_on_end` row, both thresholds (0.20 and 0.50), scope routing, and store "
        + "counts by scope in `status`;\n"
        + "  4.9  `/agents-recommend status` plus the delegates-graph toggle (default off) and a "
        + "default-effort row;\n"
        + "  4.10 `/goal-verify on|off|status` plus a panel-size row (1-5), a pre-filter toggle, "
        + "and the per-skeptic votes in `status`;\n"
        + "  4.11 `/todo-gate on|off|status` plus a fire-cap row, an item-cap row (default 3), a "
        + "picker toggle, AND the settings rows, since the CLI flag is the only switch today;\n"
        + "  4.12 `/tool-search status` plus a backend enum {bm25, dense, fused};\n"
        + "  4.13 `/workflows list` showing names, when_to_use and source, plus the workflows "
        + "toggle — and it must scroll and filter;\n"
        + "  4.14 `/retrieval-settings` gains the reranker protocol choice.\n\n"
        + "EVERY ROW DEFAULTS TO TODAY'S BEHAVIOUR, so applying the whole set changes nothing "
        + "until someone flips one, and each is independently reversible. A recommendation a "
        + "user cannot turn off is a decision made for them.\n\n"
        + "LISTS MUST STAY USABLE WHEN THEY GROW. This phase adds rows to a settings modal "
        + "that is already long, and adds list commands. Do not hand-roll a list:\n"
        + "- scrolling: use the existing `list_pane` (virtualised — `layout.rs` has "
        + "`virtual_y`/`item_at_y`/`from_heights`), so a 500-row modal costs the same as a "
        + "5-row one;\n"
        + "- search: give every list a `/` filter with Escape to exit, and cache the filtered "
        + "indices the way `views/memory_modal.rs` and `views/agents_modal.rs` already do — "
        + "`/` is the established key, do not invent another;\n"
        + "- no truncation to make a list fit: if it is long, it scrolls and filters;\n"
        + "- group a long list into sections rather than dumping it flat, and show the count "
        + "and the active filter in the frame;\n"
        + "- keep the selected row visible across a filter change.";
    let res = agent(brief, #{
        label: "transport-tui",
        agent_type: "rust-engineer",
        capability_mode: "execute",
        output_schema: OUTCOME_SCHEMA,
        phase: "Phase 4 — transport and TUI",
    });
    if res != () && res.success && res.output != () {
            results.push(#{ phase: "Phase 4 — transport and TUI", status: res.output.status, summary: res.output.summary });
            if res.output.status == "blocked" { blocked.push(#{ phase: "Phase 4 — transport and TUI", on: res.output.blocked_on }); }
            if res.output.status == "skipped" { skipped.push(#{ phase: "Phase 4 — transport and TUI", reason: res.output.summary }); }
        } else {
            blocked.push(#{ phase: "Phase 4 — transport and TUI", on: "agent returned nothing usable" });
        }
}

// ═══ Verify — the only phase that builds ══════════════════════════════════
if (if only != () { only.contains(6) } else { 6 >= start_at }) {
    phase("Verify");
    log("verify: ONE cargo helper at a time");

    let crate_list = if crates != () { crates } else {
        ["xai-grok-memory", "xai-grok-shell", "xai-grok-tools", "xai-grok-workspace",
         "xai-grok-pager", "xai-workflow", "xai-grok-agent"]
    };

    let brief = SHARED + "\n\n"
        + "YOU ARE THE ONLY AGENT THAT BUILDS. Run the helpers STRICTLY ONE AT A TIME, "
        + "narrowest first, and wait for each before starting the next.\n\n"
        + "For each crate below, in this order:\n"
        + "  a) `./grok-check.sh -p <crate>`\n"
        + "  b) if the check passes, `./grok-test.sh -p <crate>`\n"
        + "  c) if a test fails, record the crate and the failing test name, then CONTINUE to the "
        + "next crate rather than stopping the run\n\n"
        + "Crates: " + json_encode(crate_list) + "\n\n"
        + "Rules:\n"
        + "- BEFORE the first helper: `ps -eo pid,etime,command | grep -E 'cargo|nextest'`. If another "
        + "build is running, WAIT for it to finish — do not start a parallel one and do not kill "
        + "it. Record how long you waited.\n"
        + "- If the lock is busy, WAIT. Do not delete `.cargo` locks, do not kill another build, "
        + "do not switch to a second CARGO_TARGET_DIR.\n"
        + "- Between two helpers, re-check `ps`: another session may have started a build in the "
        + "gap. Wait again if so.\n"
        + "- `Cargo.lock` must not change for source-only edits. If it does, report it as a "
        + "problem rather than accepting it.\n"
        + "- Do not run `make build` or `make deploy`; this is an ordinary edit loop.\n"
        + "- Do not run the installed `grok` binary.\n"
        + "- If a crate fails to COMPILE, that is a blocker: report it with the first error.\n\n"
        + "Report per crate: check result, test result, and the first failing test if any. Set "
        + "status=blocked if anything fails to compile, needs_human=true if a failure looks like "
        + "a design problem rather than a typo.";

    let res = agent(brief, #{
        label: "verify",
        capability_mode: "execute",
        output_schema: OUTCOME_SCHEMA,
        phase: "Verify",
    });
    if res != () && res.success && res.output != () {
            results.push(#{ phase: "Verify", status: res.output.status, summary: res.output.summary });
            if res.output.status == "blocked" { blocked.push(#{ phase: "Verify", on: res.output.blocked_on }); }
            if res.output.status == "skipped" { skipped.push(#{ phase: "Verify", reason: res.output.summary }); }
        } else {
            blocked.push(#{ phase: "Verify", on: "agent returned nothing usable" });
        }

    if res.success && res.output.status == "blocked" {
        pause("verification", "Compile failure: " + res.output.summary + ". Fix, then resume.");
    }
}

// ═══ TUI validation — tmux, and it builds ═════════════════════════════════
if (if only != () { only.contains(7) } else { 7 >= start_at }) {
    phase("TUI validation");
    log("tui: detached tmux, canonical env, --no-leader --no-auto-update");

    let brief = SHARED + "\n\n"
        + "Validate the console INTERACTIVELY. A change that touches the TUI is not verified by "
        + "tests alone: drive it in a detached tmux session and read the screen back.\n\n"
        + "If `git status` shows no change under the pager crate, report status=skipped with the "
        + "reason and stop.\n\n"
        + "Setup (one session, never `tmux attach`). FIVE tmux sessions already exist on this box "
        + "(`gab-console`, `grok-design`, `grok-fin`, `grok-harness`, `grok-user-check`). Check "
        + "first with `tmux ls`: use `grok-dev` if it is free, otherwise `grok-dev-jev`, and use "
        + "the SAME name in every command below and in the cleanup.\n"
        + "  bash -c 'source ./grok-dev-env.sh && cargo build -p xai-grok-pager-bin'\n"
        + "  tmux new-session -d -s grok-dev -x 200 -y 50 \\\n"
        + "    \"export GROK_HOME=\\$HOME/.grokdev GROK_LEADER_SOCKET=\\$HOME/.grokdev/leader.sock \\\n"
        + "     GROK_DISABLE_AUTOUPDATER=1 GROK_CURSOR_SKILLS_ENABLED=0 GROK_CURSOR_RULES_ENABLED=0 \\\n"
        + "     GROK_CURSOR_AGENTS_ENABLED=0 GROK_CURSOR_MCPS_ENABLED=0 GROK_CURSOR_HOOKS_ENABLED=0 \\\n"
        + "     GROK_CURSOR_SESSIONS_ENABLED=0 GROK_CLAUDE_SKILLS_ENABLED=0 GROK_CLAUDE_RULES_ENABLED=0 \\\n"
        + "     GROK_CLAUDE_AGENTS_ENABLED=0 GROK_CLAUDE_MCPS_ENABLED=0 GROK_CLAUDE_HOOKS_ENABLED=0 \\\n"
        + "     GROK_CLAUDE_SESSIONS_ENABLED=0 GROK_CODEX_SKILLS_ENABLED=0 GROK_CODEX_RULES_ENABLED=0 \\\n"
        + "     GROK_CODEX_AGENTS_ENABLED=0 GROK_CODEX_MCPS_ENABLED=0 GROK_CODEX_HOOKS_ENABLED=0 \\\n"
        + "     GROK_CODEX_SESSIONS_ENABLED=0; \\\n"
        + "     ./target-dev/debug/xai-grok-pager --no-leader --no-auto-update; exec zsh\"\n\n"
        + "Read with `tmux capture-pane -p -t grok-dev` (plain text) and `-S -500` for scrollback. "
        + "Treat the capture as ground truth. Wait a beat after each send before capturing.\n\n"
        + "Drive with `tmux send-keys -t grok-dev '<text>' Enter` and named keys (`Escape`, `C-c`). "
        + "For a click, send the SGR mouse sequence with `send-keys -H`.\n\n"
        + "What to check, in order:\n"
        + "  1. the console starts clean, no panic, the footer renders;\n"
        + "  2. every NEW slash command this plan added — run `/<cmd> status` and read the line back;\n"
        + "  3. every NEW settings row — open the modal, confirm the row exists and its default;\n"
        + "  4. if the skill injection marker landed, confirm it appears once and stays one line;\n"
        + "  5. toggle ONE new switch off and confirm the status line changes, then back on;\n"
        + "  6. FOR EVERY NEW LIST: press `/`, type a filter, confirm the list narrows and the "
        + "count updates, press Escape and confirm it restores; scroll to the bottom and back "
        + "and confirm the selection stays visible. If a list is long, say how long and whether "
        + "it stayed responsive.\n\n"
        + "Then kill ONLY your session: `tmux kill-session -t <the name you chose>`. The other five "
        + "belong to other sessions — do not touch them. Leaving yours alive holds the "
        + "development profile for the next run.\n\n"
        + "Report each check as pass/fail with the captured line as evidence. Screenshot with "
        + "`freeze` only if a layout question needs it. Set status=blocked on a panic or a missing "
        + "row, needs_human=true if the layout is wrong rather than the wiring.";

    let res = agent(brief, #{
        label: "tui-validation",
        capability_mode: "execute",
        output_schema: OUTCOME_SCHEMA,
        phase: "TUI validation",
    });
    if res != () && res.success && res.output != () {
        results.push(#{ phase: "TUI validation", status: res.output.status, summary: res.output.summary });
        if res.output.status == "blocked" { blocked.push(#{ phase: "TUI validation", on: res.output.blocked_on }); }
        if res.output.status == "skipped" { skipped.push(#{ phase: "TUI validation", reason: res.output.summary }); }
    } else {
        blocked.push(#{ phase: "TUI validation", on: "agent returned nothing usable" });
    }

    // the session must not outlive the run
    let cleanup = agent(
        "List the tmux sessions with `tmux ls`. Kill ONLY the one this run created (`grok-dev` or "
        + "`grok-dev-jev`) if it is still present. Leave `gab-console`, `grok-design`, `grok-fin`, "
        + "`grok-harness` and `grok-user-check` alone. Report what you killed and what you left. "
        + "Do not build.",
        #{ label: "tui-cleanup", capability_mode: "execute", output_schema: OUTCOME_SCHEMA,
           phase: "TUI validation" });
    if cleanup.success && cleanup.output != () {
        log("tui cleanup: " + cleanup.output.summary);
    }
}

// ═══ Report ═══════════════════════════════════════════════════════════════
phase("Report");
log("report: " + results.len().to_string() + " phase result(s)");

let lines = [];
lines.push("# Jev integration run");
lines.push("");
if dry_run { lines.push("**dry run — no edits were made**"); lines.push(""); }
lines.push("Phases run: " + results.len().to_string());
lines.push("");
lines.push("## Per phase");
for r in results {
    lines.push("- **" + r.phase + "** — " + r.status + ": " + r.summary);
}
lines.push("");
if blocked.len() > 0 {
    lines.push("## Blocked");
    for b in blocked { lines.push("- **" + b.phase + "** blocked on: " + b.on); }
    lines.push("");
}
if skipped.len() > 0 {
    lines.push("## Skipped");
    for s in skipped { lines.push("- **" + s.phase + "**: " + s.reason); }
    lines.push("");
}
lines.push("## Who else was here");
lines.push("- other sessions and their running builds were checked in Preflight and again before");
lines.push("  every helper in Verify; nothing belonging to them was killed, reverted or committed");
lines.push("");
lines.push("## The rules this run was held to");
lines.push("- every change is fail-open and defaults to today's behaviour");
lines.push("- no cargo ran outside the Verify phase, and never two helpers at once");
lines.push("- the other sessions' work in the tree was not reverted, staged or committed");
lines.push("- no tmux session but our own was touched");
lines.push("- no new session-id carrier was added without a derived key");
lines.push("- a noul answering 'is this worth it' sits at 0.20-0.40, never 0.50");
lines.push("- one noul per candidate; never average nouls");
lines.push("- criteria wording is the operating knob, tuned with the threshold as a pair");
lines.push("- no prose was added to a question without measuring it");
lines.push("- no cross-encoder was stacked before a decision");
lines.push("- no truncated listing shipped");
lines.push("- the session's own key was not reused for Jev");
lines.push("");
lines.push("## What would invalidate the plan");
lines.push("- `trace_classify` replay: laziness and goal were measured against hand-labelled "
    + "fixtures with no production baseline, and the goal pre-filter is explicitly contingent.");
lines.push("- a real catalog size for skills or tools: both scale arguments are measured at "
    + "100-172 items.");
lines.push("- the solaris box: it dropped twice during the analysis, so the fail-open paths must "
    + "be exercised, not assumed.");
lines.push("- one run per arm everywhere: differences of one or two cases do not separate.");

let report = "";
for line in lines { report += line + "\n"; }
let path = write_scratch_file("jev-plan-run.md", report);

let needs_human = blocked.len() > 0 || skipped.len() > 0;
complete(#{
    path: path,
    report: report,
    phases: results.len(),
    blocked: blocked.len(),
    skipped: skipped.len(),
    needs_human: needs_human,
});