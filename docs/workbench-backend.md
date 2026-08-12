# Workbench external ACP backend (Mode C)

Selectable Agent Communication Protocol (ACP) backend for the Grok Build pager.
Default behavior is unchanged: the pager uses the in-process GrokShell agent
(**Mode A**). This document covers the optional **Mode C** path, where the TUI
stops owning agent orchestration and becomes an ACP client of Workbench.

```text
AgentBackend
|-- GrokShellBackend        # existing spawn_grok_shell (default / Mode A)
`-- WorkbenchBackend        # Mode C: launches workbench agent stdio
```

This is the fork-side counterpart of the monorepo crate
`workbench-terminal-backend` (Feature 016). Rendering and widgets are not
modified. Upstream touch surface stays small (≤8 files).

## Operating modes (product)

| Mode | Who owns the TUI | Who owns orchestration | When to use |
|------|------------------|------------------------|-------------|
| **A** | Grok pager | In-process GrokShell | Solo multi-model Grok session (`/providers`, subagents, etc.) |
| **B** | Workbench CLI / VS Code | Workbench daemon | Cross-runtime workflows (Claude Code, Codex, Grok, OpenRouter) without Grok TUI |
| **C** | Grok pager (this backend) | Workbench daemon via `agent stdio` | Same Grok TUI UX, but prompts and tools route through the daemon |

Mode C path:

```text
Grok TUI (pager)
  --ACP/stdio-->  workbench agent stdio
  --local NDJSON-->  workbench daemon
  --> Claude Code / Codex / Grok / OpenRouter / MCP (policy + ledger)
```

Orchestration, provider routing, spend ledger, and fail-closed write policy stay
in the daemon. The pager does not reimplement them.

## Enable Mode C

All of the following are required:

1. Select Workbench backend:
   - `WORKBENCH_TERMINAL_BACKEND=1` (or `true` / `yes`), **or**
   - `GROK_AGENT_BACKEND=workbench`
2. Absolute path to the Workbench CLI:
   - `WORKBENCH_EXECUTABLE=/absolute/path/to/workbench`, **or**
   - `--workbench-executable /absolute/path/to/workbench`

Relative paths and paths containing `..` fail closed.

Example (isolation profile + Mode C):

```sh
export GROK_HOME="${HOME}/.grokdev"
export GROK_LEADER_SOCKET="${GROK_HOME}/leader.sock"
export GROK_DISABLE_AUTOUPDATER=1
export GROK_CURSOR_SKILLS_ENABLED=0
# …other GROK_*_ENABLED=0 flags per AGENTS.md when loading resources

mkdir -p "$GROK_HOME" && chmod 0700 "$GROK_HOME"

export WORKBENCH_TERMINAL_BACKEND=1
export WORKBENCH_EXECUTABLE="/absolute/path/to/workbench"

# Workbench daemon must already be running for the workspace
cd /path/to/workspace
cargo run -p xai-grok-pager-bin -- --no-leader --no-auto-update
```

Or with a built binary:

```sh
GROK_AGENT_BACKEND=workbench \
  WORKBENCH_EXECUTABLE=/usr/local/bin/workbench \
  grok --no-leader
```

Leader mode is forced off when Workbench backend is selected (the pager speaks
ACP only to `workbench agent stdio`).

## Child process contract

| Field | Value |
|-------|-------|
| Program | absolute `WORKBENCH_EXECUTABLE` |
| Argv | `agent` `stdio` |
| cwd | workspace root (process current directory) |
| Env | `WORKBENCH_TERMINAL_BACKEND=1` |

Matches monorepo `workbench-terminal-backend::WorkbenchBackend::command()`.

## Development / tests

Offline unit tests (no Workbench binary required):

```sh
export GROK_HOME="${HOME}/.grokdev"
export GROK_LEADER_SOCKET="${GROK_HOME}/leader.sock"
export GROK_DISABLE_AUTOUPDATER=1
export GROK_CURSOR_SKILLS_ENABLED=0
mkdir -p "$GROK_HOME" && chmod 0700 "$GROK_HOME"

cargo test -p xai-grok-pager --lib workbench_backend
cargo check -p xai-grok-pager-bin
```

Default GrokShell path is unchanged when selection env vars are unset.

Optional live smoke (ignored by default):

```sh
WORKBENCH_LIVE_TEST=1 \
  WORKBENCH_EXECUTABLE=/absolute/path/to/workbench \
  cargo test -p xai-grok-pager live_spawn_workbench -- --ignored --nocapture
```

## Compatibility pin (monorepo)

After this branch is verified, publish the fork commit SHA into the Workbench
monorepo:

```text
GROK_BUILD_FORK_COMPATIBILITY_PIN = "<this-commit-sha>"
```

in `crates/workbench-terminal-backend/src/lib.rs`.

## Residual

Full dual-upstream rebase automation, PTY snapshot suite expansion, and pin
publication workflow remain follow-ups. This change ships the selectable
backend, offline argv/env validation tests, and Mode C documentation without
changing default Grok pager behavior.
