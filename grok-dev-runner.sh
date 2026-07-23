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
