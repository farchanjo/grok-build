#!/usr/bin/env bash
set -euo pipefail

# Portable symlink resolution: follow symlinks without requiring GNU realpath or
# readlink -f, which are unavailable on a default macOS install.
_grok_resolve_symlinks() {
    local target="$1"
    local depth=0

    while [[ -L "${target}" ]]; do
        if (( ++depth > 40 )); then
            printf 'error: too many symlink levels for: %s\n' "$1" >&2
            return 1
        fi

        local link
        link="$(readlink "${target}")" || {
            printf 'error: cannot read symlink: %s\n' "${target}" >&2
            return 1
        }

        if [[ "${link}" == /* ]]; then
            target="${link}"
        else
            local dir
            dir="${target%/*}"
            dir="${dir:-.}"
            target="${dir}/${link}"
        fi
    done

    printf '%s\n' "${target}"
}

SCRIPT_SRC="${BASH_SOURCE[0]}"
if [[ -L "${SCRIPT_SRC}" ]]; then
    SCRIPT_SRC="$(_grok_resolve_symlinks "${SCRIPT_SRC}")" || exit 1
fi
SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "${SCRIPT_SRC}")" && pwd -P)"

source "${SCRIPT_DIR}/grok-dev-env.sh"

cd "${SCRIPT_DIR}"

# Run the interactive console with the experimental claude-cli-runtime feature.
# This wrapper shares the same isolated ~/.grokdev profile and target-dev
# directory as ./grok-check.sh and ./grok-test.sh, so dependency artifacts are
# reused across the normal development loop. Targets that depend on
# claude-cli-runtime are rebuilt because of the feature flag, but the shared
# directory avoids duplicating everything. Use it only when you need
# claude-cli-runtime; otherwise prefer direct `cargo run` with the same
# environment.
#
# Examples:
#   ./grok-dev-runner.sh --no-leader --no-auto-update
#   ./grok-dev-runner.sh --no-leader --no-auto-update --no-alt-screen

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

# Opt in to the experimental Claude Agent CLI runtime.
export GROK_CLAUDE_CLI_RUNTIME="${GROK_CLAUDE_CLI_RUNTIME:-1}"

unset OTEL_EXPORTER_OTLP_LOGS_ENDPOINT OTEL_EXPORTER_OTLP_METRICS_ENDPOINT
unset OTEL_EXPORTER_OTLP_HEADERS OTEL_EXPORTER_OTLP_LOGS_HEADERS
unset OTEL_EXPORTER_OTLP_METRICS_HEADERS

# Space large OpenRouter tool-loop requests and keep a conservative pace for
# a few successful calls after a 429. Both defaults remain overridable.
export GROK_OPENROUTER_MIN_REQUEST_INTERVAL_MS="${GROK_OPENROUTER_MIN_REQUEST_INTERVAL_MS:-2000}"
export GROK_OPENROUTER_RATE_LIMIT_RECOVERY_REQUESTS="${GROK_OPENROUTER_RATE_LIMIT_RECOVERY_REQUESTS:-8}"

# Cargo serializes access to the shared target-dev directory with its own file
# lock, which is released once the build finishes and the long-running binary
# starts. Do not add a wrapper-level lock; keep the invocation simple.
exec cargo run --locked -p xai-grok-pager-bin --features claude-cli-runtime -- "$@"
