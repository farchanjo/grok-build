# Grok Build Development Guide

This document is the canonical technical and local-development reference for
this checkout. Repository agents must read it before making changes. Operational
requirements are enforced by [`AGENTS.md`](AGENTS.md).

## Language

All new project communication and authored content must use United States
English (`en-US`). This includes documentation, code comments, test names,
diagnostics, commit messages, and user-facing strings. Existing content should
not be translated as part of an unrelated change.

## What This Project Is

Grok Build is a terminal-based AI coding agent implemented as a Rust 2024
workspace. It supports:

- an interactive full-screen terminal UI;
- single-turn and headless automation;
- stdio integration through the Agent Client Protocol;
- shared leader processes for reconnecting and multi-client sessions;
- model inference and tool calling;
- filesystem, shell, VCS, search, MCP, and media tools;
- session persistence, worktrees, checkpoints, memory, and compaction;
- permission policies and operating-system sandboxing.

The composition-root package is `xai-grok-pager-bin`. It builds the
`xai-grok-pager` artifact, which official releases expose as `grok`.

## Repository Map

### Product Core

| Path | Responsibility |
| --- | --- |
| `crates/codegen/xai-grok-pager-bin` | Process startup, CLI dispatch, crash handling, sandbox startup, leader connection, and top-level composition |
| `crates/codegen/xai-grok-pager` | TUI state, input, rendering, scrollback, commands, and user interaction |
| `crates/codegen/xai-grok-shell` | Agent runtime, sessions, inference, tools bridge, stdio/headless/serve/leader modes, memory, and orchestration |
| `crates/codegen/xai-grok-tools` | Built-in tool descriptions, registry, dispatch, persistence, and implementations |
| `crates/codegen/xai-grok-workspace` | Filesystem and VCS abstraction, execution, permissions, checkpoints, worktrees, and workspace services |

### Supporting Systems

| Path | Responsibility |
| --- | --- |
| `crates/codegen/xai-grok-config` | Layered configuration, managed requirements, Grok home resolution, and application paths |
| `crates/codegen/xai-grok-config-types` | Shared configuration schema |
| `crates/codegen/xai-grok-sandbox` | Workspace, strict, devbox, custom, and disabled sandbox profiles |
| `crates/codegen/xai-grok-mcp` | MCP server transports, credentials, OAuth, liveness, and ACP bridging |
| `crates/codegen/xai-grok-agent` | Agent prompt templates and skill-related prompt construction |
| `crates/codegen/xai-grok-memory` | Cross-session memory, indexing, search, and consolidation |
| `crates/codegen/xai-grok-pager-render` | Themes, syntax presentation, terminal glyphs, and rendering primitives |
| `crates/codegen/xai-grok-pager-pty-harness` | PTY-based test infrastructure |
| `crates/common/xai-tool-runtime` | Core `Tool`, `ToolDyn`, streaming, and typed-output abstractions |
| `crates/common/xai-tool-protocol` | JSON-RPC protocol frames and tool/session identifiers |
| `crates/common/xai-computer-hub-*` | Tool-server registry, transport, pooling, and MCP adaptation |
| `crates/common/xai-grok-compaction` | History compaction strategies and host-independent traits |
| `third_party` | Vendored Mermaid rendering stack |

The root `Cargo.toml` is generated. Treat it as read-only and edit the relevant
crate manifest instead.

## Runtime Architecture

The primary executable entry point is:

```text
crates/codegen/xai-grok-pager-bin/src/main.rs
```

Startup follows this general sequence:

```text
main
  -> early subprocess dispatch and CLI parsing
  -> crash, telemetry, and runtime setup
  -> managed requirements validation
  -> GROK_HOME-backed configuration loading
  -> sandbox profile resolution and installation
  -> command dispatch
     -> interactive TUI
     -> single-turn/headless
     -> ACP stdio
     -> WebSocket serve
     -> shared leader
  -> agent session
  -> model inference
  -> tool selection and dispatch
  -> permission gate
  -> workspace or external operation
  -> persisted session and UI update
```

### TUI

The TUI is centered in `xai-grok-pager/src/app`. It uses an Elm-like separation:

- actions describe events;
- dispatch mutates synchronous state and produces effects;
- effects perform asynchronous work;
- `AppView` is the root input and drawing component;
- the event loop coordinates terminal, agent, and task events.

Visible TUI changes should be tested through render assertions, snapshots,
scripted scenarios, or the PTY harness.

### Agent Runtime and Sessions

`xai-grok-shell` owns agent modes and durable session behavior. Sessions are
ACP-oriented and coordinate prompts, model turns, tool calls, persistence,
compaction, goals, memory, MCP servers, and worktrees.

Session state is stored beneath:

```text
$GROK_HOME/sessions/
```

This is one reason development must never use the default `~/.grok` profile.

### Tools

