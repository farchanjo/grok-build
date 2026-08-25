let meta = #{
    name: "dual-language-streaming",
    description: "Implement dual language policy with streaming structured output using xAI Grok 4.6",
    phases: [
        #{ title: "Architecture", detail: "Confirm file-level invariants and rollout order" },
        #{ title: "Implement", detail: "Single writer implements TUI, policy, streaming, and validation seams" },
        #{ title: "Review", detail: "Independent streaming and language-policy reviews" },
        #{ title: "Repair", detail: "Fix confirmed findings" },
        #{ title: "Verify", detail: "Focused check and tests under ~/.grokdev" },
    ],
};

let architecture_schema = #{
    "type": "object",
    "required": ["summary", "must_change", "must_not_change", "risks"],
    "properties": #{
        "summary": #{ "type": "string", "minLength": 1 },
        "must_change": #{
            "type": "array",
            "minItems": 1,
            "items": #{
                "type": "object",
                "required": ["path", "why"],
                "properties": #{
                    "path": #{ "type": "string" },
                    "why": #{ "type": "string" },
                },
            },
        },
        "must_not_change": #{
            "type": "array",
            "items": #{ "type": "string" },
        },
        "risks": #{
            "type": "array",
            "items": #{ "type": "string" },
        },
    },
};

let implement_schema = #{
    "type": "object",
    "required": ["summary", "files_changed", "tests_added", "remaining_gaps"],
    "properties": #{
        "summary": #{ "type": "string", "minLength": 1 },
        "files_changed": #{
            "type": "array",
            "items": #{ "type": "string" },
        },
        "tests_added": #{
            "type": "array",
            "items": #{ "type": "string" },
        },
        "remaining_gaps": #{
            "type": "array",
            "items": #{ "type": "string" },
        },
    },
};

let review_schema = #{
    "type": "object",
    "required": ["findings", "summary"],
    "properties": #{
        "summary": #{ "type": "string" },
        "findings": #{
            "type": "array",
            "maxItems": 12,
            "items": #{
                "type": "object",
                "required": ["severity", "file", "issue", "evidence"],
                "properties": #{
                    "severity": #{ "type": "string", "enum": ["high", "medium", "low"] },
                    "file": #{ "type": "string" },
                    "issue": #{ "type": "string" },
                    "evidence": #{ "type": "string" },
                },
            },
        },
    },
};

let verify_schema = #{
    "type": "object",
    "required": ["passed", "commands", "summary"],
    "properties": #{
        "passed": #{ "type": "boolean" },
        "summary": #{ "type": "string" },
        "commands": #{
            "type": "array",
            "items": #{
                "type": "object",
                "required": ["command", "exit_code", "result"],
                "properties": #{
                    "command": #{ "type": "string" },
                    "exit_code": #{ "type": "integer" },
                    "result": #{ "type": "string" },
                },
            },
        },
    },
};

fn require_success(result, label) {
    if result == () || !result.success {
        pause("verification", "Agent failed: " + label + ". Resume after diagnosing the failure.");
    }
    result
}

phase("Architecture");
log("Phase 1/5 Architecture: read-only Grok 4.6 agent will lock the implementation contract.");

let architecture_prompt = "";
architecture_prompt += "You are a read-only software architect for Grok Build.\n";
architecture_prompt += "Use United States English (en-US) for all authored content.\n";
architecture_prompt += "Read AGENTS.md and GROK.md before proposing changes.\n";
architecture_prompt += "Goal: dual language policy with streaming structured output.\n";
architecture_prompt += "Requirements:\n";
architecture_prompt += "1) Conversation language is independent from code/artifact language.\n";
architecture_prompt += "2) Project policy can lock artifact language to en-US while conversation is pt-BR.\n";
architecture_prompt += "3) Structured Output must preserve streaming in the TUI.\n";
architecture_prompt += "4) Provisional streaming attempts must support commit/reset semantics.\n";
architecture_prompt += "5) JSON envelope must never appear in scrollback; only decoded response text.\n";
architecture_prompt += "6) Use xAI Grok 4.6 assumptions; do not depend on OpenRouter-only behavior.\n";
architecture_prompt += "Inspect these seams with read_file/grep before answering:\n";
architecture_prompt += "- crates/codegen/xai-grok-pager/src/settings\n";
architecture_prompt += "- crates/codegen/xai-grok-shell/src/session/acp_session_impl/turn.rs\n";
architecture_prompt += "- crates/codegen/xai-grok-shell/src/session/acp_session_impl/tool_calls.rs\n";
architecture_prompt += "- crates/codegen/xai-grok-inference/src/stream\n";
architecture_prompt += "- crates/codegen/xai-grok-inference-types/src/conversation.rs\n";
architecture_prompt += "Return only the required schema. Prefer the smallest file set that satisfies the requirements.\n";
architecture_prompt += "Do not edit files.\n";

