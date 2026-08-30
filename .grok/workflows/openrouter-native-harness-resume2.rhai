let meta = #{
    name: "openrouter-native-harness-resume2",
    description: "Final OpenRouter-native harness slices: Sampler (identity-gated payloads, ceiling clamp) and Verify. Accounts already landed. Runs on dr:aion-labs/aion-3.0 after two prior routes stalled or looped.",
    when_to_use: "When only the sampler payload and verification slices of the OpenRouter native harness remain and a proven model route is required.",
    phases: [
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
    p += "Built-in OpenRouter catalog ids stay openrouter:<upstream>. Extra instances use <instance_id>:<upstream>. Reserved ids: xai, grok, openai, chatgpt, codex, openrouter, anthropic, admin, local.\n";
    p += "OpenRouter top_provider.max_completion_tokens is a CAPABILITY CEILING, not a request default. Do not copy it onto ModelInfo.max_completion_tokens / InferenceConfig.max_completion_tokens. Store it as max_output_ceiling and clamp only when a session policy sets a request max.\n";
    p += "xAI team ZDR (GrokAuth::is_zdr_team) is not OpenRouter provider.zdr. Do not hide Grok/ChatGPT models when an OpenRouter instance enables ZDR.\n";
    p += "Known pre-existing failures you must NOT fix or treat as caused by your slice: session::acp_session::auth_error_no_retry_tests::reconstruct_catalog_miss_openrouter_host_no_xai_bearer_resolver and reconstruct_openrouter_vault_model_no_xai_bearer_resolver (stale is_none() assertions from fdc66d4 superseded by 118f27b exact-route binding). Report them as pre-existing and move on.\n";
    p += "Do not add mold/linker flags. Do not kill Cargo locks.\n";
    p
}

fn sampler_prompt() {
    let p = dev_rules();
    p += "TASK: implement-openrouter-sampler-payloads\n";
    p += "You own OpenRouter inference payloads and post-call diagnostics.\n";
    p += "Context: Catalog, Picker, and Accounts slices already landed in this tree. Inspect current code before editing; reuse existing fields (max_output_ceiling exists on ModelInfo and InferenceConfig).\n";
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
    p += "You do not add features. You verify the working tree implements the OpenRouter native harness slices (Catalog, Picker, Accounts landed earlier; Sampler just ran).\n";
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

// Route history: rezulto/glm5.3-flash stalled mid-run; zdr:~z-ai/glm-latest
// looped (16M tokens, zero tool calls); zdr:aion-labs/aion-3.0 404s on ZDR
// policy. dr:aion-labs/aion-3.0 probed OK in 3.0s. Explicit model per agent.
let model = "dr:aion-labs/aion-3.0";

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
summary += "openrouter-native-harness-resume2\n";
summary += line("implement-openrouter-sampler-payloads", sampler);
summary += "\n";
summary += line("verify-openrouter-native-harness", verify);
summary += "\n";
summary += "Catalog, Picker, and Accounts landed in prior runs. Model: dr:aion-labs/aion-3.0.\n";

let report_path = write_scratch_file("openrouter-native-harness-resume2.md", summary);

complete(#{
    summary: summary,
    path: report_path,
    sampler_ok: ok(sampler),
    verify_ok: ok(verify),
});