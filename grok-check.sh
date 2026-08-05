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

if [[ $# -eq 0 ]]; then
    printf 'tip: pass -p <crate> for a focused check (for example, ./grok-check.sh -p xai-grok-shell)\n' >&2
fi

exec cargo check --locked "$@"
