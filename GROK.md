# Grok Build Development Guide

This document is the canonical technical and local-development reference for
this checkout. It explains the "why" and "how" behind the mandatory rules in
[`AGENTS.md`](AGENTS.md). Repository agents must read both files before making
changes.

- [`AGENTS.md`](AGENTS.md) is the enforceable operations policy: what agents
  must and must not do.
- [`GROK.md`](GROK.md) (this file) is the detailed canonical architecture and
  development guide.
- [`CLAUDE.md`](CLAUDE.md) is a concise compatibility entry point for
  Claude-compatible agents; it still requires reading `AGENTS.md` and
  `GROK.md` in full before editing.

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
set -euo pipefail
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

Do not run `make deploy`, `make deploy-binary`, `make deploy-wrapper`, or
`make verify` during normal development. Those targets install signed artifacts
outside the repository and are reserved for explicit deployment work.

Do not copy authentication or configuration from another profile. Authenticate
the development profile independently when necessary.

## Development Commands

Every example below must run in the canonical environment from the previous
section, in the same shell invocation.

### Rapid development flow

The normal edit loop is intentionally narrow and fast. Use the helper scripts
that share one worktree-local `./target-dev` directory:

1. **Fast focused check.** `./grok-check.sh -p <crate>` compiles the crate and
   its immediate dependencies under `./target-dev`. It is the fastest signal
   for type errors and is the first command after a small edit.
2. **Focused test.** `./grok-test.sh -p <crate>` runs the crate's tests through
   `cargo-nextest`, reusing the same `./target-dev` artifacts.
3. **Validate consumers.** After the affected crate passes, check and test the
   direct consumers that exercise the changed surface.
4. **Broader validation** only when the change crosses enough crate boundaries
   to justify the cost.
5. **Release-dist or `make build`** only as a deliberate final step, not inside
   the ordinary loop.

```sh
# 1. Affected crate.
./grok-check.sh -p xai-grok-shell
./grok-test.sh -p xai-grok-shell
./grok-test.sh -p xai-grok-shell -- session_runtime_family

# 2. Direct consumer. Check that the composition-root binary still compiles;
# run the consumer library's tests that exercise the changed surface.
./grok-check.sh -p xai-grok-pager-bin
./grok-test.sh -p xai-grok-pager

# 3. Deliberate broader or release validation (not the default loop).
source ./grok-dev-env.sh
cargo check --workspace --locked
./grok-test.sh -p xai-grok-shell --run-ignored all
cargo build -p xai-grok-pager-bin --release
```

### One helper at a time

Run **one** Cargo helper at a time. `./grok-check.sh`, `./grok-test.sh`, and
`./grok-dev-runner.sh` all use `./target-dev`. Cargo serializes access to that
directory with its own file lock, so a later command waits briefly while an
earlier one finishes. That wait is smaller than recompiling the same dependency
graph in a second directory.

Do **not** delete `.cargo` file locks or kill another build to bypass waiting.
If you need true parallelism, set a separate worktree-local or temporary
`CARGO_TARGET_DIR`:

```sh
CARGO_TARGET_DIR=/tmp/grok-local-target ./grok-check.sh -p xai-grok-shell
```

Each independent worktree gets its own root-local `./target-dev` by default.
Never point multiple active worktrees at the same target directory; that
breaks isolation and creates lock contention.

### Helper scripts

`./grok-check.sh` sources `./grok-dev-env.sh` and runs `cargo check --locked`
with whatever crate or extra flags you pass. Use it for fast focused
verification.

`./grok-test.sh` sources `./grok-dev-env.sh` and runs `cargo --locked nextest
run`. It applies the repository nextest configuration in
`.config/nextest.toml` (`retries = 0`, 300 s per-test ceiling, JUnit reports at
`target-dev/nextest/default/nextest-results.xml`).

`./grok-dev-runner.sh` sources `./grok-dev-env.sh` and runs the console with the
experimental `claude-cli-runtime` feature enabled. It stores artifacts in
`./target-dev` alongside the other helpers so dependency artifacts are reused
where possible. Targets affected by `claude-cli-runtime` are still rebuilt
because of the feature flag. Use it only when you need that feature; for normal
verification prefer `cargo run` with the canonical environment.

### Direct Cargo commands

