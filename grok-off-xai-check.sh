#!/usr/bin/env bash
#
# grok-off-xai-check.sh — the off-xAI acceptance battery.
#
# Proves, against a freshly built binary and with xAI made unreachable, that
# this fork still runs, and that every xAI-hosted surface goes quiet under
# GROK_XAI_ENABLED=0. Four scenarios plus one control; each one asserts, and a
# regression fails the script instead of printing a line.
#
# ---------------------------------------------------------------------------
# The blackhole technique
# ---------------------------------------------------------------------------
#
# xAI is made unreachable by pointing its two base URLs at
# http://10.255.255.1:9/v1 (override with OFF_XAI_BLACKHOLE):
#
#   GROK_CLI_CHAT_PROXY_BASE_URL   the settings / relay origin
#   GROK_XAI_API_BASE_URL          the inference origin
#
# 10.255.255.1 is a non-routable address with nothing listening on port 9, so a
# TCP connect there *hangs until the client timeout* rather than being refused
# with an RST. That is the point: a blackhole exercises the timeout budget and
# the fallback paths, while an unreachable-but-refusing host would fail fast and
# hide them. A scenario that "passes" only because xAI answered instantly would
# prove nothing.
#
# Env overrides are used rather than /etc/hosts so nothing on the host changes.
# An `[endpoints]` key in config.toml outranks these env vars, so a scratch
# GROK_HOME is used per scenario and no ambient config.toml leaks in.
#
# ---------------------------------------------------------------------------
# Isolation
# ---------------------------------------------------------------------------
#
# Each scenario gets its own scratch GROK_HOME under $TMPDIR, seeded only with
# what that scenario needs. The real ~/.grok is never read or written, and
# ~/.grokdev is only read as the default source of a grok.com session
# (OFF_XAI_AUTH_JSON). Every run also disables Cursor/Claude/Codex discovery so
# a stray ~/.claude.json cannot change the MCP server list mid-battery.
#
# Requires: bash, python3 (the stub gateway), and a built xai-grok-pager.
# Build first:
#
#   bash -c 'source ./grok-dev-env.sh && cargo build -p xai-grok-pager-bin'
#
# Run:
#
#   bash grok-off-xai-check.sh                     # skips the session scenarios if no auth.json
#   bash grok-off-xai-check.sh --require-session   # ...and fails instead of skipping
#   OFF_XAI_KEEP=1 bash grok-off-xai-check.sh      # keep the scratch dir for inspection
#
# Environment overrides: OFF_XAI_BIN (binary under test), OFF_XAI_BLACKHOLE,
# OFF_XAI_AUTH_JSON (source of a grok.com session), OFF_XAI_TIMEOUT (per-run
# wall clock).
#
# Two things this script learned the hard way, worth knowing before editing it:
#
#   * The stub gateway's Python source is emitted with `printf`, not a
#     here-document. A `cat >file <<EOF` here hangs under bash 5.3 (Homebrew's
#     bash) -- the heredoc arrives empty and `cat` blocks forever -- while
#     macOS's bash 3.2 is fine with it. `bash --version` decides which one you
#     get, and a Homebrew `bash` ahead of `/bin` in PATH picks the broken one.
#   * `--no-leader` is an `agent`-subcommand option, not a top-level one. The
#     agent runs below use `agent --no-leader headless`; a top-level
#     `--no-leader` in front of `agent` aborts with a usage error.
#
# ---------------------------------------------------------------------------
# What each scenario asserts
# ---------------------------------------------------------------------------
#
# 1. raw-no-credential — nothing configured, xAI blackholed.
#    Asserts the run does not hang and does not silently pick an endpoint: it
#    exits non-zero, and stderr carries the no-credential remedy naming both the
#    env tier and the login. This is the "fail loudly" half of the phase.
#
# 2. headless-no-session — plain bearer + stub gateway, no grok.com session.
#    Asserts `agent headless` announces the stdio-ACP fallback and never opens
#    the relay, and that it still exits 0.
#
# 3. session-present-switch-off — same, plus a grok.com session in auth.json and
#    GROK_XAI_ENABLED=0. Asserts the relay stays shut even though a session
#    exists, and that the switch really was applied (`xAI surfaces disabled`).
#
# 4. session-present-switch-on (control) — identical to 3 with
#    GROK_XAI_ENABLED=1. Asserts the relay DOES connect. Without this control,
#    scenario 3 would also pass on a binary that never relays at all.
#
# 5. full-off-xai — the documented recipe (bearer + base URL + both kill
#    switches), xAI blackholed, a turn actually completed.
#    Asserts exit 0, a real answer on stdout, no relay, and no settings fetch.
#
# A failure in any assertion is accumulated; the script exits non-zero and
# re-lists every failure at the end.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="${OFF_XAI_BIN:-${CARGO_TARGET_DIR:-$ROOT/target-dev}/debug/xai-grok-pager}"
BLACKHOLE="${OFF_XAI_BLACKHOLE:-http://10.255.255.1:9/v1}"
AUTH_SOURCE="${OFF_XAI_AUTH_JSON:-$HOME/.grokdev/auth.json}"
REQUIRE_SESSION=0
SCENARIO_TIMEOUT="${OFF_XAI_TIMEOUT:-120}"

