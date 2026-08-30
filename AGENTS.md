# Grok Build Repository Instructions

These instructions apply to the entire repository. They are mandatory for every
agent, automation, and development session that reads this file.

This file is the **enforceable operations policy** for the repository. It
states what agents must and must not do. The canonical detailed architecture
and local-development explanation is [`GROK.md`](GROK.md). A concise
compatibility entry point for Claude-compatible agents is [`CLAUDE.md`](CLAUDE.md),
but that entry point still requires reading both `AGENTS.md` and `GROK.md` in
full before editing code or running commands.

## Required Reference

Read [`GROK.md`](GROK.md) in full before analyzing code, proposing changes,
running commands, or editing files. `GROK.md` is the canonical development and
architecture reference. Claude-compatible agents may start at
[`CLAUDE.md`](CLAUDE.md), but still must read `AGENTS.md` and `GROK.md` in
full before editing.

## Mandatory Rapid Development Checklist

Before every normal edit loop, verify:

1. `GROK_HOME` is set to `"${HOME}/.grokdev"` before the process starts.
2. `GROK_LEADER_SOCKET` is `"${GROK_HOME}/leader.sock"`.
3. `GROK_DISABLE_AUTOUPDATER=1` and Cursor/Claude/Codex discovery flags are
   disabled unless the task explicitly exercises a compatibility surface.
4. The default development commands are `./grok-check.sh`, `./grok-test.sh`,
   or `./grok-dev-runner.sh`.
5. Only one Cargo helper runs at a time against the worktree-local
   `./target-dev` directory.
6. Direct `cargo` commands are used only when a helper cannot express the
   needed operation, and `source ./grok-dev-env.sh` (or its exact environment)
   is applied first so they share `./target-dev` and the isolated profile.
7. Validation starts with the narrowest affected package or test; workspace-wide
   or release-dist builds are deliberate, slower steps.
8. `Cargo.lock` is not modified for source-only changes; `--locked` is used for
   reproducible builds.
9. No production profile, installed `grok` binary, deployed wrapper, or
   production leader socket is used.
10. Interactive TUI verification runs in a detached tmux session per the
    "Interactive TUI Verification via tmux" section below; `tmux attach` and
    physical-screen captures are never used by agents. Any change that
    touches the console (TUI rendering, input, sessions, model routing,
    slash commands, settings) is validated interactively in tmux before
    handoff — passing tests alone do not substitute for a live drive-through.

## Language Policy

- Use United States English (`en-US`) for all agent-authored communication.
- Write documentation, code comments, commit messages, test names, diagnostics,
  and newly introduced user-facing text in `en-US`.
- Do not switch languages because a prompt was written in another language.
- Preserve existing non-English content when unrelated to the task. Translate it
  only when the task explicitly requires translation.
- Follow standard Rust naming conventions for identifiers.

## Project Summary

Grok Build is a Rust 2024 terminal coding agent. The released command is `grok`;
the source artifact is `xai-grok-pager`.

The primary crates are:

- `crates/codegen/xai-grok-pager-bin`: composition root and executable entry
  point.
- `crates/codegen/xai-grok-pager`: TUI, rendering, input, and commands.
- `crates/codegen/xai-grok-shell`: agent runtime, sessions, sampling,
  stdio/headless/leader modes, and orchestration.
- `crates/codegen/xai-grok-tools`: built-in tool registration and execution.
- `crates/codegen/xai-grok-workspace`: filesystem, VCS, permissions,
  checkpoints, and worktrees.
- `crates/codegen/xai-grok-config`: configuration loading and path resolution.
- `crates/codegen/xai-grok-sandbox`: operating-system sandbox profiles.
- `crates/codegen/xai-grok-mcp`: MCP transports, authentication, and lifecycle.
- `crates/common`: shared protocols and runtime abstractions.
- `third_party`: vendored Mermaid rendering dependencies.

The high-level execution path is:

```text
xai-grok-pager-bin
  -> CLI and configuration
  -> sandbox initialization
  -> TUI, stdio, headless, serve, or leader mode
  -> agent session and model sampling
  -> tool registry and dispatch
  -> workspace, permission, filesystem, and VCS operations
```

## Mandatory Development Profile Isolation

The user's normal Grok state must never be touched by repository development or
verification. Every command that can build, test, run, inspect, or otherwise
execute repository code must start with the development environment configured.

The only persistent profile allowed for development is:

```text
~/.grokdev
```

