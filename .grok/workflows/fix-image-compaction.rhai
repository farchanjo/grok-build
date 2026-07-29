let meta = #{
    name: "fix-image-compaction",
    description: "Implement and adversarially verify portable image-safe conversation compaction",
    phases: [
        #{ title: "Design", detail: "Luna derives the smallest safe cross-crate change" },
        #{ title: "Implement", detail: "Terra implements the fix and focused tests" },
        #{ title: "Review", detail: "Luna and Grok independently review correctness and regressions" },
        #{ title: "Repair", detail: "Terra addresses verified findings" },
        #{ title: "Verify", detail: "Grok runs final focused validation" },
    ],
};

let plan_schema = #{
    "type": "object", "required": ["approach", "files", "tests", "risks"],
    "properties": #{
        "approach": #{ "type": "string" },
        "files": #{ "type": "array", "maxItems": 12, "items": #{ "type": "string" } },
        "tests": #{ "type": "array", "maxItems": 12, "items": #{ "type": "string" } },
        "risks": #{ "type": "array", "maxItems": 12, "items": #{ "type": "string" } },
    },
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

let objective = if args == () { () } else { args.objective };
if objective == () { pause("verification", "Pass args.objective for the image-compaction fix."); }

phase("Design");
let design_prompt = "Read AGENTS.md and GROK.md in full before investigating. Use read_file and grep to inspect the actual repository. Design the smallest maintainable fix for this objective: " + json_encode(objective) + ". The confirmed defect is that default full-replace and two-pass verbatim compaction preserve raw User and ToolResult images, compaction samples through direct InferenceClient streams rather than the normal ChatState request builder and RetryWithImageStrip path, rolling/lossy behavior differs, and compaction model vision metadata is not a sufficient portability guarantee. Requirements: make compaction input provider-agnostic and text-only while retaining existing text descriptions, image paths, tool text, and tool-pair invariants; sanitize both User ContentPart::Image and ToolResult.images consistently for full-replace, fitted, rolling, two-pass, and segment persistence; add a bounded one-time recovery for payload-too-large or recognized image-processing rejection without broadening unrelated retry/fallback behavior; add focused unit/integration tests for user images, tool-result images, manual/full-replace preparation, rolling preparation, segment preparation, and error classification. Preserve all unrelated dirty files. Do not edit. Return concrete files, tests, and risks.";
let design = agent(design_prompt, #{
    label: "luna-design",
    phase: "Design",
    capability_mode: "read-only",
    agent_type: "plan",
    model: "chatgpt-gpt-5.6-luna",
    output_schema: plan_schema,
});
if design == () || !design.success { pause("infra", "Luna design phase failed."); }

phase("Implement");
let implement_prompt = "Implement the image-compaction fix in the current shared workspace. First read AGENTS.md and GROK.md in full, inspect current git status, and preserve every unrelated pre-existing modification. Never use git reset, checkout, clean, or destructive commands. Do not edit the generated root Cargo.toml or Cargo.lock. Objective: " + json_encode(objective) + ". Luna's plan is: " + json_encode(design.output) + ". Treat that plan as advice and verify it against the current code. Required behavior: compaction is provider-agnostic text-only; raw image payloads are removed from both User content and ToolResult.images while textual descriptions, saved paths, tool-result text, and tool-call/result pairing survive; this must cover full-replace verbatim and fitted paths, rolling, two-pass, and segment persistence through shared preparation helpers where practical. Add a bounded one-time image sanitization retry/backstop for 413/payload-too-large and recognized image-processing errors only, without retry loops, policy-denial fallback, or changing general deterministic/transient semantics. Add regression tests that would fail before the fix, including tool-result images. Keep changes focused and use en-US. Run only the narrowest applicable helpers, one at a time: ./grok-check.sh and ./grok-test.sh, which enforce the isolated ~/.grokdev environment. Do not run installed grok, deployment, make, or network model calls. Return a precise summary, files, validation commands/results, and any remaining issue.";
let implementation = agent(implement_prompt, #{
    label: "terra-implement",
    phase: "Implement",
    capability_mode: "all",
    agent_type: "general-purpose",
    model: "chatgpt-gpt-5.6-terra",
    output_schema: implementation_schema,
});
if implementation == () || !implementation.success { pause("infra", "Terra implementation phase failed."); }

