//! Ripgrep binary resolution.
//!
//! `xai-grok-tools` owns the bundled-rg decision: its build script stages the
//! binary into `OUT_DIR`, emits `cargo:rustc-cfg=bundle_rg`, and the crate then
//! extracts it to `<grok_home>/vendor/` on first use. A crate that only
//! *depends* on the tools crate cannot see those build-script emissions, so a
//! second copy of the resolution logic here could never honor the bundle.
//!
//! Delegating keeps a bundled release resolving one binary for every grep path
//! (tool grep, codex grep, content search) instead of silently falling back to
//! `rg` on `PATH` for the ones that live in this crate.
pub use xai_grok_tools::implementations::grok_build::grep::ripgrep::rg_path;