Set the environment before the process starts. `GROK_HOME` is cached by the
application, so changing it after startup is not valid isolation.
The CLI option `--agent-profile` is unrelated: it selects an agent-definition
file and must not be used as a substitute for `GROK_HOME`.

At minimum, every development command must run with:

```sh
export GROK_HOME="${HOME}/.grokdev"
export GROK_LEADER_SOCKET="${GROK_HOME}/leader.sock"
export GROK_DISABLE_AUTOUPDATER=1
```

Before using the profile:

```sh
set -euo pipefail
umask 077
mkdir -p "${HOME}/.grokdev"
chmod 0700 "${HOME}/.grokdev"
```

For interactive runs, headless runs, integration tests, and any command that
loads Grok resources, also disable discovery of state owned by other coding
agents:

```sh
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
```

Compatibility-specific tests may override only the variables needed by the
scenario. Their external state must still come from temporary fixtures or the
development profile, never from a real user profile.

### Prohibited Development Actions

- Never run the installed `grok` command to validate repository changes.
- Never run `/opt/grok-custom/grok`, the `grok-custom` wrapper, `make deploy`,
  or any deployed artifact unless the user explicitly requests deployment
  work.
- Never use, read, write, migrate, copy, or delete development data in
  `~/.grok`, `~/.grok-prod`, or another production profile.
- Never copy `auth.json`, MCP credentials, sessions, memory, hooks, plugins, or
  configuration from a production profile into `~/.grokdev`.
- Never reuse a production leader socket. Use
  `~/.grokdev/leader.sock` exclusively.
- Never remove `~/.grokdev` wholesale without explicit user approval.

The development profile may require its own login. That is expected and is
safer than reusing production credentials.

## Running the Built Console

Use the freshly built repository binary. For ordinary TUI or headless
verification, pass `--no-leader` so a stale leader cannot substitute older
code:

```sh
source ./grok-dev-env.sh
cargo run -p xai-grok-pager-bin -- --no-leader --no-auto-update
```

Use leader mode only when the behavior under test requires it. In that case,
keep `GROK_LEADER_SOCKET` under `~/.grokdev` and verify that every participating
process uses the same isolated environment.

Do not assume exports from a previous terminal command are still active. When
commands are executed in separate shells, export the development environment
again in the same command invocation.

## Interactive TUI Verification via tmux

The agent's shell is non-interactive: each command runs, captures output, and
exits. A full-screen TUI needs a real PTY plus a persistent session the agent
can type into and read between commands. Use a detached tmux session for that.
This is the sanctioned way to exercise the console interactively; it does not
replace the PTY test harness for automated coverage.

### One-time setup

tmux is not preinstalled on every host. Install it once (an agent must ask the
user before installing software):

```sh
brew install tmux
```

### Session lifecycle

Create the session with the canonical environment inline — a detached tmux
session does not inherit exports from the current shell. Build the binary
first so the session runs fresh code:

```sh
# Build if needed (shares ./target-dev with the helpers; run under bash,
# because ./grok-dev-env.sh uses bash arrays).
bash -c 'source ./grok-dev-env.sh && cargo build -p xai-grok-pager-bin'

# Create the detached session. One session per task; never point it at
# ~/.grok or any production profile.
tmux new-session -d -s grok-dev -x 200 -y 50 \
  "export GROK_HOME=\"\$HOME/.grokdev\" \
   GROK_LEADER_SOCKET=\"\$HOME/.grokdev/leader.sock\" \
   GROK_DISABLE_AUTOUPDATER=1 \
   GROK_CURSOR_SKILLS_ENABLED=0 GROK_CURSOR_RULES_ENABLED=0 GROK_CURSOR_AGENTS_ENABLED=0 \
   GROK_CURSOR_MCPS_ENABLED=0 GROK_CURSOR_HOOKS_ENABLED=0 GROK_CURSOR_SESSIONS_ENABLED=0 \
   GROK_CLAUDE_SKILLS_ENABLED=0 GROK_CLAUDE_RULES_ENABLED=0 GROK_CLAUDE_AGENTS_ENABLED=0 \
   GROK_CLAUDE_MCPS_ENABLED=0 GROK_CLAUDE_HOOKS_ENABLED=0 GROK_CLAUDE_SESSIONS_ENABLED=0 \
   GROK_CODEX_SKILLS_ENABLED=0 GROK_CODEX_RULES_ENABLED=0 GROK_CODEX_AGENTS_ENABLED=0 \
   GROK_CODEX_MCPS_ENABLED=0 GROK_CODEX_HOOKS_ENABLED=0 GROK_CODEX_SESSIONS_ENABLED=0; \
   ./target-dev/debug/xai-grok-pager --no-leader --no-auto-update; exec zsh"
```

