//! Build script for bundling ripgrep for the xai-grok-tools crate.
//!
//! - If `GROK_TOOLS_BUNDLE_RG_PATH` is set, always bundle it
//! - Otherwise, only bundle in release builds
use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

const RG_VER: &str = "15.0.0";
const BFS_VER: &str = "4.1";
const UGREP_VER: &str = "7.7.0";

/// FFmpeg libraries required by the native media-understanding shim
/// (plan section 10.2): the tuple is `(short name, pkg-config module,
/// expected ABI major)` for FFmpeg 8.x.
///
/// | Library         | Major |
/// |-----------------|-------|
/// | `libavutil`     | 60    |
/// | `libavcodec`    | 62    |
/// | `libavformat`   | 62    |
/// | `libswscale`    | 9     |
/// | `libswresample` | 6     |
const FFMPEG_LIBS: [(&str, &str, u32); 5] = [
    ("libavutil", "libavutil", 60),
    ("libavcodec", "libavcodec", 62),
    ("libavformat", "libavformat", 62),
    ("libswscale", "libswscale", 9),
    ("libswresample", "libswresample", 6),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    bundle_rg()?;
    // bfs/ugrep back the bash-harness find/grep shadows (embedded_search_tools).
    bundle_search_tool("bfs", "BFS", BFS_VER)?;
    bundle_search_tool("ugrep", "UGREP", UGREP_VER)?;
    // PR 4: native FFmpeg 8 preprocessing layer. Compiles out cleanly with a
    // clear diagnostic when compatible headers are absent or disabled.
    media_ffmpeg()?;
    Ok(())
}

/// Discover FFmpeg 8 headers and compile the Grok-owned C shim, or compile
/// the native backend out with a clear diagnostic.
///
/// Invariants (plan sections 10.1-10.2):
/// - Discovery uses `pkg_config::Config::cargo_metadata(false)` ONLY, so no
///   FFmpeg link directives (no `rustc-link-lib` / `rustc-link-search`
///   metadata) are ever emitted. The Grok binary has no link-time FFmpeg
///   dependency; the shim calls FFmpeg exclusively through an immutable
///   per-context function-pointer table supplied by Rust via `libloading`
///   at runtime.
/// - `GROK_DISABLE_MEDIA_FFMPEG` forces the feature off.
/// - Missing or incompatible headers are NEVER a hard build failure; the
///   backend compiles out with a `cargo:warning` diagnostic so release
///   builders without FFmpeg 8 headers still produce a working binary.
/// - Platform gating: only `macos`/`linux` (non-musl) targets can `dlopen`
///   user-installed libraries; everything else compiles the backend out.
fn media_ffmpeg() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rustc-check-cfg=cfg(media_ffmpeg)");
    println!("cargo:rerun-if-env-changed=GROK_DISABLE_MEDIA_FFMPEG");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");
    println!("cargo:rerun-if-changed=ffmpeg/grok_av.h");
    println!("cargo:rerun-if-changed=ffmpeg/grok_av.c");

    if env::var_os("GROK_DISABLE_MEDIA_FFMPEG").is_some_and(|v| !v.is_empty()) {
        println!(
            "cargo:warning=media_ffmpeg: GROK_DISABLE_MEDIA_FFMPEG is set; the native FFmpeg \
             preprocessing backend is compiled out."
        );
        return Ok(());
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if !(target_os == "macos" || target_os == "linux") || target_env == "musl" {
        println!(
            "cargo:warning=media_ffmpeg: target {target_os}/{target_env} does not support \
             runtime library loading; the native FFmpeg preprocessing backend is compiled out."
        );
        return Ok(());
    }

    // Discover headers/versions only. `cargo_metadata(false)` prevents any
    // `cargo:rustc-link-*` output: pkg-config is used for include dirs and
    // version validation alone.
    let mut found: Vec<(&str, &str, pkg_config::Library)> = Vec::new();
    for (name, module, expected_major) in FFMPEG_LIBS {
        match pkg_config::Config::new()
            .cargo_metadata(false)
            .probe(module)
        {
            Ok(lib) => {
                let major = lib
                    .version
                    .split('.')
                    .next()
                    .and_then(|v| v.parse::<u32>().ok());
                match major {
                    Some(m) if m == expected_major => {
                        found.push((name, module, lib));
                    }
                    Some(m) => {
                        println!(
                            "cargo:warning=media_ffmpeg: {module} headers found at version {} \
                             (major {m}); expected major {expected_major} for FFmpeg 8. The \
                             native preprocessing backend is compiled out.",
                            lib.version
                        );
                        return Ok(());
                    }
                    None => {
                        println!(
                            "cargo:warning=media_ffmpeg: {module} headers found but their version \
                             '{}' could not be parsed; the native preprocessing backend is \
                             compiled out.",
                            lib.version
                        );
                        return Ok(());
                    }
                }
            }
            Err(e) => {
                println!(
                    "cargo:warning=media_ffmpeg: {module} headers not available via pkg-config \
                     ({e}); the native FFmpeg preprocessing backend is compiled out. Install \
                     FFmpeg 8 development headers to enable in-process media preprocessing."
                );
                return Ok(());
            }
        }
    }

    // Union of include dirs; per-library link dirs go to runtime env vars so
    // the Rust loader prefers the exact libraries the headers matched.
    let mut include_dirs: Vec<PathBuf> = Vec::new();
    for (name, _module, lib) in &found {
        for dir in &lib.include_paths {
            if !include_dirs.contains(dir) {
                include_dirs.push(dir.clone());
            }
        }
        for dir in &lib.link_paths {
            println!(
                "cargo:rustc-env=GROK_FFMPEG_{}_LIBDIR={}",
                name.to_uppercase(),
                dir.display()
            );
        }
    }

    // Compile the static Grok-owned shim. This is the ONLY link metadata we
    // produce, and it is for our own object file (`libgrok_av.a`) — never for
    // FFmpeg.
    let mut build = cc::Build::new();
    build.file("ffmpeg/grok_av.c").flag("-std=c11");
    for dir in &include_dirs {
        build.include(dir);
    }
    build.compile("grok_av");

    println!("cargo:rustc-cfg=media_ffmpeg");
    Ok(())
}