let architecture = agent(architecture_prompt, #{
    label: "architecture",
    capability_mode: "read-only",
    output_schema: architecture_schema,
});
architecture = require_success(architecture, "architecture");
log("Architecture complete. Moving to single-writer implementation.");

phase("Implement");
log("Phase 2/5 Implement: one Grok 4.6 writer only. No parallel writers. Avoid Cargo lock contention.");

let implement_prompt = "";
implement_prompt += "You are a fullstack implementer for Grok Build.\n";
implement_prompt += "Use United States English (en-US) for all authored content, comments, tests, and user-facing strings.\n";
implement_prompt += "Read AGENTS.md and GROK.md before editing.\n";
implement_prompt += "Implement the dual-language + streaming structured-output design.\n";
implement_prompt += "Architecture contract JSON follows:\n";
implement_prompt += json_encode(architecture.output);
implement_prompt += "\n";
implement_prompt += "Required product behavior:\n";
implement_prompt += "- Settings: Models -> Model language group with Conversation language, Code & artifacts language, Structured response mode.\n";
implement_prompt += "- Conversation language and artifact language are independent.\n";
implement_prompt += "- Project/managed policy can lock artifact language to en-US.\n";
implement_prompt += "- Structured Output streams by projecting only the response string field to the TUI.\n";
implement_prompt += "- Add provisional attempt/commit/reset semantics so failed validation can reset streamed text.\n";
implement_prompt += "- Persist committed decoded response text only; keep validated envelope as metadata.\n";
implement_prompt += "- Prefer native schema streaming; synthetic StructuredOutput tool path may buffer only if it cannot stream safely, but document the gap.\n";
implement_prompt += "Development rules:\n";
implement_prompt += "- GROK_HOME must be ${HOME}/.grokdev for all commands.\n";
implement_prompt += "- Prefer ./grok-check.sh and ./grok-test.sh; run ONE helper at a time.\n";
implement_prompt += "- unset GROK_HOME GROK_LEADER_SOCKET GROK_CLAUDE_CLI_RUNTIME before helpers if needed.\n";
implement_prompt += "- Do not run make deploy or touch ~/.grok / ~/.grok-prod.\n";
implement_prompt += "- Preserve unrelated working-tree changes.\n";
implement_prompt += "- Keep changes focused and add tests proportional to risk.\n";
implement_prompt += "When finished, return the required schema. Do not claim tests passed unless you ran them.\n";

let implementation = agent(implement_prompt, #{
    label: "implement",
    capability_mode: "all",
    output_schema: implement_schema,
});
implementation = require_success(implementation, "implement");
log("Implementation draft complete. Starting independent reviews.");

phase("Review");
log("Phase 3/5 Review: two read-only Grok 4.6 reviewers in parallel.");

let streaming_review_prompt = "";
streaming_review_prompt += "You are an adversarial streaming/protocol reviewer.\n";
streaming_review_prompt += "Use United States English (en-US).\n";
streaming_review_prompt += "Read AGENTS.md and GROK.md. Inspect the actual working tree with read-only tools.\n";
streaming_review_prompt += "Implementation summary JSON:\n";
streaming_review_prompt += json_encode(implementation.output);
streaming_review_prompt += "\n";
streaming_review_prompt += "Verify whether streaming structured output truly preserves live TUI text without leaking the JSON envelope.\n";
streaming_review_prompt += "Check provisional attempt/commit/reset, retry-after-projection, native vs synthetic fallback, and persistence/replay.\n";
streaming_review_prompt += "Report only concrete findings with file paths and evidence. Empty findings are allowed only after reading the code.\n";
streaming_review_prompt += "Do not edit files.\n";

let language_review_prompt = "";
language_review_prompt += "You are an adversarial language-policy and TUI settings reviewer.\n";
language_review_prompt += "Use United States English (en-US).\n";
language_review_prompt += "Read AGENTS.md and GROK.md. Inspect the actual working tree with read-only tools.\n";
language_review_prompt += "Implementation summary JSON:\n";
language_review_prompt += json_encode(implementation.output);
language_review_prompt += "\n";
language_review_prompt += "Verify dual independent language settings, project lock for artifact language, tool-boundary enforcement, and that conversation can be pt-BR while artifacts stay en-US.\n";
language_review_prompt += "Confirm user-facing settings text and tests are en-US.\n";
language_review_prompt += "Report only concrete findings with file paths and evidence. Empty findings are allowed only after reading the code.\n";
language_review_prompt += "Do not edit files.\n";

let reviews = parallel([
    #{
        prompt: streaming_review_prompt,
        label: "review-streaming",
        capability_mode: "read-only",
            output_schema: review_schema,
    },
    #{
        prompt: language_review_prompt,
        label: "review-language",
        capability_mode: "read-only",
            output_schema: review_schema,
    },
]);

