#!/usr/bin/env bash
set -euo pipefail

# Reclaim disk from build outputs, without giving up the dev incremental cache.
#
# `target-dev` is the shared dev target directory and its `incremental/` tree is
# the point: it is what makes an edit-to-build cycle fast, so a blunt
# `cargo clean` costs more than it frees. This script removes only what is
# genuinely spent:
#
#   1. Superseded incremental sessions — cargo keeps the newest session per
#      crate/configuration and leaves older ones behind; they can never be
#      resumed. Keeping the newest session preserves the cache for every crate.
#   2. Cold trees (`--stale-days N`) — whole incremental dirs and `deps`
#      executables not touched in N days. `deps` *libraries* (`.rlib`) are kept
#      regardless: an active crate links against cold dependencies, so dropping
#      them forces rebuilds the incremental cache cannot absorb.
#   3. The deploy output (`--release-dist`) — `target/release-dist` is rebuilt
#      from scratch by the next `make deploy`, so it is dead weight between
#      releases.
#   4. The stray default target directory (`--target-debug`) — plain `cargo`
#      runs without `grok-dev-env.sh` write to `target/` and duplicate the dev
#      graph there.
#   5. The sccache cache (`--sccache`) — rustc's incremental mode bypasses it,
#      so a dev-only cache measures ~0% hits.
#
# Report by default; nothing is deleted without `--apply`.
#
# Usage:
#   ./grok-clean.sh                     # report sizes and what is prunable
#   ./grok-clean.sh --apply             # prune superseded sessions
#   ./grok-clean.sh --apply --all       # + deploy output, stray target, sccache
#   ./grok-clean.sh --apply --stale-days 7
#
# Examples:
#   ./grok-clean.sh --apply --release-dist     # right after a deploy
#   ./grok-clean.sh --apply --stale-days 3     # when the disk is tight

APPLY=0
STALE_DAYS=0
DROP_RELEASE_DIST=0
DROP_TARGET_DEBUG=0
CLEAR_SCCACHE=0

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
            printf 'error: cannot read symlink: %s\n' "$1" >&2
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

usage() {
    sed -n '3,42p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --apply) APPLY=1 ;;
        --stale-days)
            shift
            [[ $# -gt 0 ]] || { printf 'error: --stale-days needs a value\n' >&2; exit 1; }
            STALE_DAYS="$1"
            ;;
        --release-dist) DROP_RELEASE_DIST=1 ;;
        --target-debug) DROP_TARGET_DEBUG=1 ;;
        --sccache) CLEAR_SCCACHE=1 ;;
        --all)
            DROP_RELEASE_DIST=1
            DROP_TARGET_DEBUG=1
            CLEAR_SCCACHE=1
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            printf 'error: unknown argument: %s\n' "$1" >&2
            usage >&2
            exit 1
            ;;
    esac
    shift
done

if ! [[ "${STALE_DAYS}" =~ ^[0-9]+$ ]]; then
    printf 'error: --stale-days must be a non-negative integer (got %s)\n' "${STALE_DAYS}" >&2
    exit 1
fi

SCRIPT_SRC="${BASH_SOURCE[0]}"
if [[ -L "${SCRIPT_SRC}" ]]; then
    SCRIPT_SRC="$(_grok_resolve_symlinks "${SCRIPT_SRC}")" || exit 1
fi
SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "${SCRIPT_SRC}")" && pwd -P)"

source "${SCRIPT_DIR}/grok-dev-env.sh"

cd "${SCRIPT_DIR}"

DEV_TARGET="${CARGO_TARGET_DIR}"
DEV_DEBUG="${DEV_TARGET}/debug"
INCREMENTAL="${DEV_DEBUG}/incremental"
DEPS="${DEV_DEBUG}/deps"
RELEASE_TARGET="${SCRIPT_DIR}/target"

# One `du` per path, humanized. Prints nothing for a missing path.
size_of() {
    [[ -e "$1" ]] || return 0
    du -sh "$1" 2>/dev/null | cut -f1
}

# Total size of the newline-separated paths in a file, humanized.
size_of_list() {
    local file="$1"
    [[ -s "${file}" ]] || {
        printf '0B'
        return 0
    }
    tr '\n' '\0' <"${file}" |
        xargs -0 du -sk 2>/dev/null |
        awk '{ s += $1 } END { printf "%.1f GB", s / 1048576 }'
}

report_row() {
    printf '  %-34s %10s  %s\n' "$1" "$2" "$3"
}

printf '\nBuild footprint\n'
report_row "target-dev (dev)" "$(size_of "${DEV_TARGET}")" "shared by ./grok-check.sh, ./grok-test.sh"
report_row "  debug/incremental" "$(size_of "${INCREMENTAL}")" "kept: makes rebuilds fast"
report_row "  debug/deps" "$(size_of "${DEPS}")" "kept: rlibs are linked, not cached"
report_row "target (deploy)" "$(size_of "${RELEASE_TARGET}")" "release-dist only; rebuilt by make deploy"
if command -v sccache >/dev/null 2>&1; then
    sccache_dir="${SCCACHE_DIR:-}"
    if [[ -z "${sccache_dir}" ]]; then
        case "$(uname -s)" in
            Darwin) sccache_dir="${HOME}/Library/Caches/Mozilla.sccache" ;;
            *) sccache_dir="${XDG_CACHE_HOME:-${HOME}/.cache}/sccache" ;;
        esac
    fi
    report_row "sccache cache" "$(size_of "${sccache_dir}")" "rustc incremental bypasses it"
fi