Ordinary verification always passes `--no-leader --no-auto-update`, exactly
like the direct-run path.

### Reading the screen

`capture-pane` is how an agent observes full-screen TUI state. Run it after
every interaction step and treat its output as ground truth for rendering,
modals, and footer state:

```sh
# Current pane as plain text (the agent's "screenshot").
tmux capture-pane -p -t grok-dev

# Include the last 500 lines of scrollback.
tmux capture-pane -p -t grok-dev -S -500

# Capture with ANSI colors preserved (input for the PNG pipeline below).
tmux capture-pane -e -p -t grok-dev
```

### Sending input

```sh
tmux send-keys -t grok-dev '/tersify lite' Enter  # type + submit
tmux send-keys -t grok-dev Escape                 # special keys by name
tmux send-keys -t grok-dev C-c C-q                # control combos as C-<key>
```

Wait briefly after each `send-keys` before the next `capture-pane` so async
TUI effects settle.

### Synthetic mouse clicks

Click-driven UI (hit-tested regions such as the context/status-bar segment
that opens the context view) is exercised by injecting raw SGR mouse
sequences — `send-keys -M` does not accept coordinates. Compute the target
column from a `capture-pane` line (`awk '{print index($0, "TARGET")}'`, row
from the line number), then send press + release with 1-based SGR
coordinates (`ESC [ < 0 ; COL ; ROW M` then `… m`):

```sh
# Left-click at column 138, row 1 (the "21K / 1.0M" context segment).
tmux send-keys -t grok-dev -H 1b 5b 3c 30 3b 31 33 38 3b 31 4d
tmux send-keys -t grok-dev -H 1b 5b 3c 30 3b 31 33 38 3b 31 6d
```

This requires the TUI's mouse capture to be active (it is, for hit-tested
regions) and replaces manual clicking entirely.

### Screenshots (PNG)

The text captures above are the default evidence. When a real image is
required — layout review, spacing/rendering bugs, color issues — convert the
colored capture with `freeze` (one-time install: `brew install freeze`):

```sh
tmux capture-pane -e -p -t grok-dev > /tmp/grok-pane.ansi
freeze --ansi /tmp/grok-pane.ansi -o /tmp/grok-pane.png
```

Read the image back and inspect the actual rendered layout before concluding
anything visual. Text captures prove content; screenshots prove layout.

### Screen recording (motion / interaction debug)

For layout behavior that depends on time — streaming renders, spinner/fold
animations, modal transitions, resize behavior — record the run and render a
GIF (one-time install: `brew install asciinema agg`):

```sh
# Launch the run inside an asciinema recording.
tmux new-session -d -s grok-dev -x 200 -y 50 \
  "export GROK_HOME=\"\$HOME/.grokdev\" GROK_DISABLE_AUTOUPDATER=1; \
   asciinema rec -c './target-dev/debug/xai-grok-pager --no-leader \
   --no-auto-update' /tmp/grok-run.cast"

# After reproducing the issue, render to GIF and inspect it.
agg /tmp/grok-run.cast /tmp/grok-run.gif
```

`screencapture` (physical screen, still or with `-V` video) photographs the
user's actual desktop, requires macOS Screen Recording permission, and can
capture unrelated windows; never use it unless the user explicitly asks.

### Operating the user's installed grok (explicit request only)

Agents normally never touch the deployed binary or production profiles. The
exception is an explicit user request to operate their grok — resuming a
crashed session, re-binding a model with `/model`, inspecting `/context`.
When authorized:

- Run the deployed binary in a dedicated tmux session with the user's own
  `GROK_HOME`. Do **not** add `--debug` unless the user asks for it.
- Drive it exactly like a dev session: `capture-pane` to read, `send-keys`
  (including SGR mouse clicks) to interact.
- Resume flow: `/` search by title in the resume dialog, arrow to the entry,
  `Enter`.
- Send **no prompts** unless the user asked for that specific interaction; a
  resumed session with live background tasks must not be nudged into new
  work.
- Finish with `/quit` and kill the tmux session. Report any state left
  behind — a model re-bind or a sent prompt is a state change the user must
  know about.

### Rules