for arg in "$@"; do
    case "$arg" in
    --require-session) REQUIRE_SESSION=1 ;;
    -h | --help)
        awk 'NR > 1 { if ($0 ~ /^set -uo pipefail/) exit; sub(/^# ?/, ""); print }' \
            "${BASH_SOURCE[0]}"
        exit 0
        ;;
    *)
        echo "unknown argument: $arg" >&2
        exit 2
        ;;
    esac
done

if [ ! -x "$BIN" ]; then
    echo "error: $BIN is not executable; build with:" >&2
    echo "  bash -c 'source ./grok-dev-env.sh && cargo build -p xai-grok-pager-bin'" >&2
    exit 2
fi
if ! command -v python3 >/dev/null 2>&1; then
    echo "error: python3 is required for the stub gateway" >&2
    exit 2
fi

WORK="$(mktemp -d "${TMPDIR:-/tmp}/grok-off-xai-check.XXXXXX")"
STUB_PID=""
cleanup() {
    [ -n "$STUB_PID" ] && kill "$STUB_PID" 2>/dev/null
    if [ "${OFF_XAI_KEEP:-0}" = 1 ]; then
        echo "  scratch kept: $WORK"
    else
        rm -rf "$WORK"
    fi
}
trap cleanup EXIT

PASS=0
FAIL=0
SKIP=0
declare -a FAILURES=()

ok() { PASS=$((PASS + 1)); printf '  \033[32mPASS\033[0m %s\n' "$1"; }
bad() {
    FAIL=$((FAIL + 1))
    FAILURES+=("$1")
    printf '  \033[31mFAIL\033[0m %s\n' "$1"
}
note() { printf '  .... %s\n' "$1"; }
skip() { SKIP=$((SKIP + 1)); printf '  \033[33mSKIP\033[0m %s\n' "$1"; }

assert_eq() { # <want> <got> <label>
    if [ "$1" = "$2" ]; then ok "$3"; else bad "$3 (wanted '$1', got '$2')"; fi
}
assert_contains() { # <file> <regex> <label>
    if grep -qE "$2" "$1" 2>/dev/null; then ok "$3"; else bad "$3 (no /$2/ in $1)"; fi
}
assert_absent() { # <file> <regex> <label>
    if grep -qE "$2" "$1" 2>/dev/null; then
        bad "$3 (found /$2/ in $1: $(grep -m1 -E "$2" "$1"))"
    else
        ok "$3"
    fi
}

# Run a command under a wall-clock watchdog; no coreutils `timeout` on macOS.
with_timeout() { # <secs> <out> <err> <cmd...>
    local secs="$1" out="$2" err="$3"
    shift 3
    "$@" >"$out" 2>"$err" &
    local pid=$!
    (
        sleep "$secs"
        kill -TERM "$pid" 2>/dev/null
        sleep 2
        kill -KILL "$pid" 2>/dev/null
    ) &
    local watcher=$!
    wait "$pid"
    local rc=$?
    kill "$watcher" 2>/dev/null
    wait "$watcher" 2>/dev/null
    return $rc
}

