# Repository Guidelines

## Project Structure & Module Organization

This is a Rust 2024 workspace containing the Grok Build terminal coding agent. The composition-root binary lives in `crates/codegen/xai-grok-pager-bin`; official releases expose it as `grok`. Core responsibilities are split across:

- `xai-grok-pager`: TUI, input handling, rendering, and user-facing commands.
- `xai-grok-shell`: agent runtime, sessions, headless/stdio modes, and orchestration.
- `xai-grok-tools`: built-in tool definitions and implementations.
- `xai-grok-workspace`: filesystem, VCS, execution, permissions, and checkpoints.
- `crates/common`: shared protocol and runtime libraries.
- `third_party`: vendored Mermaid rendering dependencies.

Tests are colocated in Rust modules or under each crate's `tests/` directory. User documentation is in `crates/codegen/xai-grok-pager/docs/user-guide/`. Treat the generated root `Cargo.toml` as read-only; edit crate manifests instead.

## Build, Test, and Development Commands

Install DotSlash first because `bin/protoc` uses it for hermetic protobuf tooling.

```sh
cargo run -p xai-grok-pager-bin
cargo check -p xai-grok-pager-bin
cargo build -p xai-grok-pager-bin --release
cargo test -p xai-grok-config
cargo clippy -p xai-grok-shell
cargo fmt --all
```

Prefer crate-targeted checks and tests; full-workspace builds are expensive. The release artifact is `target/release/xai-grok-pager`.

## Coding Style & Naming Conventions

Use standard Rust conventions: four-space indentation, `snake_case` functions/modules, `CamelCase` types/traits, and `SCREAMING_SNAKE_CASE` constants. Format with the pinned toolchain and `rustfmt.toml`; lint with Clippy using `clippy.toml` and workspace lint settings. Keep crate boundaries purposeful and avoid adding dependencies to foundational crates without checking downstream impact.

## Testing Guidelines

Add focused unit tests beside changed code and integration tests for cross-crate or process behavior. Name tests after observable behavior, such as `replays_session_after_reconnect`. Run the narrowest affected crate first, then check direct consumers. TUI changes should include the relevant render, snapshot, or PTY-harness coverage. No repository-wide numeric coverage threshold is documented.

## Commit & Pull Request Guidelines

The public history is primarily periodic commits titled `Synced from monorepo`; preserve that wording for mirror syncs. For upstream development, use concise imperative commit subjects and keep unrelated changes separate. Include motivation, affected crates, and exact validation commands in review descriptions; attach terminal screenshots or recordings for visible TUI changes.

This public repository does not accept external pull requests or unsolicited patches. Follow `CONTRIBUTING.md` and report vulnerabilities through `SECURITY.md`, not public issues.

## Security & Configuration

Never commit credentials, API keys, or files from `~/.grok/` such as `auth.json` or MCP credentials. Preserve sandbox, permission-gate, and managed-configuration fail-closed behavior unless the change explicitly targets those controls.
