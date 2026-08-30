let meta = #{
    name: "openrouter-native-harness-resume",
    description: "Resume the OpenRouter-native harness after a dead model route: Accounts, Sampler, and Verify slices only (Catalog and Picker already landed and verified). Runs on an explicit replacement model.",
    when_to_use: "When the original openrouter-native-harness run stalled mid-run and the remaining slices (multi-account keys, sampler payloads, verification) still need to execute.",
    phases: [
        #{ title: "Accounts", detail: "Extra OpenRouter keys, secret scopes, cache isolation" },
        #{ title: "Sampler", detail: "Identity-gated payloads, clamp output, generation diagnostics" },
        #{ title: "Verify", detail: "Focused grok-check and tests under ~/.grokdev" },
    ],
};

fn ok(result) {
    result != () && result.success
}

fn line(label, result) {
    if result == () {
        return label + ": missing";
    }
    if result.success {
        return label + ": ok";
    }
    label + ": failed"
}

fn dev_rules() {
    let p = "";
    p += "You work in /Users/farchanjo/dev/grok-build on branch fcustom.\n";
    p += "Language: United States English (en-US) for all authored content, comments, tests, diagnostics, and user-facing strings.\n";
    p += "Read AGENTS.md and GROK.md before editing or running commands.\n";
    p += "Development isolation is mandatory: GROK_HOME=${HOME}/.grokdev, GROK_LEADER_SOCKET=${GROK_HOME}/leader.sock, GROK_DISABLE_AUTOUPDATER=1.\n";
    p += "Always sanitize inherited host variables: prefix commands with env -u GROK_HOME -u GROK_LEADER_SOCKET when sourcing grok-dev-env.sh or running helpers from a shell that may carry another GROK_HOME.\n";
    p += "Prefer ./grok-check.sh and ./grok-test.sh. Run one Cargo helper at a time against ./target-dev. Source ./grok-dev-env.sh before any direct cargo command.\n";
    p += "Start narrow: -p xai-grok-shell, then -p xai-grok-inference if you touched it, then -p xai-grok-pager if you touched the TUI.\n";
    p += "Do not run make deploy, make verify, make build, or the installed grok / grok-custom wrapper.\n";
    p += "Do not modify Cargo.lock unless a manifest change requires it. Do not touch ~/.grok or ~/.grok-prod.\n";
    p += "Keep unrelated working-tree changes. Inspect git diff and the current files before editing.\n";
    p += "Do not invent model slugs, credentials, or context windows. Unknown stays unknown.\n";
    p += "HARD WALLS: never send OpenRouter fields (models[], provider, plugins, reasoning object, X-OpenRouter-*) for xAI, ChatGPT OAuth, OpenAI platform, Anthropic, or openai_compatible.\n";
    p += "Never borrow xAI session tokens, XAI_API_KEY, openai::api_key, or openai::oauth for an OpenRouter instance.\n";
    p += "Built-in OpenRouter catalog ids stay openrouter:<upstream>. Extra instances use <instance_id>:<upstream> (recommended ids openrouter-home / openrouter-work). Reserved ids: xai, grok, openai, chatgpt, codex, openrouter, anthropic, admin, local.\n";
    p += "OpenRouter top_provider.max_completion_tokens is a CAPABILITY CEILING, not a request default. Do not copy it onto ModelInfo.max_completion_tokens / InferenceConfig.max_completion_tokens or every non-empty prompt fails context validation. Store it as a separate ceiling and clamp only when a session policy sets a request max.\n";
    p += "xAI team ZDR (GrokAuth::is_zdr_team) is not OpenRouter provider.zdr. Do not hide Grok/ChatGPT models when an OpenRouter instance enables ZDR.\n";
    p += "Known pre-existing failures you must NOT fix or treat as caused by your slice: session::acp_session::auth_error_no_retry_tests::reconstruct_catalog_miss_openrouter_host_no_xai_bearer_resolver and reconstruct_openrouter_vault_model_no_xai_bearer_resolver (stale is_none() assertions from fdc66d4 superseded by 118f27b exact-route binding). Report them as pre-existing and move on.\n";
    p += "Do not add mold/linker flags. Do not kill Cargo locks.\n";
    p
}