/// Bundle a prebuilt **static** search-tool binary (`bfs`/`ugrep`) when
/// `GROK_TOOLS_BUNDLE_<NAME>_PATH` points at one (supplied by the release
/// pipeline). Emits
/// `cfg(bundle_<name>)` so the crate's `include_bytes!` + self-extract engages.
///
/// No auto-download (unlike ripgrep): bfs/ugrep publish no prebuilt static
/// release assets, so the release pipeline supplies the path. Unset → not
/// bundled (the runtime resolver falls back to `~/.grok/vendor` / `$PATH`);
/// never a hard failure, so an un-wired build still succeeds.
fn bundle_search_tool(
    name: &str,
    name_uc: &str,
    ver: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let override_env = format!("GROK_TOOLS_BUNDLE_{name_uc}_PATH");
    println!("cargo:rerun-if-env-changed={override_env}");
    // Always declare the cfg so `#[cfg(bundle_<name>)]` is lint-clean when unset.
    println!("cargo:rustc-check-cfg=cfg(bundle_{name})");

    // The consumer (`embedded_search_tools`) is `#[cfg(unix)]`, so embedding on a
    // Windows target is dead weight — skip (mirrors the ripgrep Windows skip).
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        return Ok(());
    }

    let Some(src) = env::var(&override_env).ok().filter(|s| !s.is_empty()) else {
        return Ok(());
    };

    let gen_dir = PathBuf::from(env::var("OUT_DIR")?).join(format!("bundle-{name}"));
    fs::create_dir_all(&gen_dir)?;
    let dest = gen_dir.join(format!("{name}-{ver}-override.bin"));
    let _ = fs::remove_file(&dest);
    fs::copy(&src, &dest)
        .map_err(|e| format!("copy {override_env} from {src} to {}: {e}", dest.display()))?;

    println!("cargo:rustc-cfg=bundle_{name}");
    println!("cargo:rustc-env=GROK_TOOLS_{name_uc}_VER={ver}");
    println!("cargo:rustc-env=GROK_TOOLS_{name_uc}_TARGET=override");
    Ok(())
}