Direct `cargo` commands are allowed only when a helper cannot express the
needed operation. Before running any direct `cargo check`, `cargo test`,
`cargo clippy`, `cargo fmt`, or similar command from Bash, `source
./grok-dev-env.sh` (or reproduce its exact environment) so the command:

* uses the isolated `~/.grokdev` profile;
* sets `CARGO_TARGET_DIR` to `./target-dev` (or honors your explicit override);
* disables Cursor / Claude / Codex state discovery;
* enables sccache when available.

```sh
source ./grok-dev-env.sh
cargo test -p xai-grok-config
cargo clippy -p xai-grok-shell
cargo fmt --all --check
```

### Test runners

`cargo-nextest` is the fast local default. Nextest runs each test as a separate
process and honors `.config/nextest.toml`. `cargo test` remains fully supported
for shared-process semantics and ignores `.config/nextest.toml`. Use nextest for
speed by default; use `cargo test` when the different process model matters.

```sh
# Fast default: one affected crate.
./grok-test.sh -p xai-grok-shell

# Filter to a test family module or single test. Nextest filters use the
# test module path, so the family module prefix narrows scheduling.
./grok-test.sh -p xai-grok-shell -- session_runtime_family
./grok-test.sh -p xai-grok-shell -- session_runtime_family::test_fork_session

# Include ignored acceptance / perf tests when you have the needed fixtures.
./grok-test.sh -p xai-grok-shell --run-ignored all

# List or dry-run without executing tests.
source ./grok-dev-env.sh
cargo nextest list -p xai-grok-shell
./grok-test.sh -p xai-grok-shell --no-run

# Cargo-test compatibility and quick lints.
source ./grok-dev-env.sh
cargo test -p xai-grok-shell
cargo clippy -p xai-grok-shell
cargo fmt --all --check
```

WARNING: Passing `--no-capture` to installed `cargo-nextest` disables output
capture and serializes test execution. Use it only for short diagnostic runs
where live output matters more than speed.

### Running the console

For ordinary verification, use the freshly built source checkout and disable
leader reuse:

```sh
source ./grok-dev-env.sh
cargo run -p xai-grok-pager-bin -- --no-leader --no-auto-update
```

For inline terminal output during diagnostics:

```sh
source ./grok-dev-env.sh
cargo run -p xai-grok-pager-bin -- \
  --no-leader \
  --no-auto-update \
  --no-alt-screen
```

Use the alternate-screen TUI when testing full-screen rendering. Use
`--no-alt-screen` only when persistent diagnostic output is more useful.

### Headless mode

```sh
source ./grok-dev-env.sh
cargo run -p xai-grok-pager-bin -- \
  --no-leader \
  --no-auto-update \
  -p "Describe the current repository."
```

Headless calls can contact configured model services and consume credentials or
quota from the development profile. Do not run them unless that external action
is relevant to the task.

### Makefile (release-dist and deployment only)

The repository `Makefile` is release-dist and deployment-oriented; it is not for
normal development. Its default `make build` target compiles `xai-grok-pager-bin`
with the `release-dist` profile, `--locked`, `--timings`, `CARGO_BUILD_JOBS`,
`sccache`/`RUSTC_WRAPPER`, and the optional `FEATURES` Cargo-feature variable.

```sh
make build
make build FEATURES=claude-cli-runtime
```

Do not run `make build`, `make deploy`, `make deploy-binary`,
`make deploy-wrapper`, or `make verify` in ordinary edit loops. Deployment and
verify targets require an explicit user request.

### Version identity and bumps (fcustom)

The user-visible version of the fork comes from exactly four **lockstep CLI
crates**, whose `version` fields must always stay identical:

- `crates/codegen/xai-grok-pager/Cargo.toml`
- `crates/codegen/xai-grok-shell/Cargo.toml`
- `crates/codegen/xai-grok-pager-bin/Cargo.toml`
- `crates/codegen/xai-grok-version/Cargo.toml`

The fcustom base is `1.0.0` (bumped from the upstream `0.2.110` base in
commit `807fc91`; local tag `v1.0.0-fcustom`). Other workspace crates keep
their own independent versions.

How the version reaches the binary:

1. The manifest version becomes `CARGO_PKG_VERSION` and
   `xai_grok_version::VERSION`. These feed `grok --version`, the welcome
   screen, the `x-grok-client-version` telemetry header, MCP client
   identification, and internal user-agent strings.
