# Grok Build — Claude Agent Compatibility Entry Point

This file is a concise compatibility entry point for Claude-compatible agents
working in the Grok Build repository. It repeats safety-critical rules and a
condensed rapid-development workflow only. It is **not** a substitute for the
full references:

- [`AGENTS.md`](AGENTS.md) — enforceable operations policy, including the
  mandatory rapid development checklist.
- [`GROK.md`](GROK.md) — detailed canonical architecture and local
  development guide.

Read `AGENTS.md` and `GROK.md` in full before editing code, proposing changes,
or running repository commands.

## Safety-critical rules

- Write all authored content in United States English (`en-US`).
- Development must use only the isolated development profile:

  ```sh
  export GROK_HOME="${HOME}/.grokdev"
  export GROK_LEADER_SOCKET="${GROK_HOME}/leader.sock"
  export GROK_DISABLE_AUTOUPDATER=1
  ```

  The `--agent-profile` CLI option selects an agent-definition file; it is not
  a substitute for `GROK_HOME`.
- Never use, read, write, migrate, copy, or delete development data in
  `~/.grok`, `~/.grok-prod`, or any other production profile.
- Never run the installed `grok` command, `/opt/grok-custom/grok`, or the
  `grok-custom` wrapper to validate repository changes.
- Do not run `make deploy`, `make deploy-binary`, `make deploy-wrapper`, or
  `make verify` unless the task explicitly requests deployment.
- Always pass `--no-leader` and `--no-auto-update` when running the freshly
  built console for local verification.

## Quick build/test workflow

For ordinary development, **prefer the helpers** and run **one at a time**.
They enforce the isolated `~/.grokdev` profile and share one worktree-local
`./target-dev` directory. Waiting on Cargo's shared target lock is expected and
smaller than recompiling the same graph in a second directory.

- Focused check first (for example, the crate you are editing):

  ```sh
  ./grok-check.sh -p xai-grok-shell
  ./grok-check.sh -p xai-grok-pager-bin
  ```

- Fast local test runner with `cargo-nextest` (canonical name; not
  "rustnext"):

  ```sh
  ./grok-test.sh -p xai-grok-shell
  ./grok-test.sh -p xai-grok-shell -- session_runtime_family
  ./grok-test.sh -p xai-grok-shell --run-ignored all
  ./grok-test.sh -p xai-grok-shell --no-run
  ```

  `cargo test` remains supported for shared-process semantics; it ignores
  `.config/nextest.toml`. Keep nextest as the fast default.

- Direct `cargo` commands only when a helper cannot express the operation.
  From Bash, `source ./grok-dev-env.sh` first so direct commands share
  `./target-dev` and the isolated profile:

  ```sh
  source ./grok-dev-env.sh
  cargo clippy -p xai-grok-shell
  cargo fmt --all --check
  cargo test -p xai-grok-config
  ```

- Experimental `claude-cli-runtime` run (only when needed):

  ```sh
  ./grok-dev-runner.sh --no-leader --no-auto-update
  ```

- Isolated parallel experiment with a separate target directory:

  ```sh
  CARGO_TARGET_DIR=/tmp/grok-local-target ./grok-check.sh -p xai-grok-shell
  ```

- Release-dist/deployment builds (not for ordinary edit loops; require
  explicit user request):

  ```sh
  make build
  make build FEATURES=claude-cli-runtime
  ```

Start narrow (affected package or test), validate direct consumers, and expand
to workspace or release-dist only in proportion to risk. Do not delete Cargo
file locks or kill another build to bypass waiting.

## Mandatory checklist

Before every normal edit loop, consult the checklist in `AGENTS.md`:
`GROK_HOME=${HOME}/.grokdev`, prefer `./grok-check.sh`/`./grok-test.sh`,
source `./grok-dev-env.sh` before direct `cargo` commands, run one helper at a
time, start narrow, keep `make build` for deliberate release validation, and
preserve `Cargo.lock` discipline.

## Cargo.lock discipline

`Cargo.lock` is tracked and must remain deterministic.

- Do not modify `Cargo.lock` for source-only changes.
- Use `--locked` for reproducible builds and release verification.
- For intentional dependency changes, regenerate with the pinned toolchain in
  the isolated environment, review the diff, and include it.
- Never hand-edit `Cargo.lock`.
- Use precise updates (`cargo update -p <name> --precise <version>`), not broad
  `cargo update`.
- If `--locked` fails, diagnose the manifest/lock mismatch instead of removing
  `--locked` or deleting the lockfile.
- Normal development helpers (`grok-check.sh`, `grok-test.sh`,
  `grok-dev-runner.sh`) share `./target-dev` for artifact reuse. Run one at a
  time; use a separate `CARGO_TARGET_DIR` for isolated experiments or other
  worktrees; do not delete transient `.cargo` file locks.

## Version bumps (fcustom)

- The CLI version is lockstepped across exactly four manifests:
  `xai-grok-pager`, `xai-grok-shell`, `xai-grok-pager-bin`,
  `xai-grok-version` (current fcustom base: `1.0.0`).
- Bumps use **plain semver, no pre-release suffix** (a suffix makes the
  stable-channel auto-updater overwrite the custom build) and stay ahead of
  the upstream stable feed so the fork is never auto-replaced.
- Regenerate `Cargo.lock` in the isolated environment (direct cargo command;
  the diff must be only the four member lines) and commit it with the
  manifests. Finish with explicit `make deploy` + `make verify`; optional
  local tag `v<major.minor.patch>-fcustom`.
- Full procedure and version-flow details:
  [`GROK.md`](GROK.md#version-identity-and-bumps-fcustom).

## Tooling notes

- Root `Cargo.toml` is generated and read-only; edit per-crate manifests.
- sccache and Linux `lld` are already configured in `.cargo/config.toml`. Do
  not add `mold` or custom linker flags casually.
- `cargo-nextest` runs each test in its own process; `cargo test` does not.
  Nextest is configured with `retries = 0`; do not add blanket retries.
- Passing `--no-capture` to installed `cargo-nextest` serializes execution; use
  it only for short diagnostics.