phase("Review");
let review_base = "Read AGENTS.md and GROK.md, then independently inspect the current working tree and actual diff for the image-compaction implementation. Do not edit files and do not rely only on the implementer's report. Preserve awareness that unrelated dirty files predated this task. Objective: " + json_encode(objective) + ". Implementation report: " + json_encode(implementation.output) + ". Verify: both User and ToolResult raw images are sanitized while useful text/path context remains; full-replace, fitted, rolling, two-pass, and segments have consistent behavior; the one-time image-error recovery is bounded and cannot loop or accidentally retry policy/auth/schema errors; API backends and tool-pair invariants remain valid; tests exercise pre-fix failures and not just helper internals. Report only concrete findings with file evidence. An empty findings list is valid only after reading every changed relevant file.";
let reviews = parallel([
    #{
        prompt: review_base + " Focus on architecture, data flow, provider portability, and missing paths.",
        label: "luna-review",
        phase: "Review",
        capability_mode: "read-only",
        agent_type: "explore",
        model: "chatgpt-gpt-5.6-luna",
        output_schema: review_schema,
    },
    #{
        prompt: review_base + " Be adversarial about Rust correctness, retry classification, regressions, test quality, and accidental unrelated changes.",
        label: "grok-review",
        phase: "Review",
        capability_mode: "read-only",
        agent_type: "explore",
        model: "grok-4.5",
        output_schema: review_schema,
    },
]);
let usable_reviews = [];
for r in reviews {
    if r != () && r.success { usable_reviews.push(r.output); }
}
if usable_reviews.len() == 0 { pause("verification", "Both independent review agents failed; refusing an unreviewed implementation."); }

phase("Repair");
let repair_prompt = "Read AGENTS.md and GROK.md, inspect the current diff and the implementation itself, then address every valid high/medium correctness finding from these independent reviews: " + json_encode(usable_reviews) + ". Preserve unrelated pre-existing changes and do not use destructive git operations. Do not blindly obey a review: verify each claim in the code, fix confirmed issues, add or strengthen focused regression tests, and leave false positives unchanged with an explanation. Maintain provider-agnostic text-only compaction, complete User plus ToolResult image sanitization, and bounded one-time image-error recovery. Use en-US. Run the narrowest relevant ./grok-check.sh and ./grok-test.sh commands one at a time under the repository helper environment. Never use installed grok, deployment, make, or external inference. Return files changed, exact validations, and remaining concerns.";
let repair = agent(repair_prompt, #{
    label: "terra-repair",
    phase: "Repair",
    capability_mode: "all",
    agent_type: "general-purpose",
    model: "chatgpt-gpt-5.6-terra",
    output_schema: implementation_schema,
});
if repair == () || !repair.success { pause("infra", "Terra repair phase failed."); }

phase("Verify");
let verify_prompt = "Perform final verification of the current shared workspace for this objective: " + json_encode(objective) + ". Read AGENTS.md and GROK.md first. Do not edit files. Inspect the actual diff and relevant unchanged call sites. Ensure unrelated pre-existing dirty files were not overwritten. Verify provider-agnostic text-only compaction sanitizes User and ToolResult images across full-replace, fitted, rolling, two-pass, and segments; useful descriptions/paths and tool invariants remain; image-error recovery is exactly bounded and narrowly classified; tests are meaningful. Run the narrowest relevant ./grok-check.sh and ./grok-test.sh commands one at a time in the repository's isolated helper environment. Do not use direct cargo unless a helper cannot express the command, and never use installed grok, deployment, make, or external inference. Return passed=false on any code, test, formatting, or scope failure and list exact failures.";
let verification = agent(verify_prompt, #{
    label: "grok-verify",
    phase: "Verify",
    capability_mode: "all",
    agent_type: "general-purpose",
    model: "grok-4.5",
    output_schema: verify_schema,
});
if verification == () || !verification.success { pause("verification", "Final Grok verification agent failed."); }

complete(#{
    objective: objective,
    design: design.output,
    implementation: implementation.output,
    reviews: usable_reviews,
    repair: repair.output,
    verification: verification.output,
});