# --- 1. Superseded incremental sessions ------------------------------------
# Cargo resumes the newest session in a crate/configuration dir; earlier ones are
# unreachable. A trailing `.lock` file matches `s-*` too, so filter to dirs.
old_sessions="$(mktemp "${TMPDIR:-/tmp}/grok-clean-sessions.XXXXXX")"
trap 'rm -f "${old_sessions}"' EXIT

if [[ -d "${INCREMENTAL}" ]]; then
    find "${INCREMENTAL}" -maxdepth 1 -mindepth 1 -type d | while read -r dir; do
        # A directory about to be deleted wholesale (cold tree) makes its
        # superseded sessions moot; counting them twice would overstate the win.
        if [[ "${STALE_DAYS}" -gt 0 ]] &&
            [[ -n "$(find "${dir}" -maxdepth 0 -mtime "+${STALE_DAYS}" 2>/dev/null)" ]]; then
            continue
        fi
        find "${dir}" -maxdepth 1 -mindepth 1 -type d -name 's-*' -exec stat -f '%m %N' {} + 2>/dev/null |
            sort -rn |
            tail -n +2 |
            cut -d' ' -f2-
    done >"${old_sessions}"
fi
superseded_count="$(wc -l <"${old_sessions}" | tr -d ' ')"
superseded_size="$(size_of_list "${old_sessions}")"

# --- 2. Cold trees ---------------------------------------------------------
cold_incremental="$(mktemp "${TMPDIR:-/tmp}/grok-clean-cold.XXXXXX")"
cold_deps_exec="$(mktemp "${TMPDIR:-/tmp}/grok-clean-cold-deps.XXXXXX")"
trap 'rm -f "${old_sessions}" "${cold_incremental}" "${cold_deps_exec}"' EXIT

if [[ "${STALE_DAYS}" -gt 0 ]]; then
    [[ -d "${INCREMENTAL}" ]] &&
        find "${INCREMENTAL}" -maxdepth 1 -mindepth 1 -type d -mtime "+${STALE_DAYS}" >"${cold_incremental}"
    # Executables only: test/bin artifacts nothing links against. `deps/*.rlib`
    # stays, because an active crate links against cold dependencies.
    [[ -d "${DEPS}" ]] &&
        find "${DEPS}" -maxdepth 1 -type f -perm -u+x -mtime "+${STALE_DAYS}" >"${cold_deps_exec}"
fi
cold_incremental_count="$(wc -l <"${cold_incremental}" | tr -d ' ')"
cold_deps_count="$(wc -l <"${cold_deps_exec}" | tr -d ' ')"
cold_size="$(size_of_list "${cold_incremental}")"
cold_deps_size="$(size_of_list "${cold_deps_exec}")"

printf '\nPrunable\n'
report_row "superseded incremental sessions" "${superseded_size}" "${superseded_count} session dirs"
if [[ "${STALE_DAYS}" -gt 0 ]]; then
    report_row "incremental older than ${STALE_DAYS}d" "${cold_size}" "${cold_incremental_count} crate dirs"
    report_row "deps executables older than ${STALE_DAYS}d" "${cold_deps_size}" "${cold_deps_count} files"
else
    report_row "cold trees" "not scanned" "pass --stale-days N to include"
fi
[[ "${DROP_RELEASE_DIST}" == 1 ]] && report_row "target/release-dist" "$(size_of "${RELEASE_TARGET}/release-dist")" "--release-dist"
[[ "${DROP_TARGET_DEBUG}" == 1 ]] && report_row "target/debug" "$(size_of "${RELEASE_TARGET}/debug")" "--target-debug"
[[ "${DROP_TARGET_DEBUG}" == 1 ]] && report_row "target/cargo-timings" "$(size_of "${RELEASE_TARGET}/cargo-timings")" "--target-debug"

if [[ "${APPLY}" == 0 ]]; then
    printf '\nnothing deleted; re-run with --apply\n\n'
    exit 0
fi

printf '\nApplying\n'

# Delete by path list, tolerating concurrent cargo runs (a vanished path is fine).
delete_list() {
    local file="$1" label="$2" count=0
    [[ -s "${file}" ]] || return 0
    count="$(wc -l <"${file}" | tr -d ' ')"
    tr '\n' '\0' <"${file}" | xargs -0 rm -rf
    printf '  removed %s %s\n' "${count}" "${label}"
}

delete_list "${old_sessions}" "superseded sessions"
[[ "${STALE_DAYS}" -gt 0 ]] && delete_list "${cold_incremental}" "cold incremental dirs"
[[ "${STALE_DAYS}" -gt 0 ]] && delete_list "${cold_deps_exec}" "cold deps executables"

if [[ "${DROP_TARGET_DEBUG}" == 1 ]]; then
    rm -rf "${RELEASE_TARGET}/debug" "${RELEASE_TARGET}/cargo-timings"
    printf '  removed target/debug and target/cargo-timings\n'
fi
if [[ "${DROP_RELEASE_DIST}" == 1 ]]; then
    rm -rf "${RELEASE_TARGET}/release-dist"
    printf '  removed target/release-dist (next make deploy is a full build)\n'
fi
if [[ "${CLEAR_SCCACHE}" == 1 ]] && command -v sccache >/dev/null 2>&1; then
    # Stop the server first: it holds the files open, and the next build
    # restarts it on demand.
    sccache --stop-server >/dev/null 2>&1 || true
    rm -rf "${sccache_dir}"
    printf '  cleared sccache cache at %s\n' "${sccache_dir}"
fi

printf '\nAfter\n'
report_row "target-dev (dev)" "$(size_of "${DEV_TARGET}")" ""
report_row "target (deploy)" "$(size_of "${RELEASE_TARGET}")" ""
printf '\n'