fn accounts_prompt() {
    let p = dev_rules();
    p += "TASK: implement-multi-account-openrouter-keys\n";
    p += "You own extra OpenRouter accounts and credential isolation.\n";
    p += "NOTE: prior slices (Catalog, Picker) already landed in this tree; inspect current code before editing and avoid duplicating existing fields.\n";
    p += "Primary files:\n";
    p += "- crates/codegen/xai-grok-shell/src/provider_registry/secrets.rs\n";
    p += "- crates/codegen/xai-grok-shell/src/provider_registry/id.rs (reserved ids already exist; keep them)\n";
    p += "- crates/codegen/xai-grok-shell/src/agent/providers.rs stored_api_key / inject_configured_instance_catalogs\n";
    p += "- crates/codegen/xai-grok-shell/src/provider_registry/management/mod.rs catalog refresh scopes\n";
    p += "- crates/codegen/xai-grok-shell/src/cli/provider_cmd.rs set/clear key routing\n";
    p += "- crates/codegen/xai-grok-shell/src/agent/provider_catalog/refresh.rs kind fork\n";
    p += "Implement:\n";
    p += "1) Extra kind=openrouter instances store application keys as openrouter::<instance_id>::api_key (optional admin openrouter::<id>::admin_key). Built-in remains openrouter::api_key. Do not write extra OpenRouter keys as openai_compatible::<id>::api_key.\n";
    p += "2) parse_secret_scope and allowlists must accept openrouter::<configured-id>::api_key without admitting built-in aliases (openai, chatgpt, xai, grok, anthropic) as configured OpenRouter ids.\n";
    p += "3) stored_api_key / refresh / test-connection: extra OpenRouter reads its own scope. Missing key fails closed. Never fall back to built-in OpenRouter, sibling, ChatGPT OAuth, or xAI session.\n";
    p += "4) Extra OpenRouter catalogs use the native adapter (kind fork already in refresh.rs). Skip inject into singleton openrouter_models_cache.json (inject_configured_instance_catalogs already skips id openrouter; keep that).\n";
    p += "5) Credits/generation/pacing stay per instance. A 401/429 on Work must not drop Home, xAI, or ChatGPT.\n";
    p += "6) Tests: scope parse, sibling isolation, reserved ids, extra instance canonical ids openrouter-work:vendor/model vs built-in openrouter:vendor/model.\n";
    p += "After edits: ./grok-check.sh -p xai-grok-shell and focused grok-test.sh for secrets / provider_registry / providers.\n";
    p += "Reply with files changed, tests run, remaining gaps in en-US. Do not commit.\n";
    p
}

fn sampler_prompt() {
    let p = dev_rules();
    p += "TASK: implement-openrouter-sampler-payloads\n";
    p += "You own OpenRouter inference payloads and post-call diagnostics.\n";
    p += "Primary files:\n";
    p += "- crates/codegen/xai-grok-inference/src/client.rs\n";
    p += "- crates/codegen/xai-grok-inference/src/config.rs\n";
    p += "- crates/codegen/xai-grok-shell/src/agent/config.rs inference_config_for_model\n";
    p += "- OpenRouter generation/credits only if a small existing client path can be wired without a new platform\n";
    p += "Implement:\n";
    p += "1) Keep emitting models[], provider, plugins, reasoning object ONLY when ProviderIdentity::OpenRouter. openai_compatible and hostname openrouter.ai without identity must not gain extensions (pacing hostname fallback may stay).\n";
    p += "2) When the OpenRouter instance has provider.zdr true, the request provider.zdr must be true. Catalog filter and request flag stay consistent.\n";
    p += "3) Clamp request max_completion_tokens to max_output_ceiling when a request max is set. If no request max is set, omit it (do not send the raw ceiling).\n";
    p += "4) reasoning stays the native object (effort/max_tokens/exclude). Do not send flat reasoning_effort to OpenRouter when the object is used.\n";
    p += "5) include_message_model_id stays false for OpenRouter.\n";
    p += "6) If GET /generation can be attached after a completed OpenRouter turn with the existing OpenRouterClient without destabilizing the actor, do it and record provider/finish/tokens on diagnostics (secret-free). If that would be a large new actor path, add a clear TODO comment in one place and skip rather than half-wiring.\n";
    p += "7) Tests in xai-grok-inference: extensions omitted for Custom/OpenAi/Xai; zdr flag only when identity OpenRouter; clamp ceiling; existing openrouter_regression tests still pass.\n";
    p += "After edits: ./grok-check.sh -p xai-grok-inference then ./grok-check.sh -p xai-grok-shell, then focused grok-test.sh -p xai-grok-inference -- openrouter\n";
    p += "Reply with files changed, tests run, remaining gaps in en-US. Do not commit.\n";
    p
}