`xai-grok-tools` registers built-in tools and adapts them to the shared tool
runtime. Tool execution can stream progress before producing one terminal
result. Filesystem, shell, web, MCP, LSP, media, task, and editing tools share
common session resources.

Tool changes must preserve:

- typed argument validation;
- permission classification and prompting;
- progress and terminal-result ordering;
- path and workspace containment;
- durable state and notification behavior;
- safe error reporting without credential leakage.

### Permissions and Sandbox

Permissions and sandboxing are distinct layers:

- permission policy decides whether an operation is allowed, denied, or must be
  confirmed;
- sandboxing constrains what the operating system permits the process or child
  process to access.

Do not treat one as a replacement for the other. Changes to shell parsing,
command wrappers, path matching, MCP access, web access, or sandbox profiles are
security-sensitive and require adversarial failure-path tests.

### MCP

`xai-grok-mcp` owns MCP transports, authentication, OAuth, process lifecycle,
server health, and tool invocation. MCP credentials are stored below
`GROK_HOME`; they must remain separate from production credentials.

### Leader Mode

Leader mode allows multiple clients to share an agent process and supports
reconnection with ACP state replay. A leader can outlive the binary invocation
that started a client, so accidentally connecting a new build to an old leader
can invalidate a test.

Ordinary build verification must use `--no-leader`. Tests specifically covering
leader behavior must use the isolated development socket:

```text
~/.grokdev/leader.sock
```

## Sealed Development Console

### Non-Negotiable Profile

All repository builds, tests, inspections, and executions must use:

```text
~/.grokdev
```

The application reads the location from `GROK_HOME`. Use the absolute,
shell-expanded form:

```sh
export GROK_HOME="${HOME}/.grokdev"
```

The `--agent-profile` CLI option does not select this profile. It points to an
agent-definition file. Development-home isolation must always use `GROK_HOME`.

Do not pass the literal string `~/.grokdev` through an API that does not perform
shell expansion.

`GROK_HOME` affects configuration, authentication, sessions, memory, logs,
MCP credentials, marketplace data, installed Grok paths, and other persistent
runtime state. The application caches this path, so it must be set before the
process starts.

### Canonical Environment

Run development commands in a shell configured like this:

```sh
set -eu
umask 077

export GROK_HOME="${HOME}/.grokdev"
export GROK_LEADER_SOCKET="${GROK_HOME}/leader.sock"
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
chmod 0700 "${GROK_HOME}"
```

This environment prevents the development console from sharing the normal
Grok profile, leader socket, auto-updater, or compatibility resources belonging
to Cursor, Claude, or Codex.

Compatibility integration tests may explicitly enable the exact compatibility
surface they exercise. They must use controlled fixtures and must not inspect
or modify real external-agent state.

### Required Preflight

Before launching repository code, verify:

```sh
test "${GROK_HOME}" = "${HOME}/.grokdev"
test "${GROK_LEADER_SOCKET}" = "${HOME}/.grokdev/leader.sock"
test -d "${GROK_HOME}"
```

If any check fails, stop. Do not fall back to `~/.grok`.

### Forbidden Profiles and Artifacts

Development must not access or mutate:

- `~/.grok`;
- `~/.grok-prod`;
- another user's Grok home;
- `/opt/grok-custom/grok`;
- the installed `grok` executable;
- the `grok-custom` deployment wrapper.

Do not run `make deploy`, `make deploy-binary`, or `make deploy-wrapper` during
normal development. Those targets install signed artifacts outside the
repository and are reserved for explicit deployment work.

Do not copy authentication or configuration from another profile. Authenticate
the development profile independently when necessary.

## Development Commands

Every example below must run in the canonical environment from the previous
section, in the same shell invocation.

### Build and Check

```sh
cargo check -p xai-grok-pager-bin
cargo build -p xai-grok-pager-bin
cargo build -p xai-grok-pager-bin --release
```

The regular release artifact is:

```text
target/release/xai-grok-pager
```

The repository `Makefile` defaults to the `release-dist` profile and is mainly
for signed custom deployment. Prefer direct, crate-targeted Cargo commands for
development.

### Run the TUI

Use the newly built source checkout and disable leader reuse:

```sh
cargo run -p xai-grok-pager-bin -- --no-leader --no-auto-update
```

For inline terminal output during diagnostics:

```sh
cargo run -p xai-grok-pager-bin -- \
  --no-leader \
  --no-auto-update \
  --no-alt-screen
```

Use the alternate-screen TUI when testing full-screen rendering. Use
`--no-alt-screen` only when persistent diagnostic output is more useful.

### Run Headless

```sh
cargo run -p xai-grok-pager-bin -- \
  --no-leader \
  --no-auto-update \
  -p "Describe the current repository."
```

Headless calls can contact configured model services and consume credentials or
quota from the development profile. Do not run them unless that external action
is relevant to the task.

### Test and Lint