let confirmed = [];
let review_labels = ["review-streaming", "review-language"];
let ri = 0;
for r in reviews {
    if r == () || !r.success || r.output.findings == () {
        log("Review panel missing or failed: " + review_labels[ri] + ". Treating as unverified.");
    } else {
        for f in r.output.findings {
            if f.evidence != () && f.evidence != "" {
                confirmed.push(f);
            }
        }
    }
    ri += 1;
}
log("Reviews produced " + confirmed.len().to_string() + " evidenced findings.");

phase("Repair");
log("Phase 4/5 Repair: one Grok 4.6 writer addresses confirmed findings.");

let repair_prompt = "";
repair_prompt += "You are a repair engineer for Grok Build.\n";
repair_prompt += "Use United States English (en-US) for all authored content.\n";
repair_prompt += "Read AGENTS.md and GROK.md before editing.\n";
repair_prompt += "Implementation summary JSON:\n";
repair_prompt += json_encode(implementation.output);
repair_prompt += "\nConfirmed review findings JSON:\n";
repair_prompt += json_encode(confirmed);
repair_prompt += "\n";
repair_prompt += "Fix the confirmed findings. Preserve unrelated working-tree changes.\n";
repair_prompt += "Development rules:\n";
repair_prompt += "- GROK_HOME=${HOME}/.grokdev only.\n";
repair_prompt += "- Prefer ./grok-check.sh and ./grok-test.sh; run ONE helper at a time.\n";
repair_prompt += "- Do not run make deploy.\n";
repair_prompt += "If findings is empty, verify the current implementation is coherent and add any missing focused tests.\n";
repair_prompt += "Return the required schema.\n";

let repair = agent(repair_prompt, #{
    label: "repair",
    capability_mode: "all",
    output_schema: implement_schema,
});
repair = require_success(repair, "repair");
log("Repair complete. Starting focused verification.");

phase("Verify");
log("Phase 5/5 Verify: sequential focused checks under ~/.grokdev. One Cargo helper at a time.");

let verify_prompt = "";
verify_prompt += "You are a verification engineer for Grok Build.\n";
verify_prompt += "Use United States English (en-US).\n";
verify_prompt += "Read AGENTS.md and GROK.md. Do not edit production code unless a tiny test-only fix is required for a broken assertion you introduced; prefer no edits.\n";
verify_prompt += "Final implementation summary JSON:\n";
verify_prompt += json_encode(repair.output);
verify_prompt += "\n";
verify_prompt += "Run these commands ONE AT A TIME in the repo root. Never run two Cargo helpers concurrently.\n";
verify_prompt += "Always unset production env first:\n";
verify_prompt += "unset GROK_HOME GROK_LEADER_SOCKET GROK_CLAUDE_CLI_RUNTIME\n";
verify_prompt += "Then run:\n";
verify_prompt += "1) ./grok-check.sh -p xai-grok-inference-types\n";
verify_prompt += "2) ./grok-check.sh -p xai-grok-inference\n";
verify_prompt += "3) ./grok-check.sh -p xai-grok-shell\n";
verify_prompt += "4) ./grok-check.sh -p xai-grok-pager\n";
verify_prompt += "5) Focused nextest for changed surfaces using ./grok-test.sh with --status-level pass --failure-output immediate and a narrow -E filter.\n";
verify_prompt += "Do not run make deploy, workspace-wide release builds, or production grok-custom validation.\n";
verify_prompt += "Return passed=true only if every command you report exited 0.\n";

let verification = agent(verify_prompt, #{
    label: "verify",
    capability_mode: "execute",
    output_schema: verify_schema,
});
verification = require_success(verification, "verify");

if verification.output.passed != true {
    pause("verification", "Focused verification did not fully pass. Inspect the report, then resume after fixes.");
}

let report = "";
report += "# Dual Language Streaming Workflow Report\n\n";
report += "Model: xAI Grok 4.6 (`grok-4.6`). Effort guidance: high reasoning for architecture/implementation/review; verification is evidence-driven.\n\n";
report += "## Architecture\n\n";
report += json_encode(architecture.output);
report += "\n\n## Implementation\n\n";
report += json_encode(implementation.output);
report += "\n\n## Review findings\n\n";
report += json_encode(confirmed);
report += "\n\n## Repair\n\n";
report += json_encode(repair.output);
report += "\n\n## Verification\n\n";
report += json_encode(verification.output);
report += "\n";

let report_path = write_scratch_file("dual-language-streaming-report.md", report);
log("Workflow finished. Report written to " + report_path);
complete(#{
    summary: "Dual language streaming implementation workflow completed on grok-4.6",
    path: report_path,
    passed: verification.output.passed,
    findings: confirmed.len(),
});