# Export the canonical isolated environment into the current shell. Every
# scenario runs inside a subshell that called this first.
iso_env() { # <scratch-home>
    export GROK_HOME="$1"
    export GROK_LEADER_SOCKET="$1/leader.sock"
    export GROK_DISABLE_AUTOUPDATER=1
    export GROK_CURSOR_SKILLS_ENABLED=0 GROK_CURSOR_RULES_ENABLED=0
    export GROK_CURSOR_AGENTS_ENABLED=0 GROK_CURSOR_MCPS_ENABLED=0
    export GROK_CURSOR_HOOKS_ENABLED=0 GROK_CURSOR_SESSIONS_ENABLED=0
    export GROK_CLAUDE_SKILLS_ENABLED=0 GROK_CLAUDE_RULES_ENABLED=0
    export GROK_CLAUDE_AGENTS_ENABLED=0 GROK_CLAUDE_MCPS_ENABLED=0
    export GROK_CLAUDE_HOOKS_ENABLED=0 GROK_CLAUDE_SESSIONS_ENABLED=0
    export GROK_CODEX_SKILLS_ENABLED=0 GROK_CODEX_RULES_ENABLED=0
    export GROK_CODEX_AGENTS_ENABLED=0 GROK_CODEX_MCPS_ENABLED=0
    export GROK_CODEX_HOOKS_ENABLED=0 GROK_CODEX_SESSIONS_ENABLED=0
    # The blackhole. Nothing else here reaches the network.
    export GROK_CLI_CHAT_PROXY_BASE_URL="$BLACKHOLE"
    export GROK_XAI_API_BASE_URL="$BLACKHOLE"
    # `relay_connecting` / `xAI surfaces disabled` are info-level instrumentation.
    export RUST_LOG=info
    # No ambient credential unless a scenario sets one.
    unset GROK_API_KEY XAI_API_KEY GROK_MODELS_BASE_URL GROK_XAI_ENABLED GROK_REMOTE_FETCH
}

scratch_home() { # <name> [--with-session]
    local home="$WORK/$1"
    mkdir -p "$home"
    if [ "${2:-}" = "--with-session" ]; then
        cp "$AUTH_SOURCE" "$home/auth.json"
    fi
    printf '%s' "$home"
}

has_xai_session() {
    [ -f "$AUTH_SOURCE" ] && grep -q 'accounts\.x\.ai\|auth\.x\.ai\|Oidc' "$AUTH_SOURCE"
}

