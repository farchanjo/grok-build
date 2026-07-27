# Grok Build — Claude Agent Compatibility Entry Point

This file is a concise compatibility entry point for Claude-compatible agents
working in the Grok Build repository. It repeats only safety-critical rules and
high-level workflow. The full bindings are:

- [`AGENTS.md`](AGENTS.md) — mandatory operational rules.
- [`GROK.md`](GROK.md) — canonical architecture and local development guide.

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

- Direct crate-targeted development commands (normal daily work):

  ```sh
  cargo check -p xai-grok-pager-bin
  cargo test -p xai-grok-config
  cargo clippy -p xai-grok-shell
  cargo fmt --all --check
  cargo build -p xai-grok-pager-bin --release
  ```

- Fast local test runner with `cargo-nextest` (canonical name; not
  "rustnext"):

  ```sh
  ./grok-test.sh -p xai-grok-shell
  ./grok-test.sh -p xai-grok-shell -- session_runtime_family
  ./grok-test.sh -p xai-grok-shell --run-ignored all
  ./grok-test.sh -p xai-grok-shell --no-run
  ```

  `cargo test -p xai-grok-shell` remains supported; `cargo test` ignores
  `.config/nextest.toml`.

- Experimental `claude-cli-runtime` run with isolated `./target-dev` artifacts:

  ```sh
  ./grok-dev-runner.sh --no-leader --no-auto-update
  ```

- Release-dist/deployment builds (not for normal development):

  ```sh
  make build
  make build FEATURES=claude-cli-runtime
  ```

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
- Use separate `CARGO_TARGET_DIR` for independent runners or worktrees; do not
  delete transient `.cargo` file locks.

## Tooling notes

- Root `Cargo.toml` is generated and read-only; edit per-crate manifests.
- sccache and Linux `lld` are already configured in `.cargo/config.toml`. Do
  not add `mold` or custom linker flags casually.
- `cargo-nextest` runs each test in its own process; `cargo test` does not.
  Nextest is configured with `retries = 0`; do not add blanket retries.
- Passing `--no-capture` to installed `cargo-nextest` serializes execution; use
  it only for short diagnostics.
