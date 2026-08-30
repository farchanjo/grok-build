let meta = #{
    name: "openrouter-native-harness",
    description: "Complete OpenRouter-native harness: ZDR-filtered /model, token ceilings, modalities, multi-account keys, identity-gated payloads. Sequential file-owned slices; inherit the session model; never pause.",
    when_to_use: "When OpenRouter must become a first-class native provider: ZDR picker filter, catalog budgets, extra keys, and full request payloads without touching xAI or ChatGPT.",
    phases: [
        #{ title: "Catalog", detail: "Native catalog: ZDR fetch, token ceilings, tools, efforts, modalities" },
        #{ title: "Picker", detail: "ModelInfo, ACP meta, /model ZDR filter, account labels" },
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
    p += "Omit any other model; inherit this session. Do not add mold/linker flags. Do not kill Cargo locks.\n";
    p
}

fn catalog_prompt() {
    let p = dev_rules();
    p += "TASK: implement-catalog-zdr-budgets\n";
    p += "You own the OpenRouter native catalog path only.\n";
    p += "Primary files:\n";
    p += "- crates/codegen/xai-grok-shell/src/agent/provider_catalog/types.rs\n";
    p += "- crates/codegen/xai-grok-shell/src/agent/provider_catalog/openrouter_adapter.rs\n";
    p += "- crates/codegen/xai-grok-shell/src/agent/provider_catalog/project.rs\n";
    p += "- crates/codegen/xai-grok-shell/src/agent/provider_catalog/publish.rs\n";
    p += "- crates/codegen/xai-grok-shell/src/agent/provider_catalog/refresh.rs\n";
    p += "- crates/codegen/xai-grok-shell/src/agent/providers.rs parse_openrouter_catalog / OpenRouterModel structs (keep bit-for-bit semantics with the adapter projector)\n";
    p += "Implement:\n";
    p += "1) Parse OpenRouter GET /models fields: context_length, top_provider.context_length, top_provider.max_completion_tokens, top_provider.is_moderated, per_request_limits.prompt_tokens/completion_tokens, architecture.input_modalities (text/image/file/audio/video), architecture.output_modalities, supported_parameters (tools, tool_choice, function_calling, reasoning, reasoning_effort, include_reasoning, structured_outputs, response_format, parallel_tool_calls), reasoning.supported_efforts/default_effort/mandatory/supports_max_tokens.\n";
    p += "2) context_window = min of positive advertised and routed context_length. Do not invent a window when both are absent. Never fall back to the 200k ModelInfo::fallback for OpenRouter rows that advertised a window.\n";
    p += "3) max_output_ceiling = min of positive top_provider.max_completion_tokens and per_request_limits.completion_tokens. Keep ModelInfo.max_completion_tokens unset for OpenRouter discovery (existing publish.rs test openrouter_discovery_does_not_set_request_max_completion_tokens must keep passing). Put the ceiling on DiscoveredModel / ProjectedCapabilities / ProviderModelPreset as a new field.\n";
    p += "4) Project supports_tools, reasoning efforts/selection, supports_image_input/audio/video, supports_file_input, output_has_text, supports_native_schema from structured_outputs/response_format. Never guess embeddings or rerank.\n";
    p += "5) ZDR catalog: when the instance provider_preferences.zdr is true, fetch GET /models?zdr=true for THAT instance only. Optionally intersect GET /endpoints/zdr. Stamp supports_zdr on rows. When zdr is false/unset, fetch the full list and still tag supports_zdr when the ZDR endpoint list includes the slug.\n";
    p += "6) Extra kind=openrouter accounts must use this native adapter, not the identity-only OpenAI adapter. Do not write extra-account catalogs into openrouter_models_cache.json.\n";
    p += "7) Add unit tests for conservative window, ceiling vs request-max split, modality projection, and ZDR query gating. Use existing fixtures under provider_catalog tests.\n";
    p += "Inspect live OpenRouter types in crates/codegen/xai-grok-inference/src/openai_platform/generated/openrouter_types.rs (OrModel, OrTopProviderInfo, OrPerRequestLimits, OrInputModality, GetModelsParams.zdr).\n";
    p += "After edits run: ./grok-check.sh -p xai-grok-shell then ./grok-test.sh -p xai-grok-shell -- provider_catalog\n";
    p += "If a test filter is wrong, run the closest existing catalog/openrouter tests. Fix failures you caused.\n";
    p += "Reply with a short summary of files changed, tests run, and remaining gaps. Do not commit.\n";
    p
}