The repository uses `cargo-nextest` as the fast local default. `cargo test`
remains fully supported (`cargo test` ignores `.config/nextest.toml`; `cargo
nextest` honors it), and both runners use the same isolated `~/.grokdev`
profile.

```sh
# Default: one affected crate with nextest.
./grok-test.sh -p xai-grok-config
./grok-test.sh -p xai-grok-shell

# Run a specific integration-test family or test function. Nextest filters use
# the test module path, so the family module prefix narrows scheduling.
./grok-test.sh -p xai-grok-shell -- session_runtime_family
./grok-test.sh -p xai-grok-shell -- session_runtime_family::test_fork_session
./grok-test.sh -p xai-grok-shell -- session_runtime_family::test_fork_session::test_fork_session_creates_new_session_with_parent_tracking

# Include ignored acceptance / perf tests when you have the needed fixture or
# pre-built binary.
./grok-test.sh -p xai-grok-shell --run-ignored all

# Cargo-test compatibility and quick lints.
cargo test -p xai-grok-shell
cargo clippy -p xai-grok-shell
cargo fmt --all --check
```

`./grok-test.sh` is a thin wrapper that:
* sets the canonical development profile;
* disables discovery of Cursor / Claude / Codex state;
* enables `sccache` when present;
* invokes `cargo nextest run` with any trailing arguments.

For a no-build metadata check or to list matching tests without running them:

```sh
cargo nextest list -p xai-grok-shell
cargo nextest run -p xai-grok-shell --no-run
```

Prefer the narrowest affected crate, then validate direct consumers. Full
workspace builds and tests are expensive.

Install DotSlash before commands that require `bin/protoc`:

```sh
cargo install dotslash
/usr/bin/env dotslash --help
```

### Measuring Test Compile/Link Cost

When changing the test layout, measure the impact before adding linker
replacements. Rust link times are usually the bottleneck for multi-binary
integration-test suites.

```sh
# cargo-test: compile every test target without executing.
cargo test -p xai-grok-shell --no-run --timings

# nextest: build and resolve test metadata without executing tests.
./grok-test.sh -p xai-grok-shell --no-run

# Inspect shared-cache hit rate for the current shell.
sccache --show-stats
```

The repository-wide `.config/nextest.toml` keeps timeouts conservative
(`retries = 0`, `slow-timeout` enforces a 300s per-test ceiling) and reports
slow tests; it does not blanket-retry failures.

## Testing Strategy

- Add focused unit tests beside changed code.
- Use crate-level integration tests for cross-module or process behavior.
- Use PTY tests for terminal lifecycle, input, rendering, reconnect, and
  full-screen behavior.
- Use snapshots for stable visual output.
- Test both success and failure paths for permissions, sandboxing, MCP,
  persistence, reconnection, and recovery.
- Avoid tests that depend on a developer's existing configuration, credentials,
  sessions, shell aliases, or running leader process.
- Use temporary directories for test-specific mutable state whenever possible.
  The persistent `~/.grokdev` profile is a safety boundary, not a substitute
  for hermetic fixtures.

## Change Guidelines

- Use the pinned Rust toolchain and repository formatting configuration.
- Follow standard Rust conventions: four-space indentation, `snake_case`
  functions and modules, `CamelCase` types and traits, and
  `SCREAMING_SNAKE_CASE` constants.
- Keep crate boundaries purposeful.
- Check downstream impact before changing common protocol or runtime crates.
- Preserve user-visible behavior unless the task intentionally changes it.
- Keep unrelated working-tree changes intact.
- Never weaken fail-closed managed configuration, permission gates, sandbox
  restrictions, credential protection, or path containment accidentally.

## Important Starting Points

- `crates/codegen/xai-grok-pager-bin/src/main.rs`: executable composition root.
- `crates/codegen/xai-grok-pager/src/app`: TUI architecture and event loop.
- `crates/codegen/xai-grok-shell/src/agent/app.rs`: agent mode entry points.
- `crates/codegen/xai-grok-shell/src/session`: session runtime.
- `crates/codegen/xai-grok-tools/src/registry`: tool registration.
- `crates/codegen/xai-grok-workspace/src/permission`: permission system.
- `crates/codegen/xai-grok-sandbox/src/lib.rs`: sandbox manager.
- `crates/codegen/xai-grok-config/src/paths.rs`: `GROK_HOME` behavior.
- `crates/common/xai-tool-runtime/src/tool.rs`: tool abstraction.
- `crates/codegen/xai-grok-pager/docs/user-guide`: user documentation.

## Definition of Done

A change is ready for handoff when:

1. the relevant architecture and repository conventions were respected;
2. all authored content uses `en-US`;
3. all commands ran with `GROK_HOME=${HOME}/.grokdev`;
4. no production profile, installed binary, deployed wrapper, or production
   leader socket was used;
5. focused tests pass;
6. formatting and applicable lints pass;
7. security-sensitive changes include failure-path coverage;
8. the handoff lists the exact validation commands and their results.