- Never `tmux attach`: the agent interacts only through `send-keys` /
  `capture-pane`. Attaching would take over the user's terminal.
- Development sessions always launch with the canonical environment inline
  and `--no-leader --no-auto-update`, and every isolation rule of this file
  applies inside them, including never touching production profiles. The
  only exception is the explicit user-request path above, which has its own
  rules and stays read-mostly.
- One development console per tmux session. Kill the session when
  verification ends so a stale TUI cannot hold the development profile:

```sh
tmux kill-session -t grok-dev
```

- Automated regression coverage still belongs in the PTY harness
  (`crates/codegen/xai-grok-pager-pty-harness`, `tests/pty_e2e/`); tmux is for
  interactive diagnosis and manual verification.

## Build and Validation

Install DotSlash first because `bin/protoc` uses it for hermetic protobuf
tooling.

### Mandatory normal workflow

For ordinary development, agents **must** prefer the crate-targeted helper
scripts:

- `./grok-check.sh` for fast focused checks.
- `./grok-test.sh` for fast `cargo-nextest` runs.
- `./grok-dev-runner.sh` only when the experimental `claude-cli-runtime`
  feature is required.

These helpers **must** be used by default because they:

- enforce the canonical isolated `~/.grokdev` profile;
- disable discovery of Cursor / Claude / Codex state;
- share one worktree-local `./target-dev` directory so check, test, and run
  reuse the same Cargo artifacts instead of compiling the dependency graph
  multiple times.

```sh
# Fast focused check first.
./grok-check.sh -p xai-grok-shell
./grok-check.sh -p xai-grok-pager-bin

# Focused test with cargo-nextest after the check passes.
./grok-test.sh -p xai-grok-shell
./grok-test.sh -p xai-grok-shell -- session_runtime_family
./grok-test.sh -p xai-grok-shell --run-ignored all
./grok-test.sh -p xai-grok-shell --no-run

# Experimental claude-cli-runtime run (only when needed).
./grok-dev-runner.sh --no-leader --no-auto-update
```

### Direct Cargo commands

Direct `cargo` commands are allowed **only** when a helper cannot express the
needed operation. Before running any direct `cargo check`, `cargo test`,
`cargo clippy`, `cargo fmt`, or similar command from Bash, `source
./grok-dev-env.sh` (or reproduce its exact environment) so the command:

- uses the isolated `~/.grokdev` profile;
- shares `./target-dev` with the helpers;
- applies the same Cursor/Claude/Codex discovery disablement.

```sh
source ./grok-dev-env.sh
cargo test -p xai-grok-config
cargo clippy -p xai-grok-shell
cargo fmt --all --check
```

### One helper at a time

Run **one** Cargo helper at a time by default. The helpers share the same
`./target-dev` directory, and Cargo serializes access with its own file lock.
Waiting on that lock is expected and preferable to launching duplicate compile
graphs. Do **not** delete `.cargo` file locks, kill another build, or bypass
Cargo's locking merely to avoid waiting.

Intentional parallel experiments must set a separate worktree-local or
temporary `CARGO_TARGET_DIR`:

```sh
CARGO_TARGET_DIR=/tmp/grok-local-target ./grok-check.sh -p xai-grok-shell
```

Independent worktrees get their own root-local `target-dev` by default. Never
point multiple active worktrees at the same target directory.

### Start narrow

Always start validation with the narrowest affected package, target, or test:

1. `./grok-check.sh -p <affected-crate>`
2. `./grok-test.sh -p <affected-crate>`
3. `./grok-check.sh -p <direct-consumer>` and `./grok-test.sh -p <direct-consumer>`
4. Full-workspace or release-dist validation only when the change crosses
   enough crate boundaries to justify the cost.

`cargo build -p xai-grok-pager-bin --release` and `make build` are the
deliberate slow/comprehensive paths and remain separate. Do not use them in
ordinary edit loops.

### Test runners

`cargo-nextest` is the fast local default. `cargo test` remains fully
supported for shared-process semantics and ignores `.config/nextest.toml`.
Keep nextest as the default for speed; use `cargo test` only when the
different process model matters.

Passing `--no-capture` to installed `cargo-nextest` serializes execution. Use
it only for short diagnostic runs.

### Makefile

`Makefile` targets are release-dist and deployment-oriented. Default `make build`
uses `--profile release-dist`, plus `--locked`, `--timings`, sccache/jobs, and
optional `FEATURES=`. Do not use `make build`, `make deploy`,
`make deploy-binary`, `make deploy-wrapper`, or `make verify` in ordinary edit
loops; deployment and verify targets require an explicit user request.