fn picker_prompt() {
    let p = dev_rules();
    p += "TASK: implement-picker-zdr-modalities\n";
    p += "You own /model, ModelInfo, and ACP picker metadata. Catalog fields may already exist from the previous slice; inspect current types before duplicating.\n";
    p += "Primary files:\n";
    p += "- crates/codegen/xai-grok-shell/src/agent/config.rs (ModelInfo, ACP meta, inference_config_for_model)\n";
    p += "- crates/codegen/xai-grok-shell/src/agent/models.rs (selectable catalog, task tools gate)\n";
    p += "- crates/codegen/xai-grok-pager/src/slash/commands/model.rs\n";
    p += "- crates/codegen/xai-grok-pager/src/acp/model_state.rs if display qualification lives there\n";
    p += "- crates/codegen/xai-grok-shell/src/agent/provider_catalog/publish.rs discovered_to_model_entry (copy new catalog fields onto ModelInfo; do not set request max_completion_tokens from OpenRouter ceiling)\n";
    p += "Implement:\n";
    p += "1) Copy OpenRouter catalog capabilities onto ModelInfo: supports_tools, reasoning_effort_selection + efforts, image/audio/video/file input, output_has_text, supports_native_schema, supports_zdr, max_output_ceiling (NEW field, not max_completion_tokens), context_window.\n";
    p += "2) ACP /model meta: keep inputModalities/supportsImageInput. Add supportsZdr, maxOutputTokens (ceiling), supportsFileInput, outputHasText, providerInstanceId already exists. Filter haystack includes canonical id + account label.\n";
    p += "3) When an OpenRouter instance has provider.zdr true, /model and selectable catalog hide that instance's non-ZDR rows. Other instances and all xAI/ChatGPT/OpenAI/Anthropic rows stay visible. Missing ZDR slug on resume is missing, never remapped to a sibling or Grok.\n";
    p += "4) When two or more OpenRouter instances are enabled, picker display always qualifies with display_name or instance id (example: GLM 5.3 Flash · Work · ZDR). Canonical insert/persist id stays openrouter:<slug> or <instance>:<slug>. Ambiguous short names fail closed with both canonical ids.\n";
    p += "5) spawn_subagent / task gate: OpenRouter still requires supports_tools == Some(true). Prefer also requiring output_has_text when that field is Some(false). Do not tighten xAI/OpenAI unknown-tools eligibility.\n";
    p += "6) Tests: ZDR filter is instance-local; two accounts same slug stay distinct; display qualification; ACP meta flags. Pager tests if you change slash command matching.\n";
    p += "After edits run: ./grok-check.sh -p xai-grok-shell then ./grok-check.sh -p xai-grok-pager if you touched pager, then focused grok-test.sh for model identity / slash model / provider_catalog publish.\n";
    p += "Reply with files changed, tests run, remaining gaps. Do not commit.\n";
    p
}

fn accounts_prompt() {
    let p = dev_rules();
    p += "TASK: implement-multi-account-openrouter-keys\n";
    p += "You own extra OpenRouter accounts and credential isolation.\n";
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
    p += "Reply with files changed, tests run, remaining gaps. Do not commit.\n";
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
    p += "Reply with files changed, tests run, remaining gaps. Do not commit.\n";
    p
}

fn verify_prompt() {
    let p = dev_rules();
    p += "TASK: verify-openrouter-native-harness\n";
    p += "You do not add features. You verify the working tree implements the OpenRouter native harness slices.\n";
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
    p += "Fix only compile/test failures caused by these slices. Do not expand scope.\n";
    p += "Reply with commands, exit codes, and whether the harness slices are complete or which gaps remain. Do not commit.\n";
    p
}

// No pause / await_user. Missing survey is not a blocker: each prompt is self-contained.
// Sequential writers with disjoint primary files so Cargo and git do not thrash.
// Inherit the session model: never set opts.model.

phase("Catalog");
log("Catalog: native OpenRouter list, ZDR fetch, token ceilings, tools, efforts, modalities.");
let catalog = agent(catalog_prompt(), #{
    label: "implement-catalog-zdr-budgets",
    capability_mode: "all",
});
log(line("implement-catalog-zdr-budgets", catalog));

phase("Picker");
log("Picker: ModelInfo/ACP meta, instance-local ZDR /model filter, account labels.");
let picker = agent(picker_prompt(), #{
    label: "implement-picker-zdr-modalities",
    capability_mode: "all",
});
log(line("implement-picker-zdr-modalities", picker));

phase("Accounts");
log("Accounts: extra OpenRouter keys, openrouter::<id>::api_key, cache isolation.");
let accounts = agent(accounts_prompt(), #{
    label: "implement-multi-account-openrouter-keys",
    capability_mode: "all",
});
log(line("implement-multi-account-openrouter-keys", accounts));

phase("Sampler");
log("Sampler: identity-gated payloads, clamp output to ceiling, optional generation diagnostics.");
let sampler = agent(sampler_prompt(), #{
    label: "implement-openrouter-sampler-payloads",
    capability_mode: "all",
});
log(line("implement-openrouter-sampler-payloads", sampler));

phase("Verify");
log("Verify: focused grok-check/test under ~/.grokdev. Failures are reported, not paused.");
let verify = agent(verify_prompt(), #{
    label: "verify-openrouter-native-harness",
    capability_mode: "all",
});
log(line("verify-openrouter-native-harness", verify));

let summary = "";
summary += "openrouter-native-harness\n";
summary += line("implement-catalog-zdr-budgets", catalog);
summary += "\n";
summary += line("implement-picker-zdr-modalities", picker);
summary += "\n";
summary += line("implement-multi-account-openrouter-keys", accounts);
summary += "\n";
summary += line("implement-openrouter-sampler-payloads", sampler);
summary += "\n";
summary += line("verify-openrouter-native-harness", verify);
summary += "\n";
summary += "Watch /workflows. Session model inherited on every agent. No pause gates.\n";

let report_path = write_scratch_file("openrouter-native-harness.md", summary);

complete(#{
    summary: summary,
    path: report_path,
    catalog_ok: ok(catalog),
    picker_ok: ok(picker),
    accounts_ok: ok(accounts),
    sampler_ok: ok(sampler),
    verify_ok: ok(verify),
});
