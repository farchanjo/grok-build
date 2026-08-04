//! Build script for the standalone media-decode fuzz harness (plan PR 10).
//!
//! Mirrors `xai-grok-tools/build.rs` FFmpeg discovery so the harness
//! compiles in both build situations:
//!
//! - Compatible FFmpeg 8 headers present (avutil 60 / avcodec 62 /
//!   avformat 62 / swscale 9 / swresample 6) and `GROK_DISABLE_MEDIA_FFMPEG`
//!   unset: the crate-local `media_ffmpeg` cfg is set and
//!   `fuzz_targets/decode.rs` fuzzes the native `DecodeSession` public API.
//! - Headers absent or the kill switch set: no cfg; the native arm compiles
//!   out and the target degrades to the always-on deserialization surface.
//!
//! Discovery uses `pkg_config::Config::cargo_metadata(false)` only, so no
//! FFmpeg link metadata is ever emitted — matching the tools crate
//! invariant. The cfg applies to this fuzz crate only; `xai-grok-tools`
//! enables its own `media_ffmpeg` through its own build script.

use std::env;

/// FFmpeg libraries required by the native shim: `(short name, pkg-config
/// module, expected ABI major)` for FFmpeg 8.x (plan section 10.2).
const FFMPEG_LIBS: [(&str, &str, u32); 5] = [
    ("libavutil", "libavutil", 60),
    ("libavcodec", "libavcodec", 62),
    ("libavformat", "libavformat", 62),
    ("libswscale", "libswscale", 9),
    ("libswresample", "libswresample", 6),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rustc-check-cfg=cfg(media_ffmpeg)");
    println!("cargo:rerun-if-env-changed=GROK_DISABLE_MEDIA_FFMPEG");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");

    // Kill switch: the tools crate compiles the native module out under the
    // same variable, so the harness must agree or its imports would dangle.
    if env::var_os("GROK_DISABLE_MEDIA_FFMPEG").is_some_and(|v| !v.is_empty()) {
        return Ok(());
    }

    // Platform gating mirrors xai-grok-tools/build.rs.
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if !(target_os == "macos" || target_os == "linux") || target_env == "musl" {
        return Ok(());
    }

    for (_name, module, expected_major) in FFMPEG_LIBS {
        let Ok(lib) = pkg_config::Config::new()
            .cargo_metadata(false)
            .probe(module)
        else {
            return Ok(());
        };
        let Some(major) = lib.version.split('.').next().and_then(|v| v.parse::<u32>().ok())
        else {
            return Ok(());
        };
        if major != expected_major {
            return Ok(());
        }
    }

    println!("cargo:rustc-cfg=media_ffmpeg");
    Ok(())
}