# ---------------------------------------------------------------------------
# Stub gateway: an OpenAI-compatible endpoint that is NOT xAI.
# ---------------------------------------------------------------------------
# The stub source is emitted with `printf` rather than a here-document. Under
# bash 5.3 -- which is what a bare `bash` resolves to whenever
# /opt/homebrew/bin is ahead of /bin in PATH -- the here-document below hangs:
# it arrives empty and `cat` blocks forever on the still-open pipe. macOS ships
# bash 3.2, where the very same heredoc is fine, so the hang only shows up on
# machines with Homebrew bash first. `printf` sidesteps it under both.
printf '%s\n' \
    'import json, sys' \
    'from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer' \
    '' \
    'class Handler(BaseHTTPRequestHandler):' \
    '    protocol_version = "HTTP/1.1"' \
    '' \
    '    def log_message(self, *a):' \
    '        pass' \
    '' \
    '    def _json(self, obj, code=200):' \
    '        body = json.dumps(obj).encode()' \
    '        self.send_response(code)' \
    '        self.send_header("Content-Type", "application/json")' \
    '        self.send_header("Content-Length", str(len(body)))' \
    '        self.end_headers()' \
    '        self.wfile.write(body)' \
    '' \
    '    def do_GET(self):' \
    '        if self.path.rstrip("/").endswith("/models"):' \
    '            self._json({"data": [{' \
    '                "id": "grok-build",' \
    '                "model": "grok-build",' \
    '                "name": "Stub Gateway",' \
    '                "context_window": 128000,' \
    '                "api_backend": "chat_completions",' \
    '            }]})' \
    '        else:' \
    '            self._json({"data": []})' \
    '' \
    '    def do_POST(self):' \
    '        length = int(self.headers.get("Content-Length") or 0)' \
    '        self.rfile.read(length)' \
    '        if not self.path.rstrip("/").endswith("/chat/completions"):' \
    '            self._json({"data": []})' \
    '            return' \
    '        self.send_response(200)' \
    '        self.send_header("Content-Type", "text/event-stream")' \
    '        self.send_header("Cache-Control", "no-cache")' \
    '        self.end_headers()' \
    '        for chunk in (' \
    '            {"id": "c1", "object": "chat.completion.chunk", "created": 1700000000,' \
    '             "model": "grok-build",' \
    '             "choices": [{"index": 0, "delta": {"role": "assistant", "content": "OK"},' \
    '                          "finish_reason": None}]},' \
    '            {"id": "c1", "object": "chat.completion.chunk", "created": 1700000000,' \
    '             "model": "grok-build",' \
    '             "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],' \
    '             "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}},' \
    '        ):' \
    '            self.wfile.write(b"data: " + json.dumps(chunk).encode() + b"\n\n")' \
    '            self.wfile.flush()' \
    '        self.wfile.write(b"data: [DONE]\n\n")' \
    '        self.wfile.flush()' \
    '' \
    'ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()' \
    >"$WORK/stub.py"

start_stub() {
    local port
    port="$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1]);s.close()')"
    python3 "$WORK/stub.py" "$port" &
    STUB_PID=$!
    local i
    for i in $(seq 1 50); do
        if (exec 3<>"/dev/tcp/127.0.0.1/$port") 2>/dev/null; then break; fi
        sleep 0.1
    done
    STUB_PORT="$port"
    STUB_BASE="http://127.0.0.1:$port/v1"
    echo "  stub gateway on $STUB_BASE (pid $STUB_PID)"
}

# ---------------------------------------------------------------------------
# Scenarios
# ---------------------------------------------------------------------------
sc_raw_no_credential() {
    local home out err rc
    home="$(scratch_home raw)"
    out="$WORK/raw.out"
    err="$WORK/raw.err"
    (
        iso_env "$home"
        with_timeout "$SCENARIO_TIMEOUT" "$out" "$err" \
            "$BIN" --no-leader --no-auto-update -p 'Reply with exactly: OK'
    )
    rc=$?
    assert_eq 1 "$rc" "exits non-zero instead of hanging on the blackhole"
    assert_contains "$err" 'No credential for model' "stderr carries the no-credential remedy"
    assert_contains "$err" 'GROK_MODELS_BASE_URL' "remedy names the env tier"
    assert_contains "$err" 'grok provider connect xai' "remedy names the login path"
    note "rc=$rc"
}

sc_headless_no_session() {
    local home out err rc
    home="$(scratch_home headless)"
    out="$WORK/headless.out"
    err="$WORK/headless.err"
    (
        iso_env "$home"
        export GROK_API_KEY=stub-bearer
        export GROK_MODELS_BASE_URL="$STUB_BASE"
        with_timeout "$SCENARIO_TIMEOUT" "$out" "$err" \
            "$BIN" agent --no-leader headless </dev/null
    )
    rc=$?
    assert_contains "$err" 'No xAI session: serving the agent over stdio' \
        "announces the stdio-ACP fallback"
    assert_absent "$err" 'relay_connecting' "never opens the relay"
    assert_eq 0 "$rc" "exits 0 on stdin EOF"
    note "rc=$rc"
}