2. `xai-grok-pager/build.rs` bakes `VERSION_WITH_COMMIT` as
   `<version> (<short commit HEAD>)`, so each build embeds the exact commit.
3. A `GROK_VERSION` environment variable at build time overrides the manifest
   in both the pager build script and `xai-grok-version`. Use it only for
   one-off experimental builds; the manifest is the persistent source of
   truth.
4. The channel suffix (` [stable]` / ` [alpha]`) is derived at runtime by
   comparing the compiled version against the cached stable pointer in
   `$GROK_HOME/version.json` (written by the auto-updater). Because the
   fcustom base is ahead of the upstream feed, deployed builds display
   ` [alpha]`. This is expected and harmless.

Bump procedure (only on explicit release intent):

1. Choose **plain semver with no pre-release suffix** (for example `1.0.1`,
   `1.1.0`). A pre-release suffix such as `1.0.1-fcustom` makes the
   stable-channel updater force-install the feed's latest stable over the
   custom build (`needs_update` in `xai-grok-update` always returns true for
   pre-release currents on stable). Non-semver strings are rejected by Cargo
   in manifests and, if forced via `GROK_VERSION`, break
   `minimum_version` and `version_overrides` parsing.
2. Edit exactly the four lockstep manifests. No workspace dependency pins
   these crates by version (all requirements are path-based), so no other
   manifest changes are needed. Never edit the generated root `Cargo.toml`.
3. Regenerate `Cargo.lock` with a direct cargo command in the canonical
   isolated environment (the helpers pass `--locked` and refuse the change,
   so a direct command is the sanctioned path here). Review the diff: it must
   contain only the four member version lines. Commit the manifests and the
   lockfile together.
4. Rebuild and redeploy with `make deploy`, then `make verify` (explicit user
   request required, as for all deployment targets). Confirm the deployed
   binary reports the new version and the new commit hash.
5. Optionally mark the commit with a local annotated tag
   `v<major.minor.patch>-fcustom`. Tag pushes are blocked by the repository
   push policy (`fcustom -> fcustom` only); tags are local markers.

Expected side effects of a bump: the deployed binary shows ` [alpha]` while
ahead of the cached stable pointer, and any running leader from the previous
version is evicted on reconnect due to version skew (normal).

Version-selection invariant: keep the fcustom base **ahead of the upstream
stable feed** (currently `1.0.0 > 0.2.x`). Then `target > current` is false
for every feed release and the auto-updater never replaces a custom build
with a feed release.

### Cargo.lock discipline

`Cargo.lock` is tracked and must remain deterministic.

* Use `--locked` for reproducible CI/release verification and whenever dependency
  resolution must not change.
* Ordinary source-only changes should not modify `Cargo.lock`. If `git diff`
  shows lock changes after editing code only, the environment is out of sync;
  investigate before committing.
* When a dependency, feature, or manifest change intentionally alters resolution,
  regenerate the lockfile with the pinned toolchain in the canonical isolated
  environment, review the resulting diff, and include it in the change.
* Never hand-edit `Cargo.lock`.
* Do not run `cargo update` broadly. For a targeted update, use
  `cargo update -p <name> --precise <version>`.
* If `--locked` fails, investigate the manifest/lock mismatch rather than
  removing `--locked` or deleting `Cargo.lock`.

Distinguish the persistent `Cargo.lock` dependency lockfile from transient
file locks Cargo creates under `.cargo` and `target/`.

### Tooling policy

Do not add custom build serialization, new linker flags, `mold`, or
alternative caching without measurement and an explicit rationale. Cargo's own
file locks, the shared worktree-local `./target-dev`, and `sccache` are the
current policy. sccache and Linux `lld` are already configured in
`.cargo/config.toml`.

### Measuring compile/link cost

Install DotSlash before commands that require `bin/protoc`:

```sh
cargo install dotslash
/usr/bin/env dotslash --help
```

When changing the test layout, measure the impact before adding custom linker
replacements. Rust link times are usually the bottleneck for multi-binary
integration-test suites.

```sh
# cargo-test: compile every test target without executing.
source ./grok-dev-env.sh
cargo test -p xai-grok-shell --no-run --timings

# nextest: build and resolve test metadata without executing tests.
./grok-test.sh -p xai-grok-shell --no-run

# Inspect shared-cache hit rate.
sccache --show-stats
```

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