### Validation practices

- Start with the narrowest affected crate.
- Check direct consumers after the focused tests pass.
- Use full-workspace validation only when the change crosses enough crate
  boundaries to justify its cost.
- TUI changes require relevant render, snapshot, scenario, or PTY-harness
  coverage.
- Permission, sandbox, session, leader, and MCP changes require failure-path
  tests in addition to happy-path tests.

### Cargo.lock discipline

`Cargo.lock` is tracked and must remain deterministic. Use `--locked` for
reproducible builds and release verification. Ordinary source-only changes must
not modify `Cargo.lock`. When a dependency, feature, or manifest change
intentionally alters resolution, regenerate the lockfile with the pinned
toolchain in the canonical isolated environment, review the diff, and include
it. Never hand-edit `Cargo.lock`. Do not run `cargo update` broadly; use a
targeted `cargo update -p <name> --precise <version>` instead. If `--locked`
fails, diagnose the manifest/lock mismatch rather than removing `--locked` or
deleting the lockfile.

Distinguish the persistent `Cargo.lock` dependency lockfile from transient
Cargo file locks under `.cargo` and `target/`.

### Version bumps (fcustom)

The fcustom CLI version lives in exactly four lockstep manifests:
`xai-grok-pager`, `xai-grok-shell`, `xai-grok-pager-bin`, and
`xai-grok-version`. Bumping it is a deliberate release action, not an
ordinary edit:

- Use **plain semver with no pre-release suffix**. A pre-release suffix
  makes the stable-channel auto-updater force-install the feed's latest
  stable over the custom build; non-semver strings are rejected by Cargo in
  manifests and break `minimum_version`/`version_overrides` parsing.
- Keep the fcustom base ahead of the upstream stable feed
  (currently `1.0.0 > 0.2.x`) so the auto-updater never replaces a custom
  build with a feed release.
- Edit the four lockstep manifests only; regenerate `Cargo.lock` in the
  canonical isolated environment with a direct cargo command, confirm the
  diff contains only the four member version lines, and commit manifests and
  lockfile together.
- Finish with an explicit `make deploy` + `make verify`; optionally mark the
  commit with a local annotated tag `v<major.minor.patch>-fcustom` (tag
  pushes are blocked; tags are local markers).

The canonical procedure, including how the version flows into the binary and
why deployed builds display ` [alpha]`, is in
[`GROK.md`](GROK.md#version-identity-and-bumps-fcustom).

### Tooling policy

Do not add custom build serialization, new linker flags, `mold`, or
alternative caching without measurement and an explicit rationale in the
change. Cargo's own file locks, the shared worktree-local `./target-dev`, and
`sccache` are the current policy. sccache and Linux `lld` are already
configured in `.cargo/config.toml`.

## Editing Rules

- Treat the generated root `Cargo.toml` as read-only. Edit crate manifests.
- Preserve purposeful crate boundaries.
- Avoid adding dependencies to foundational crates without checking downstream
  impact.
- Use standard Rust formatting and the pinned toolchain.
- Keep changes focused and preserve unrelated work in the working tree.
- Do not weaken permission gates, sandboxing, managed-configuration
  fail-closed behavior, credential handling, or path containment unless the
  task explicitly targets that behavior and includes appropriate tests.

## Test Organization

Unit tests normally live beside the implementation. Cross-crate and process
behavior belongs in each crate's `tests/` directory. Important test surfaces
include:

- `crates/codegen/xai-grok-pager/tests`: TUI, scenarios, leader behavior, and
  PTY end-to-end tests.
- `crates/codegen/xai-grok-shell/tests`: sessions, MCP, authentication,
  leader, recovery, and sandbox integration.
- `crates/codegen/xai-grok-pager-pty-harness`: reusable PTY test support.
- `crates/codegen/xai-grok-workspace/src/permission`: permission policy and
  command-gating tests.
- `crates/codegen/xai-grok-sandbox/tests`: sandbox integration tests.

Name tests after observable behavior, such as
`replays_session_after_reconnect`.

## Security

Never commit credentials, API keys, session data, memory databases, MCP
credentials, or files copied from any Grok home. Treat `~/.grokdev` as
sensitive local state even though it is a development profile.

Follow `SECURITY.md` for vulnerabilities. This public mirror does not accept
external pull requests; follow `CONTRIBUTING.md`.