fn verify_prompt() {
    let p = dev_rules();
    p += "TASK: verify-openrouter-native-harness\n";
    p += "You do not add features. You verify the working tree implements the OpenRouter native harness slices (Catalog and Picker landed earlier; Accounts and Sampler just ran).\n";
    p += "Use grep/read_file to confirm:\n";
    p += "- OpenRouter discovery still does not set request max_completion_tokens from the ceiling.\n";
    p += "- ZDR filter is instance-local (provider.zdr) and does not hide xAI/ChatGPT.\n";
    p += "- Extra OpenRouter keys are not openai_compatible::<id>::api_key.\n";
    p += "- OpenRouter wire extensions remain identity-gated.\n";
    p += "- Modalities (image/audio/video/file) and tools/efforts flow onto ModelInfo/ACP for OpenRouter rows.\n";
    p += "Run, one helper at a time:\n";
    p += "1) ./grok-check.sh -p xai-grok-shell\n";
    p += "2) ./grok-check.sh -p xai-grok-inference\n";
    p += "3) ./grok-check.sh -p xai-grok-pager if pager files changed\n";
    p += "4) ./grok-test.sh -p xai-grok-shell -- provider_catalog\n";
    p += "5) ./grok-test.sh -p xai-grok-inference -- openrouter\n";
    p += "If a filter matches nothing, run the nearest existing test names you find with grep. Do not start a second helper while one is compiling.\n";
    p += "Fix only compile/test failures caused by these slices. The two auth_error_no_retry_tests failures listed in the rules are pre-existing; do not fix or block on them.\n";
    p += "Reply with commands, exit codes, and whether the harness slices are complete or which gaps remain, in en-US. Do not commit.\n";
    p
}

// Replacement model: the inherited session route (rezulto/glm5.3-flash) stalled
// mid-run. Explicit model per agent so no slice inherits the dead route.
let model = "zdr:~z-ai/glm-latest";

phase("Accounts");
log("Accounts: extra OpenRouter keys, openrouter::<id>::api_key, cache isolation.");
let accounts = agent(accounts_prompt(), #{
    label: "implement-multi-account-openrouter-keys",
    capability_mode: "all",
    model: model,
});
log(line("implement-multi-account-openrouter-keys", accounts));

phase("Sampler");
log("Sampler: identity-gated payloads, clamp output to ceiling, optional generation diagnostics.");
let sampler = agent(sampler_prompt(), #{
    label: "implement-openrouter-sampler-payloads",
    capability_mode: "all",
    model: model,
});
log(line("implement-openrouter-sampler-payloads", sampler));

phase("Verify");
log("Verify: focused grok-check/test under ~/.grokdev. Failures are reported, not paused.");
let verify = agent(verify_prompt(), #{
    label: "verify-openrouter-native-harness",
    capability_mode: "all",
    model: model,
});
log(line("verify-openrouter-native-harness", verify));

let summary = "";
summary += "openrouter-native-harness-resume\n";
summary += line("implement-multi-account-openrouter-keys", accounts);
summary += "\n";
summary += line("implement-openrouter-sampler-payloads", sampler);
summary += "\n";
summary += line("verify-openrouter-native-harness", verify);
summary += "\n";
summary += "Catalog and Picker landed in the original run. Replacement model: zdr:~z-ai/glm-latest.\n";

let report_path = write_scratch_file("openrouter-native-harness-resume.md", summary);

complete(#{
    summary: summary,
    path: report_path,
    accounts_ok: ok(accounts),
    sampler_ok: ok(sampler),
    verify_ok: ok(verify),
});