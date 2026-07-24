# Grok Build Repository Instructions

These instructions apply to the entire repository. They are mandatory for every
agent, automation, and development session that reads this file.

## Required Reference

Read [`GROK.md`](GROK.md) in full before analyzing code, proposing changes,
running commands, or editing files. `GROK.md` is the canonical development and
architecture reference for this checkout.

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
cargo run -p xai-grok-pager-bin -- --no-leader --no-auto-update
```

Use leader mode only when the behavior under test requires it. In that case,
keep `GROK_LEADER_SOCKET` under `~/.grokdev` and verify that every participating
process uses the same isolated environment.

Do not assume exports from a previous terminal command are still active. When
commands are executed in separate shells, export the development environment
again in the same command invocation.

## Build and Validation

Install DotSlash first because `bin/protoc` uses it for hermetic protobuf
tooling.

Prefer crate-targeted validation:

```sh
cargo check -p xai-grok-pager-bin
cargo test -p xai-grok-config
cargo clippy -p xai-grok-shell
cargo fmt --all --check
cargo build -p xai-grok-pager-bin --release
```

The examples assume the mandatory `~/.grokdev` environment has already been
set in the same shell invocation. The release artifact is
`target/release/xai-grok-pager`.

- Start with the narrowest affected crate.
- Check direct consumers after the focused tests pass.
- Use full-workspace validation only when the change crosses enough crate
  boundaries to justify its cost.
- TUI changes require relevant render, snapshot, scenario, or PTY-harness
  coverage.
- Permission, sandbox, session, leader, and MCP changes require failure-path
  tests in addition to happy-path tests.

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