sc_session_switch_off() {
    local home out err rc
    home="$(scratch_home session-off --with-session)"
    out="$WORK/session-off.out"
    err="$WORK/session-off.err"
    (
        iso_env "$home"
        export GROK_XAI_ENABLED=0
        export GROK_API_KEY=stub-bearer
        export GROK_MODELS_BASE_URL="$STUB_BASE"
        with_timeout "$SCENARIO_TIMEOUT" "$out" "$err" \
            "$BIN" agent --no-leader headless </dev/null
    )
    rc=$?
    assert_contains "$err" 'xAI surfaces disabled' "the switch is actually applied"
    assert_absent "$err" 'relay_connecting' "relay stays shut with a session present"
    assert_eq 0 "$rc" "exits 0"
    note "rc=$rc"
}

sc_session_switch_on() {
    local home out err rc
    home="$(scratch_home session-on --with-session)"
    out="$WORK/session-on.out"
    err="$WORK/session-on.err"
    (
        iso_env "$home"
        export GROK_XAI_ENABLED=1
        export GROK_API_KEY=stub-bearer
        export GROK_MODELS_BASE_URL="$STUB_BASE"
        with_timeout "$SCENARIO_TIMEOUT" "$out" "$err" \
            "$BIN" agent --no-leader headless </dev/null
    )
    rc=$?
    assert_contains "$err" 'relay_connecting' "control: the relay DOES connect with the switch on"
    note "rc=$rc"
}

sc_full_off_xai() {
    local home out err rc
    home="$(scratch_home full)"
    out="$WORK/full.out"
    err="$WORK/full.err"
    (
        iso_env "$home"
        # The documented recipe, verbatim.
        export GROK_API_KEY=stub-bearer
        export GROK_MODELS_BASE_URL="$STUB_BASE"
        export GROK_XAI_ENABLED=0
        export GROK_REMOTE_FETCH=0
        export GROK_CHANGELOG_OFFLINE=1
        export GROK_TELEMETRY_ENABLED=false
        export GROK_FEEDBACK_ENABLED=false
        with_timeout "$SCENARIO_TIMEOUT" "$out" "$err" \
            "$BIN" --no-leader --no-auto-update -p 'Reply with exactly: OK'
    )
    rc=$?
    assert_eq 0 "$rc" "a turn completes against a non-xAI gateway with xAI blackholed"
    assert_contains "$out" 'OK' "the answer reaches stdout"
    assert_absent "$err" 'relay_connecting' "no relay"
    assert_contains "$err" 'xAI surfaces disabled' "the switch is actually applied"
    note "rc=$rc"
}

# ---------------------------------------------------------------------------
echo "off-xAI battery"
echo "  repo   : $ROOT"
echo "  binary : $BIN"
echo "  xAI    : blackholed at $BLACKHOLE"
echo "  session: $AUTH_SOURCE"
echo

start_stub

echo
echo "== 1. raw no-credential run (xAI blackholed) =="
sc_raw_no_credential

echo
echo "== 2. headless with no grok.com session =="
sc_headless_no_session

if has_xai_session; then
    echo
    echo "== 3. grok.com session present, GROK_XAI_ENABLED=0 =="
    sc_session_switch_off

    echo
    echo "== 4. control: grok.com session present, GROK_XAI_ENABLED=1 =="
    sc_session_switch_on
else
    echo
    echo "== 3. grok.com session present, GROK_XAI_ENABLED=0 =="
    skip "no grok.com session at $AUTH_SOURCE (set OFF_XAI_AUTH_JSON)"
    echo
    echo "== 4. control: grok.com session present, GROK_XAI_ENABLED=1 =="
    skip "no grok.com session at $AUTH_SOURCE"
    if [ "$REQUIRE_SESSION" = 1 ]; then
        bad "session scenarios were required but could not run"
    fi
fi

echo
echo "== 5. full off-xAI run =="
sc_full_off_xai

echo
echo "-------------------------------------------------------------"
printf 'passed %d, failed %d, skipped %d\n' "$PASS" "$FAIL" "$SKIP"
if [ "$FAIL" -gt 0 ]; then
    echo
    echo "failures:"
    for f in "${FAILURES[@]}"; do printf '  - %s\n' "$f"; done
    exit 1
fi
if [ "$SKIP" -gt 0 ]; then
    echo "note: $SKIP scenario(s) skipped — re-run with --require-session to enforce them"
fi
exit 0