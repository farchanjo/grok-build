#!/usr/bin/env bash
# shellcheck shell=bash
# Reusable isolated development environment for Grok Build repository scripts.
#
# Source this file from repo-root wrappers (for example, grok-check.sh,
# grok-test.sh, grok-dev-runner.sh). It ensures they all share the same
# worktree-local target-dev directory while keeping the mandatory ~/.grokdev
# profile isolated from ~/.grok and other production profiles.
#
# It does not run cargo itself; wrappers continue to invoke cargo directly.

set -euo pipefail

# Pure-bash symlink resolution for paths to this repository. This avoids
# relying on GNU-only `realpath` or `readlink -f`, which are absent from the
# default macOS install.
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
            # Use the directory that held the symlink to interpret relative
            # link targets.
            local dir
            dir="${target%/*}"
            dir="${dir:-.}"
            target="${dir}/${link}"
        fi
    done

    printf '%s\n' "${target}"
}

# Resolve this file's real directory so that callers work even when invoked
# through a symlink, from an arbitrary cwd, or with a symlinked directory
# component in the path. `_GROK_*` variables are internal and intentionally
# not exported to child processes.
_GROK_DEV_ENV_SELF="${BASH_SOURCE[0]}"
if [[ -L "${_GROK_DEV_ENV_SELF}" ]]; then
    _GROK_DEV_ENV_SELF="$(_grok_resolve_symlinks "${_GROK_DEV_ENV_SELF}")" || return 1
fi
_GROK_DEV_ENV_DIR="$(CDPATH= cd -- "$(dirname -- "${_GROK_DEV_ENV_SELF}")" && pwd -P)"

if [[ -z "${HOME:-}" ]]; then
    printf 'error: HOME is not set\n' >&2
    return 1
fi

_GROK_DEV_HOME="${HOME}/.grokdev"
_GROK_DEV_LEADER_SOCKET="${_GROK_DEV_HOME}/leader.sock"

# Fail closed: refuse to run against any profile other than the canonical
# isolated development profile. Also reject a symlinked canonical path so that
# the application cannot be tricked into using a different directory.
if [[ -n "${GROK_HOME:-}" && "${GROK_HOME}" != "${_GROK_DEV_HOME}" ]]; then
    printf 'error: GROK_HOME must be unset or exactly %s (got %s)\n' \
        "${_GROK_DEV_HOME}" "${GROK_HOME}" >&2
    return 1
fi
if [[ -L "${_GROK_DEV_HOME}" ]]; then
    printf 'error: GROK_HOME path is a symlink: %s\n' "${_GROK_DEV_HOME}" >&2
    return 1
fi
export GROK_HOME="${_GROK_DEV_HOME}"

# The repository requires this exact leader socket. Reject any pre-existing
# value that does not match it exactly, and reject a symlinked socket file.
if [[ -n "${GROK_LEADER_SOCKET:-}" ]]; then
    if [[ "${GROK_LEADER_SOCKET}" != "${_GROK_DEV_LEADER_SOCKET}" ]]; then
        printf 'error: GROK_LEADER_SOCKET must equal exactly %s (got %s)\n' \
            "${_GROK_DEV_LEADER_SOCKET}" "${GROK_LEADER_SOCKET}" >&2
        return 1
    fi
    if [[ -L "${GROK_LEADER_SOCKET}" ]]; then
        printf 'error: GROK_LEADER_SOCKET exists as a symlink: %s\n' \
            "${GROK_LEADER_SOCKET}" >&2
        return 1
    fi
else
    export GROK_LEADER_SOCKET="${_GROK_DEV_LEADER_SOCKET}"
fi

# Reject a symlinked socket file even when the caller did not set an explicit
# GROK_LEADER_SOCKET. The canonical path must be a real Unix-domain socket or
# not yet exist; a symlink could redirect traffic to a production leader.
if [[ -L "${GROK_LEADER_SOCKET}" ]]; then
    printf 'error: GROK_LEADER_SOCKET exists as a symlink: %s\n' \
        "${GROK_LEADER_SOCKET}" >&2
    return 1
fi

export GROK_DISABLE_AUTOUPDATER=1

# Prevent the development console from discovering state owned by Cursor, Claude,
# and Codex. Compatibility-specific tests can re-enable only the surfaces they
# explicitly exercise, and must still use controlled fixtures or ~/.grokdev.
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

# Secure the dev profile with owner-only permissions.
umask 077
mkdir -p "${GROK_HOME}"
chmod 0700 "${GROK_HOME}"

# Internal repo root, not exported to child binaries.
_GROK_REPO_ROOT="${_GROK_DEV_ENV_DIR}"

# Default to a worktree-local target directory so check, test, and run share
# artifacts without colliding with release or other worktrees. Honoring an
# explicit caller-provided override lets users run isolated experiments without
# changing repository files.
if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
    export CARGO_TARGET_DIR="${_GROK_REPO_ROOT}/target-dev"
fi
mkdir -p "${CARGO_TARGET_DIR}"

# Prefer sccache caching when it is available, but do not fail when it is not
# installed. If the caller already exported RUSTC_WRAPPER, leave it alone.
if [[ -z "${RUSTC_WRAPPER+x}" ]]; then
    if command -v sccache >/dev/null 2>&1; then
        export RUSTC_WRAPPER=sccache
    else
        # Setting this to the empty string lets Cargo ignore any
        # rustc-wrapper declared in .cargo/config.toml rather than failing
        # because the command is not on PATH.
        export RUSTC_WRAPPER=""
    fi
fi
