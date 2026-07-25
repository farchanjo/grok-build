# Workbench external ACP backend

Selectable Agent Communication Protocol (ACP) backend for the Grok Build pager.
Default behavior is unchanged: the pager uses the in-process GrokShell agent.

```text
AgentBackend
|-- GrokShellBackend        # existing spawn_grok_shell (default)
`-- WorkbenchBackend        # launches: workbench agent stdio
```

This is the fork-side counterpart of the monorepo crate
`workbench-terminal-backend` (Feature 016). Rendering and widgets are not
modified.

## Enable

All of the following are required:

1. Select Workbench backend:
   - `WORKBENCH_TERMINAL_BACKEND=1` (or `true` / `yes`), **or**
   - `GROK_AGENT_BACKEND=workbench`
2. Absolute path to the Workbench CLI:
   - `WORKBENCH_EXECUTABLE=/absolute/path/to/workbench`, **or**
   - `--workbench-executable /absolute/path/to/workbench`

Example:

```sh
export GROK_HOME="${HOME}/.grokdev"
export WORKBENCH_TERMINAL_BACKEND=1
export WORKBENCH_EXECUTABLE="/path/to/workbench"

# workbench daemon must already be running for the workspace
cd /path/to/workspace
grok --no-leader
```

Or:

```sh
GROK_AGENT_BACKEND=workbench \
  grok --workbench-executable /usr/local/bin/workbench --no-leader
```

Leader mode is forced off when Workbench backend is selected.

## Child process contract

| Field | Value |
|---|---|
| Program | absolute `WORKBENCH_EXECUTABLE` |
| Argv | `agent` `stdio` |
| cwd | workspace root (process current directory) |
| Env | `WORKBENCH_TERMINAL_BACKEND=1` |

Relative or `..`-containing executable paths fail closed.

## Development / tests

Offline unit tests (no binary required):

```sh
export GROK_HOME="${HOME}/.grokdev"
export GROK_LEADER_SOCKET="${GROK_HOME}/leader.sock"
export GROK_DISABLE_AUTOUPDATER=1

cargo test -p xai-grok-pager workbench_backend -- --nocapture
```

Optional live smoke (ignored by default):

```sh
WORKBENCH_LIVE_TEST=1 \
  WORKBENCH_EXECUTABLE=/path/to/workbench \
  cargo test -p xai-grok-pager live_spawn_workbench -- --ignored --nocapture
```

## Compatibility pin (monorepo)

After this branch is merged and verified, publish the fork commit SHA into the
Workbench monorepo:

```text
GROK_BUILD_FORK_COMPATIBILITY_PIN = "<this-commit-sha>"
```

in `crates/workbench-terminal-backend/src/lib.rs`.

## Residual

Full dual-upstream rebase automation, PTY snapshot suite expansion, and pin
publication workflow remain follow-ups. This change ships the selectable
backend, offline argv/env validation tests, and documentation without changing
default Grok pager behavior.
