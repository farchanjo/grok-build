let meta = #{
    name: "chatgpt-subscription-tui",
    description: "Execute the ChatGPT subscription TUI plan: preset/override merge fix, pager config helper, /providers ChatGPT section with context-window override, review, repair, verify",
    when_to_use: "Run to implement .grok/plans/chatgpt-subscription-tui.md end-to-end",
    phases: [
        #{ title: "Implement", detail: "shell merge fix + pager config helper (parallel)" },
        #{ title: "UI", detail: "providers modal ChatGPT subscription section" },
        #{ title: "Review", detail: "architecture + adversarial reviewers" },
        #{ title: "Repair", detail: "address verified findings" },
        #{ title: "Verify", detail: "focused check/test on shell + pager" },
    ],
};

let implementation_schema = #{
    "type": "object", "required": ["summary", "files_changed", "validation", "remaining"],
    "properties": #{
        "summary": #{ "type": "string" },
        "files_changed": #{ "type": "array", "maxItems": 20, "items": #{ "type": "string" } },
        "validation": #{ "type": "array", "maxItems": 20, "items": #{ "type": "string" } },
        "remaining": #{ "type": "array", "maxItems": 12, "items": #{ "type": "string" } },
    },
};
let review_schema = #{
    "type": "object", "required": ["approved", "findings", "evidence"],
    "properties": #{
        "approved": #{ "type": "boolean" },
        "findings": #{ "type": "array", "maxItems": 10, "items": #{
            "type": "object", "required": ["severity", "file", "issue", "fix"],
            "properties": #{
                "severity": #{ "type": "string" },
                "file": #{ "type": "string" },
                "issue": #{ "type": "string" },
                "fix": #{ "type": "string" },
            },
        }},
        "evidence": #{ "type": "string" },
    },
};
let verify_schema = #{
    "type": "object", "required": ["passed", "commands", "failures", "notes"],
    "properties": #{
        "passed": #{ "type": "boolean" },
        "commands": #{ "type": "array", "maxItems": 20, "items": #{ "type": "string" } },
        "failures": #{ "type": "array", "maxItems": 12, "items": #{ "type": "string" } },
        "notes": #{ "type": "string" },
    },
};

let rules = "Repo rules (non-negotiable, from AGENTS.md and GROK.md): ";
rules += "read AGENTS.md and GROK.md in full before editing or running commands, and read the plan at .grok/plans/chatgpt-subscription-tui.md; ";
rules += "run all development commands through the helper scripts ./grok-check.sh and ./grok-test.sh, one at a time (they enforce the isolated ~/.grokdev profile); if a Cargo lock is held by a parallel task, wait for it - never kill another build or delete lock files; ";
rules += "never run the installed grok, /opt/grok-custom, the grok-custom wrapper, or any make deploy/verify target; never read or write ~/.grok or ~/.grok-prod; ";
rules += "never use destructive git operations (git reset, git checkout, git clean, branch -D); preserve every unrelated pre-existing modification in the working tree; ";
rules += "do not edit the generated root Cargo.toml and do not modify Cargo.lock (this is a source-only change); use direct cargo only when a helper cannot express the operation, after sourcing ./grok-dev-env.sh; ";
rules += "write all new code, comments, diagnostics, test names, and user-facing text in en-US; keep every change inside the plan scope.";

phase("Implement");