/// Download + embed ripgrep. Unchanged behavior; split out of `main` so the new
/// search-tool bundling runs regardless of ripgrep's early returns.
fn bundle_rg() -> Result<(), Box<dyn std::error::Error>> {
    // Only bundle in release builds to avoid slowing down cargo check.
    println!("cargo:rerun-if-env-changed=GROK_TOOLS_BUNDLE_RG_PATH");
    // Declare our custom cfg to the compiler so cfg(bundle_rg) is recognized by lints
    println!("cargo:rustc-check-cfg=cfg(bundle_rg)");

    let gen_dir = PathBuf::from(env::var("OUT_DIR")?).join("bundle-rg");
    fs::create_dir_all(&gen_dir)?;

    // Decide whether to bundle: path override OR release build
    let path_override = env::var("GROK_TOOLS_BUNDLE_RG_PATH").ok();
    let is_release = env::var("PROFILE").as_deref() == Ok("release");
    if path_override.is_none() && !is_release {
        return Ok(());
    }

    // Skip auto-bundling on Windows: ripgrep ships .zip on Windows (not
    // .tar.gz) and we have no zip-extraction path. Returning here BEFORE
    // emitting `cargo:rustc-cfg=bundle_rg` keeps include_bytes! macros gated
    // on cfg(bundle_rg) compiled-out, so the runtime falls back to `rg` on
    // PATH. Users install ripgrep separately (winget / scoop). An explicit
    // GROK_TOOLS_BUNDLE_RG_PATH still bundles regardless of target.
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" && path_override.is_none() {
        return Ok(());
    }

    // Expose cfg so the crate can include the bundled bytes.
    println!("cargo:rustc-cfg=bundle_rg");
    println!("cargo:rustc-env=GROK_TOOLS_RG_VER={}", RG_VER);

    // If a local rg binary is provided, copy it directly (skips target check).
    if let Some(path) = path_override {
        let dest = gen_dir.join(format!("rg-{}-override.bin", RG_VER));
        println!("cargo:rustc-env=GROK_TOOLS_RG_TARGET=override");
        let _ = fs::remove_file(&dest);
        fs::copy(PathBuf::from(path.clone()), &dest).map_err(|e| {
            format!(
                "Failed copying GROK_TOOLS_BUNDLE_RG_PATH: {e} from path {path} to dest {}",
                dest.display()
            )
        })?;
        return Ok(());
    }

    // Determine supported ripgrep asset triple for auto-download.
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let asset_triple = match (target_os.as_str(), target_arch.as_str()) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "x86_64") => "x86_64-unknown-linux-musl",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        _ => {
            return Err(format!(
                "Unsupported target for ripgrep bundling: {os}-{arch}. Set GROK_TOOLS_BUNDLE_RG_PATH to a local rg binary for offline or unsupported builds.",
                os = target_os,
                arch = target_arch
            ).into());
        }
    };

    println!("cargo:rustc-env=GROK_TOOLS_RG_TARGET={}", asset_triple);
    let dest = gen_dir.join(format!("rg-{}-{}.bin", RG_VER, asset_triple));
    let _ = fs::remove_file(&dest);

    let url = format!(
        "https://github.com/BurntSushi/ripgrep/releases/download/{v}/ripgrep-{v}-{t}.tar.gz",
        v = RG_VER,
        t = asset_triple
    );

    let bytes: Vec<u8> = {
        let resp = reqwest::blocking::get(&url).map_err(|e| {
            format!(
                "Failed to download ripgrep: {}\nSet GROK_TOOLS_BUNDLE_RG_PATH to a local rg for offline builds.",
                e
            )
        })?;
        if !resp.status().is_success() {
            return Err(format!(
                "HTTP {} downloading ripgrep. Set GROK_TOOLS_BUNDLE_RG_PATH for offline builds.",
                resp.status()
            )
            .into());
        }
        resp.bytes()?.to_vec()
    };

    let gz = flate2::read::GzDecoder::new(&bytes[..]);
    let mut ar = tar::Archive::new(gz);
    let mut found = false;
    for entry in ar.entries()? {
        let mut e = entry?;
        let p = e.path()?;
        if p.file_name().is_some_and(|n| n == "rg") {
            let data: Vec<u8> = {
                let mut v = Vec::new();
                io::copy(&mut e, &mut v)?;
                v
            };
            fs::write(&dest, &data)?;
            found = true;
            break;
        }
    }

    if !found {
        return Err(format!(
            "Could not find 'rg' in ripgrep archive {}. Set GROK_TOOLS_BUNDLE_RG_PATH for offline builds.",
            url
        )
        .into());
    }

    Ok(())
}
