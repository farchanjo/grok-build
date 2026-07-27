#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

# This script mirrors grok-dev-runner.sh but is focused on running tests with the
# repository's preferred cargo-nextest settings. It always uses the isolated
# ~/.grokdev profile and disables discovery of external-agent state.
#
# Examples:
#   ./grok-test.sh -p xai-grok-shell
#   ./grok-test.sh -p xai-grok-shell --run-ignored all
#   ./grok-test.sh -p xai-grok-shell -- session_runtime_family::test_fork_session
#   ./grok-test.sh -p xai-grok-shell --no-run

if [[ -z "${HOME:-}" ]]; then
    printf 'error: HOME must be set\n' >&2
    exit 1
fi

# Fail closed: refuse to run against any profile other than the canonical
# isolated development profile.
if [[ -n "${GROK_HOME:-}" && "${GROK_HOME}" != "${HOME}/.grokdev" ]]; then
    printf 'error: GROK_HOME must be unset or exactly %s/.grokdev; refusing to use %s\n' \
        "${HOME}" "${GROK_HOME}" >&2
    exit 1
fi

export GROK_HOME="${HOME}/.grokdev"
export GROK_LEADER_SOCKET="${GROK_LEADER_SOCKET:-${GROK_HOME}/leader.sock}"
export GROK_DISABLE_AUTOUPDATER=1

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

mkdir -p "${GROK_HOME}"
chmod 700 "${GROK_HOME}"

# Keep artifacts separate from concurrent development builds if the runner is
# used from a different shell, while still sharing the local sccache.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${SCRIPT_DIR}/target-test}"
mkdir -p "${CARGO_TARGET_DIR}"

if [[ -z "${RUSTC_WRAPPER:-}" ]] && command -v sccache >/dev/null 2>&1; then
    export RUSTC_WRAPPER=sccache
fi

cd "${SCRIPT_DIR}"

# Prefer the rustup-managed active toolchain when available so that the
# correct sysroot and linker setup are used. Fallback to the ambient `cargo`
# if rustup is not present.
CARGO_CMD=(cargo)
if command -v rustup >/dev/null 2>&1; then
    ACTIVE_TOOLCHAIN=$(rustup show active-toolchain | awk 'NR==1 { print $1 }')
    if [[ -n "${ACTIVE_TOOLCHAIN:-}" ]]; then
        CARGO_CMD=(rustup run "${ACTIVE_TOOLCHAIN}" cargo)
    fi
fi

exec "${CARGO_CMD[@]}" nextest run "$@"