let shell_prompt = "Implement Phase 1 of the plan (crate xai-grok-shell only). " + rules;
shell_prompt += " Task 1 (merge fix): in crates/codegen/xai-grok-shell/src/agent/providers.rs, install_model_presets_into currently does config_models.entry(preset.id).or_insert_with(...) (around line 748 - confirm with read_file before editing), so a user-defined [model.<preset-id>] table in config.toml REPLACES the preset ConfigModelOverride entirely. For chatgpt-* presets that breaks routing: the entry loses base_url (the codex endpoint), the model slug, and model_provider, so the ChatGPT OAuth route check (is_codex_base_url in auth/chatgpt_oauth.rs) stops matching. Change the insertion into a MERGE: when config_models already contains the preset key, produce a merged ConfigModelOverride in which every field the user set wins and every field the user left unset takes the preset value (model, base_url, max_completion_tokens, name, description, model_provider, auth_scheme, api_backend, hidden, supported_in_api, reasoning_effort, supports_reasoning_effort, supports_tools, supports_native_schema, supports_strict_tools, supports_image_input, supports_audio_input, supports_video_input, and any other Option fields). Non-Option fields (reasoning_efforts Vec, extra_headers IndexMap): use the user value when non-empty, otherwise the preset value. Behavior must stay identical for presets whose key is not in the user config, and for user tables that set every field (user fully wins). The merge must not regress the OpenAI Platform, OpenRouter, and Anthropic presets that flow through the same function.";
shell_prompt += " Task 2 (static presets): in static_chatgpt_oauth_presets (around line 1584): change context_window from 372_000 to 272_000 for gpt-5.6-sol, gpt-5.6-terra, and gpt-5.6-luna, and rewrite the comment block with the current facts: the Codex server-side default for the 5.6 family was reduced from 372k to 272k around July 18 2026 (openai/codex issues #31860 and #34619), ~1M is an opt-in via client config, and OpenCode parity is context 400k / input 272k for 5.5 and 5.6 alike. Keep 400_000 for gpt-5.5, gpt-5.4, and gpt-5.4-mini. Do not change the OAuth flow, the live catalog fetch, or the wire shaping in codex_wire.rs.";
shell_prompt += " Task 3 (tests): add focused unit tests - (i) a partial user override (only context_window) on a chatgpt preset yields a merged entry that keeps the preset base_url, model slug, and model_provider, with the user context_window winning; (ii) with no user table the resolved entry is identical to pre-change behavior; (iii) a full user table still makes user fields win.";
shell_prompt += " Validate: ./grok-check.sh -p xai-grok-shell, then ./grok-test.sh -p xai-grok-shell (start with the providers and config test families, then the full crate). Return {summary, files_changed, validation, remaining}.";

let pager_prompt = "Implement Phase 2 of the plan (crate xai-grok-pager only). " + rules;
pager_prompt += " In crates/codegen/xai-grok-pager/src/config_toml_edit.rs, generalize the existing set_hint(key, value) helper (which writes [hints].<key> into the user config.toml while preserving formatting) into a reusable set_table_field(table, key, value) that writes into the named top-level table, and add remove_table_key(table, key) which removes the key from that table (no-op when absent). Keep set_hint as a thin wrapper over set_table_field for back-compat. Preserve the existing guarantees: every other key and table stays intact, the target table is created when missing (including the file when missing), and a non-empty unparseable config file is never clobbered (return an error). Update the module docs.";
pager_prompt += " Add unit tests mirroring the existing test style in that file: create a new table ([model.\"chatgpt-gpt-5.6-sol\"] with context_window = 1000000), update an existing key, preserve unrelated tables (for example [hints]), refuse to touch a malformed file, and idempotent remove.";
pager_prompt += " Validate: ./grok-check.sh -p xai-grok-pager, then ./grok-test.sh -p xai-grok-pager (start with the config_toml_edit test family, then the full crate). Return {summary, files_changed, validation, remaining}.";

let impls = parallel([
    #{
        prompt: shell_prompt,
        label: "shell-merge",
        capability_mode: "all",
        agent_type: "general-purpose",
        model: "grok-4.6",
        output_schema: implementation_schema,
    },
    #{
        prompt: pager_prompt,
        label: "pager-helper",
        capability_mode: "all",
        agent_type: "general-purpose",
        model: "grok-4.6",
        output_schema: implementation_schema,
    },
]);
let shell_impl = ();
let pager_impl = ();
let i = 0;
for r in impls {
    if i == 0 {
        shell_impl = r;
    } else {
        pager_impl = r;
    }
    i += 1;
}
if shell_impl == () || shell_impl.success != true {
    pause("infra", "Shell merge-fix agent failed; the plan cannot continue without it.");
}
if pager_impl == () || pager_impl.success != true {
    pause("infra", "Pager config-helper agent failed; the UI phase depends on it.");
}
log("Phase 1 done: shell merge + pager config helper");

