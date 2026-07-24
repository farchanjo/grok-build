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
unset OTEL_EXPORTER_OTLP_LOGS_ENDPOINT OTEL_EXPORTER_OTLP_METRICS_ENDPOINT
unset OTEL_EXPORTER_OTLP_HEADERS OTEL_EXPORTER_OTLP_LOGS_HEADERS
unset OTEL_EXPORTER_OTLP_METRICS_HEADERS

# Disable only the Claude/Cursor compatibility hooks. Native Grok hooks remain enabled.
export GROK_CURSOR_HOOKS_ENABLED="${GROK_CURSOR_HOOKS_ENABLED:-false}"
export GROK_CLAUDE_HOOKS_ENABLED="${GROK_CLAUDE_HOOKS_ENABLED:-false}"

# Space large OpenRouter tool-loop requests and keep a conservative pace for
# a few successful calls after a 429. Both defaults remain overridable.
export GROK_OPENROUTER_MIN_REQUEST_INTERVAL_MS="${GROK_OPENROUTER_MIN_REQUEST_INTERVAL_MS:-2000}"
export GROK_OPENROUTER_RATE_LIMIT_RECOVERY_REQUESTS="${GROK_OPENROUTER_RATE_LIMIT_RECOVERY_REQUESTS:-8}"

mkdir -p "${GROK_HOME}"
chmod 700 "${GROK_HOME}"

cd "${SCRIPT_DIR}"
exec cargo run -p xai-grok-pager-bin -- "$@"
