#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

# Keep development state separate from the default ~/.grok production state.
if [[ -z "${GROK_HOME:-}" ]]; then
    if [[ -z "${HOME:-}" ]]; then
        printf 'error: HOME or GROK_HOME must be set\n' >&2
        exit 1
    fi
    GROK_HOME="${HOME}/.grokdev"
fi

export GROK_HOME
export GROK_LEADER_SOCKET="${GROK_LEADER_SOCKET:-${GROK_HOME}/leader.sock}"
export GROK_DISABLE_AUTOUPDATER="${GROK_DISABLE_AUTOUPDATER:-1}"

# Isolate artifacts from concurrent cargo test/check/build on ./target so this
# runner never blocks on "file lock on artifact directory". sccache (configured
# in .cargo/config.toml) still shares the compile cache across target dirs.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${SCRIPT_DIR}/target-dev}"

# Prefer sccache when available even if .cargo/config is overridden.
if [[ -z "${RUSTC_WRAPPER:-}" ]] && command -v sccache >/dev/null 2>&1; then
    export RUSTC_WRAPPER=sccache
fi

# Export content-redacted development usage telemetry to the dedicated Alloy
# receiver. Pin the routing before Grok starts so ambient OTEL settings cannot
# mix this profile with production or leak unrelated collector credentials.
export GROK_EXTERNAL_OTEL=1
export OTEL_METRICS_EXPORTER=otlp
export OTEL_LOGS_EXPORTER=otlp
export OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
export OTEL_EXPORTER_OTLP_ENDPOINT=http://vm.services:14318
export OTEL_LOG_USER_PROMPTS=0
export OTEL_LOG_TOOL_DETAILS=0
# Opt in to the experimental Claude Agent CLI runtime. The matching Cargo
# feature is enabled in the cargo invocation below; default/release-dist builds
# remain feature-off.
export GROK_CLAUDE_CLI_RUNTIME="${GROK_CLAUDE_CLI_RUNTIME:-1}"
unset OTEL_EXPORTER_OTLP_LOGS_ENDPOINT OTEL_EXPORTER_OTLP_METRICS_ENDPOINT
unset OTEL_EXPORTER_OTLP_HEADERS OTEL_EXPORTER_OTLP_LOGS_HEADERS
unset OTEL_EXPORTER_OTLP_METRICS_HEADERS

# Do not discover state owned by other coding agents. This does not disable
# native Grok hooks or the explicitly selected Claude CLI external runtime.
export GROK_CURSOR_SKILLS_ENABLED=0
export GROK_CURSOR_RULES_ENABLED=0
export GROK_CURSOR_AGENTS_ENABLED=0
export GROK_CURSOR_MCPS_ENABLED=0
export GROK_CURSOR_HOOKS_ENABLED=0
export GROK_CURSOR_SESSIONS_ENABLED=0

export GROK_CLAUDE_SKILLS_ENABLED=0
export GROK_CLAUDE_RULES_ENABLED=0
export GROK_CLAUDE_AGENTS_ENABLED=0
export GROK_CLAUDE_MCPS_ENABLED=0
export GROK_CLAUDE_HOOKS_ENABLED=0
export GROK_CLAUDE_SESSIONS_ENABLED=0

export GROK_CODEX_SKILLS_ENABLED=0
export GROK_CODEX_RULES_ENABLED=0
export GROK_CODEX_AGENTS_ENABLED=0
export GROK_CODEX_MCPS_ENABLED=0
export GROK_CODEX_HOOKS_ENABLED=0
export GROK_CODEX_SESSIONS_ENABLED=0

# Space large OpenRouter tool-loop requests and keep a conservative pace for
# a few successful calls after a 429. Both defaults remain overridable.
export GROK_OPENROUTER_MIN_REQUEST_INTERVAL_MS="${GROK_OPENROUTER_MIN_REQUEST_INTERVAL_MS:-2000}"
export GROK_OPENROUTER_RATE_LIMIT_RECOVERY_REQUESTS="${GROK_OPENROUTER_RATE_LIMIT_RECOVERY_REQUESTS:-8}"

mkdir -p "${GROK_HOME}"
chmod 700 "${GROK_HOME}"
mkdir -p "${CARGO_TARGET_DIR}"

cd "${SCRIPT_DIR}"
exec cargo run -p xai-grok-pager-bin --features claude-cli-runtime -- "$@"