phase("UI");
let ui_prompt = "Implement Phase 3 of the plan (TUI surface). " + rules;
ui_prompt += " Completed prerequisites - Phase 1 (xai-grok-shell) report: " + json_encode(shell_impl.output) + ". Phase 2 (xai-grok-pager helper) report: " + json_encode(pager_impl.output) + ". Inspect the actual code to verify these before relying on them.";
ui_prompt += " In crates/codegen/xai-grok-pager (and only if strictly needed, a minimal field addition in xai-grok-shell provider status):";
ui_prompt += " 1) /providers modal OpenAI row: when ChatGPT OAuth is connected, include the connected account email in the card detail (for example 'Connected with ChatGPT OAuth - user@example.com'). The email already exists in the stored OAuth tokens (xai-grok-shell auth chatgpt_oauth read_tokens); extend the ProviderStatus payload the pager receives with the minimal field, falling back to today's generic text when absent. Keep it out of telemetry/debug dumps.";
ui_prompt += " 2) Add a ChatGPT subscription settings sub-view reachable from the OpenAI row (only when connected): list the chatgpt-* models from the catalog with their current context windows, and for the selected model allow setting or clearing a context_window override. Follow the existing ProviderModalMode / handle_key / put_line rendering patterns - do not invent a new UI framework.";
ui_prompt += " 3) Add ProviderCommand::SetChatgptContextWindow { model_id: String, tokens: Option<u64> } (None = clear) plus the effect in app/effects/mod.rs: spawn_blocking write via the Phase 2 config_toml_edit helpers (set the context_window key under the model table, or remove the key on clear), then a status line noting the change takes effect via config.toml hot-reload for subsequent turns and new sessions. Do not depend on a leader restart.";
ui_prompt += " 4) Validate input: integer tokens in the range 8000 to 1050000; when the value exceeds 272000 show a short note that OpenAI may apply long-context limits or pricing to the subscription.";
ui_prompt += " Add unit tests for key handling and render/snapshot coverage following the crate's existing snapshot test infrastructure (repo policy requires render, snapshot, scenario, or PTY coverage for TUI changes).";
ui_prompt += " Do not change the OAuth flow, credential storage scopes, or wire shaping. Validate: ./grok-check.sh -p xai-grok-pager (and -p xai-grok-shell too if you touched it), then ./grok-test.sh focused families plus the full crate, one helper at a time. Return {summary, files_changed, validation, remaining}.";
let ui = agent(ui_prompt, #{
    label: "tui",
    phase: "UI",
    capability_mode: "all",
    agent_type: "general-purpose",
    model: "grok-4.6",
    output_schema: implementation_schema,
});
if ui == () || ui.success != true {
    pause("infra", "TUI implementation agent failed.");
}
log("Phase 3 done: TUI ChatGPT subscription section");

phase("Review");
let review_base = "Read AGENTS.md and GROK.md, then read the plan at .grok/plans/chatgpt-subscription-tui.md. Inspect the ACTUAL working tree and diff (git status, git diff, read every changed file) - do not trust the implementation reports and do not edit anything. Implementation reports for context: " + json_encode(#{ shell: shell_impl.output, pager: pager_impl.output, tui: ui.output }) + ".";
let reviews = parallel([
    #{
        prompt: review_base + " Focus on architecture and data flow: does the merge in install_model_presets_into preserve behavior for ALL preset kinds (openai platform, openrouter, anthropic) and not only chatgpt ones? Does a user context_window override actually reach ModelEntry.info.context_window, the auto-compact budget, and the ACP totalContextTokens meta? Does the TUI config write reach the config.toml hot-reload watcher and the model picker? Is the account email surfaced minimally and safely (no new credential exposure in logs, telemetry, or debug output)?",
        label: "review-arch",
        phase: "Review",
        capability_mode: "read-only",
        agent_type: "explore",
        model: "grok-4.6",
        output_schema: review_schema,
    },
    #{
        prompt: review_base + " Be adversarial: Rust correctness (merge edge cases, toml_edit round-trips, modal state machine and key handling, Option vs explicit-empty semantics), regressions for users who previously wrote full [model.chatgpt-*] tables, test quality (do the new tests fail before the fix?), en-US compliance, scope creep beyond the plan, and any unrelated pre-existing dirty file that got touched. Report only concrete findings with file:line evidence; an empty findings list is valid only after reading every changed file.",
        label: "grok-review",
        phase: "Review",
        capability_mode: "read-only",
        agent_type: "explore",
        model: "grok-4.6",
        output_schema: review_schema,
    },
]);
let usable_reviews = 0;
let findings = [];
for r in reviews {
    if r != () && r.success {
        usable_reviews += 1;
        if r.output.findings != () {
            for f in r.output.findings {
                findings.push(f);
            }
        }
    }
}
if usable_reviews == 0 {
    pause("verification", "Both independent review agents failed; refusing an unreviewed implementation.");
}
log(findings.len().to_string() + " findings from review");

phase("Repair");
let repair = ();
if findings.len() > 0 {
    let repair_prompt = "Read AGENTS.md, GROK.md, and the plan at .grok/plans/chatgpt-subscription-tui.md, then address the review findings below in the current working tree. " + rules + " Do not blindly obey a reviewer: re-verify each claim in the code first; fix confirmed issues, add or strengthen focused regression tests, and leave false positives unchanged with a one-line justification. Keep the scope to the plan (shell merge, pager config helper, providers modal ChatGPT section). Findings: " + json_encode(findings) + " Re-run the narrowest affected ./grok-check.sh and ./grok-test.sh commands, one at a time. Return {summary, files_changed, validation, remaining}.";
    repair = agent(repair_prompt, #{
        label: "repair",
        phase: "Repair",
        capability_mode: "all",
        agent_type: "general-purpose",
        model: "grok-4.6",
        output_schema: implementation_schema,
    });
    if repair == () || repair.success != true {
        pause("infra", "Repair agent failed.");
    }
} else {
    log("No review findings - skipping repair");
}

phase("Verify");
let verify_prompt = "Final verification of the ChatGPT-subscription-TUI plan in the current working tree. Read AGENTS.md and GROK.md first. Do not edit source files. Inspect git status and the full diff; confirm unrelated pre-existing dirty files were not modified and that Cargo.lock and the root Cargo.toml are unchanged. Run in order, one at a time: ./grok-check.sh -p xai-grok-shell; ./grok-test.sh -p xai-grok-shell; ./grok-check.sh -p xai-grok-pager; ./grok-test.sh -p xai-grok-pager; ./grok-check.sh -p xai-grok-pager-bin; then source ./grok-dev-env.sh and run cargo fmt --all --check (a pre-existing fmt diff in files NOT changed by this plan is a note, not a failure; a fmt diff in changed files is a failure). Never use the installed grok, /opt/grok-custom, or make deploy/verify. Return passed=false with exact failures on any compile, test, fmt-in-changed-files, or scope violation.";
let verification = agent(verify_prompt, #{
    label: "grok-verify",
    phase: "Verify",
    capability_mode: "all",
    agent_type: "general-purpose",
    model: "grok-4.6",
    output_schema: verify_schema,
});
if verification == () || verification.success != true {
    pause("verification", "Final verification agent failed to run.");
}

fn s(v) {
    if type_of(v) == "string" { v } else { "" }
}
let report = "# ChatGPT subscription TUI - execution report\n\nPlan: .grok/plans/chatgpt-subscription-tui.md\n\n";
report += "## 1. Shell: preset/override merge + static presets\n";
report += s(shell_impl.output.summary) + "\n\n";
report += "## 2. Pager: config write helper\n";
report += s(pager_impl.output.summary) + "\n\n";
report += "## 3. Pager: TUI surface\n";
report += s(ui.output.summary) + "\n\n";
if repair != () {
    report += "## 4. Repair\n";
    report += s(repair.output.summary) + "\n\n";
}
report += "## Review findings (" + findings.len().to_string() + ")\n";
if findings.len() == 0 {
    report += "- none\n";
}
for f in findings {
    report += "- [" + s(f.severity) + "] " + s(f.file) + ": " + s(f.issue) + "\n";
}
if verification.output.passed == true {
    report += "\n## Final verification: PASSED\n";
} else {
    report += "\n## Final verification: FAILED\n";
}
let cmds = verification.output.commands;
if cmds != () {
    for c in cmds {
        report += "- " + s(c) + "\n";
    }
}
if verification.output.passed != true {
    let fails = verification.output.failures;
    if fails != () {
        for fl in fails {
            report += "FAIL: " + s(fl) + "\n";
        }
    }
}
report += "\n" + s(verification.output.notes) + "\n";
let path = write_scratch_file("report.md", report);

if verification.output.passed != true {
    await_user("verification", "Final verification FAILED - see report. Fix the listed failures and re-run the workflow, or resume to accept the result as-is.");
}

complete(#{
    plan: ".grok/plans/chatgpt-subscription-tui.md",
    shell: shell_impl.output,
    pager: pager_impl.output,
    tui: ui.output,
    repair: repair,
    findings: findings,
    verification: verification.output,
    report: path,
});
