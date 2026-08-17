//! Provider-neutral support for opt-in process-wide extra TLS roots.
//!
//! `GROK_EXTRA_CA_BUNDLE` names a PEM bundle whose candidate certificate DER
//! bytes are loaded at most once per process. The first call latches both the
//! environment setting and the file contents; changing either later has no
//! effect. The setting is default-off, additive to normal trust roots, capped
//! at 1 MiB, and fail-open.

use std::io::Read;
use std::sync::OnceLock;

use base64::Engine as _;

pub const MAX_EXTRA_CA_BUNDLE_BYTES: u64 = 1024 * 1024;
pub const ENV_GROK_EXTRA_CA_BUNDLE: &str = "GROK_EXTRA_CA_BUNDLE";

static EXTRA_ROOT_DER: OnceLock<Vec<Vec<u8>>> = OnceLock::new();
static REQWEST_ASYNC_ROOTS: OnceLock<Vec<reqwest::Certificate>> = OnceLock::new();
static REQWEST_BLOCKING_ROOTS: OnceLock<Vec<reqwest::Certificate>> = OnceLock::new();

#[cfg(test)]
thread_local! {
    static ASYNC_ADAPTER_INVOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Return the process-wide candidate certificate DER bytes.
///
/// This API is intentionally independent of an HTTP or TLS provider so crates
/// using another reqwest version can validate and adapt each candidate locally.
/// The environment and bundle contents are read only on the first call.
#[must_use]
pub fn extra_root_certificate_der() -> &'static [Vec<u8>] {
    EXTRA_ROOT_DER.get_or_init(load_extra_root_der).as_slice()
}

/// Add independently validated extra roots to a reqwest 0.12 async builder.
///
/// Every candidate is parsed with `Certificate::from_der` and tested in an
/// isolated one-root client build before it reaches the caller's builder. A
/// malformed root is skipped without preventing other roots from being added.
#[must_use]
pub fn with_extra_root_certificates(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    #[cfg(test)]
    ASYNC_ADAPTER_INVOCATIONS.with(|count| count.set(count.get() + 1));

    add_async_roots(
        builder,
        REQWEST_ASYNC_ROOTS.get_or_init(|| validate_async_roots(extra_root_certificate_der())),
    )
}

/// Add independently validated extra roots to a reqwest 0.12 blocking builder.
///
/// Validation is separate from the async adapter so a failure in one adapter
/// cannot poison the other or the caller's complete client configuration.
#[must_use]
pub fn with_extra_root_certificates_blocking(
    builder: reqwest::blocking::ClientBuilder,
) -> reqwest::blocking::ClientBuilder {
    add_blocking_roots(
        builder,
        REQWEST_BLOCKING_ROOTS
            .get_or_init(|| validate_blocking_roots(extra_root_certificate_der())),
    )
}

fn add_async_roots(
    mut builder: reqwest::ClientBuilder,
    roots: &[reqwest::Certificate],
) -> reqwest::ClientBuilder {
    for certificate in roots {
        builder = builder.add_root_certificate(certificate.clone());
    }
    builder
}

fn add_blocking_roots(
    mut builder: reqwest::blocking::ClientBuilder,
    roots: &[reqwest::Certificate],
) -> reqwest::blocking::ClientBuilder {
    for certificate in roots {
        builder = builder.add_root_certificate(certificate.clone());
    }
    builder
}

fn validate_async_roots(candidates: &[Vec<u8>]) -> Vec<reqwest::Certificate> {
    let mut accepted = Vec::new();
    for der in candidates {
        let Ok(certificate) = reqwest::Certificate::from_der(der) else {
            continue;
        };
        if reqwest::Client::builder()
            .no_proxy()
            .add_root_certificate(certificate.clone())
            .build()
            .is_ok()
        {
            accepted.push(certificate);
        }
    }
    log_adapter_result("reqwest 0.12 async", candidates.len(), accepted.len());
    accepted
}

fn validate_blocking_roots(candidates: &[Vec<u8>]) -> Vec<reqwest::Certificate> {
    let mut accepted = Vec::new();
    for der in candidates {
        let Ok(certificate) = reqwest::Certificate::from_der(der) else {
            continue;
        };
        let validation_certificate = certificate.clone();
        let valid = std::thread::spawn(move || {
            reqwest::blocking::Client::builder()
                .no_proxy()
                .add_root_certificate(validation_certificate)
                .build()
                .is_ok()
        })
        .join()
        .unwrap_or(false);
        if valid {
            accepted.push(certificate);
        }
    }
    log_adapter_result("reqwest 0.12 blocking", candidates.len(), accepted.len());
    accepted
}

fn log_adapter_result(adapter: &str, candidates: usize, accepted: usize) {
    let rejected = candidates.saturating_sub(accepted);
    if rejected > 0 {
        tracing::warn!(
            adapter,
            accepted,
            rejected,
            "GROK_EXTRA_CA_BUNDLE adapter skipped unusable certificate roots"
        );
    }
}

fn load_extra_root_der() -> Vec<Vec<u8>> {
    let path = match std::env::var_os(ENV_GROK_EXTRA_CA_BUNDLE) {
        Some(path) if !path.is_empty() => std::path::PathBuf::from(path),
        _ => return Vec::new(),
    };

    let bytes = match read_bundle_capped(&path) {
        Ok(bytes) => bytes,
        Err(BundleReadError::Io(error)) => {
            tracing::warn!(
                path = %path.display(),
                %error,
                "GROK_EXTRA_CA_BUNDLE unreadable; continuing without extra roots"
            );
            return Vec::new();
        }
        Err(BundleReadError::NotRegular) => {
            tracing::warn!(
                path = %path.display(),
                "GROK_EXTRA_CA_BUNDLE is not a regular file; continuing without extra roots"
            );
            return Vec::new();
        }
        Err(BundleReadError::TooLarge) => {
            tracing::warn!(
                path = %path.display(),
                max_bytes = MAX_EXTRA_CA_BUNDLE_BYTES,
                "GROK_EXTRA_CA_BUNDLE exceeds size cap; continuing without extra roots"
            );
            return Vec::new();
        }
    };

    let parsed = parse_pem_candidates(&bytes);
    if !parsed.saw_block {
        tracing::warn!(
            path = %path.display(),
            "GROK_EXTRA_CA_BUNDLE contains no PEM certificate blocks; continuing without extra roots"
        );
    } else if parsed.der.is_empty() {
        tracing::warn!(
            path = %path.display(),
            rejected = parsed.rejected,
            "GROK_EXTRA_CA_BUNDLE produced zero certificate candidates; continuing without extra roots"
        );
    } else {
        if parsed.rejected > 0 {
            tracing::warn!(
                path = %path.display(),
                accepted = parsed.der.len(),
                rejected = parsed.rejected,
                "GROK_EXTRA_CA_BUNDLE dropped malformed PEM certificate blocks"
            );
        }
        tracing::info!(
            path = %path.display(),
            candidates = parsed.der.len(),
            "GROK_EXTRA_CA_BUNDLE loaded candidate root certificates"
        );
    }
    parsed.der
}

#[derive(Debug)]
enum BundleReadError {
    Io(std::io::Error),
    NotRegular,
    TooLarge,
}

fn read_bundle_capped(path: &std::path::Path) -> Result<Vec<u8>, BundleReadError> {
    let path_metadata = std::fs::symlink_metadata(path).map_err(BundleReadError::Io)?;
    if !path_metadata.file_type().is_file() {
        return Err(BundleReadError::NotRegular);
    }
    if path_metadata.len() > MAX_EXTRA_CA_BUNDLE_BYTES {
        return Err(BundleReadError::TooLarge);
    }

    let file = open_bundle_nonblocking(path).map_err(BundleReadError::Io)?;
    let file_metadata = file.metadata().map_err(BundleReadError::Io)?;
    if !file_metadata.is_file() {
        return Err(BundleReadError::NotRegular);
    }
    if file_metadata.len() > MAX_EXTRA_CA_BUNDLE_BYTES {
        return Err(BundleReadError::TooLarge);
    }

    let mut bytes = Vec::new();
    file.take(MAX_EXTRA_CA_BUNDLE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(BundleReadError::Io)?;
    if bytes.len() as u64 > MAX_EXTRA_CA_BUNDLE_BYTES {
        return Err(BundleReadError::TooLarge);
    }
    Ok(bytes)
}

#[cfg(unix)]
fn open_bundle_nonblocking(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NONBLOCK | nix::libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(not(unix))]
fn open_bundle_nonblocking(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    std::fs::File::open(path)
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ParsedPemCandidates {
    der: Vec<Vec<u8>>,
    rejected: usize,
    saw_block: bool,
}

/// Scan PEM certificate blocks without allowing malformed or unterminated
/// material to consume a later valid block. DER validity is deliberately left
/// to each HTTP-provider adapter.
fn parse_pem_candidates(bytes: &[u8]) -> ParsedPemCandidates {
    const BEGIN: &[u8] = b"-----BEGIN CERTIFICATE-----";
    const END: &[u8] = b"-----END CERTIFICATE-----";

    let mut parsed = ParsedPemCandidates::default();
    let mut cursor = 0;
    while let Some(begin_offset) = find_bytes(&bytes[cursor..], BEGIN) {
        parsed.saw_block = true;
        let begin = cursor + begin_offset;
        let body_start = begin + BEGIN.len();
        let next_end = find_bytes(&bytes[body_start..], END);
        let next_begin = find_bytes(&bytes[body_start..], BEGIN);

        // A nested BEGIN means the current block is malformed. Resume at that
        // marker rather than letting a later END hide the nested valid block.
        if let Some(nested_begin) = next_begin
            && next_end.is_none_or(|end| nested_begin < end)
        {
            parsed.rejected += 1;
            cursor = body_start + nested_begin;
            continue;
        }

        let Some(end_offset) = next_end else {
            parsed.rejected += 1;
            break;
        };
        let end_start = body_start + end_offset;
        let body = &bytes[body_start..end_start];
        let compact: Vec<u8> = body
            .iter()
            .copied()
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect();
        match base64::engine::general_purpose::STANDARD.decode(compact) {
            Ok(der) if !der.is_empty() => parsed.der.push(der),
            Ok(_) | Err(_) => parsed.rejected += 1,
        }
        cursor = end_start + END.len();
    }
    parsed
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
pub(crate) fn async_adapter_invocations() -> usize {
    ASYNC_ADAPTER_INVOCATIONS.with(std::cell::Cell::get)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_CERTIFICATE_PEM: &str = r#"-----BEGIN CERTIFICATE-----
MIIDGzCCAgOgAwIBAgIUApvYkIdxEUFiCj3O9Qb8wII+hc0wDQYJKoZIhvcNAQEL
BQAwHTEbMBkGA1UEAwwSZ3Jvay1leHRyYS1jYS10ZXN0MB4XDTI2MDgxNTIzMTgz
OFoXDTI2MDgxNjIzMTgzOFowHTEbMBkGA1UEAwwSZ3Jvay1leHRyYS1jYS10ZXN0
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAjBEGLIYvtTUvvb2c+IFj
dd3TJ+eVFgl8r3ErMi7KsJl8IjNnNyDTqfjTkld3v8J6Nq6GEOy2yDUhUGAfCoKQ
tPoRDhC/CUCd2woaSRnE/RoNECusyNF5Mu08I68OfNCytw3TM1pFbRWLn2jx/fnW
nbVySeRx6UivbaYjXn9XCXoLZ8WSD5SaOCovHQK74JLnk75m3wkylJVjyG//eREN
Vm/nq8qIKhcwr2Vvv8FI8wVH/iB/csP4SE3i3oJhIJgFbHJf4AtJ8hEBvCApdSwB
pCL+6jgthfiaQmZfFAisXQXaXXt8LKqKr9W5XJt9aWpLgpN34O0Ekm97HECPIXch
VwIDAQABo1MwUTAdBgNVHQ4EFgQUOQdm9x29ElSD64Dc0KtW+XKoeZMwHwYDVR0j
BBgwFoAUOQdm9x29ElSD64Dc0KtW+XKoeZMwDwYDVR0TAQH/BAUwAwEB/zANBgkq
hkiG9w0BAQsFAAOCAQEAZsiQpvgqUaLgIp5ecO2swl6NJ444rOw6af56mFqkQVmz
Fvg78n5fNaG8EFCjSDLFCLGj6ucciA7R81TkC0r+I/LJ48sPZsHWTdcoTfmuIglG
iZyGj/4MUMnslgB8rHw0k4CzdhDo+AKe3q4IOfRL10cooHjIlsFjTFrkOJl/N/BC
cmgfFpfXzlht3t93qs4zyzJKYqApZKA859Y0uNeYDxsnE5HxLSJvb/PCmOF1Md5i
ioRElRvlZ53Jvp4nDjDR1VjMzKF5uTxcCeJTF20MjH910qOaAq7et8MkEvNTZSEm
lubl+ZOAxg+2uOhbMMzW2ubUN8RtbEEQRwtL31bDFw==
-----END CERTIFICATE-----
"#;
    const INVALID_BASE64_PEM: &str =
        "-----BEGIN CERTIFICATE-----\nnot valid base64!\n-----END CERTIFICATE-----\n";

    fn valid_der() -> Vec<u8> {
        parse_pem_candidates(VALID_CERTIFICATE_PEM.as_bytes())
            .der
            .into_iter()
            .next()
            .expect("valid fixture must decode")
    }

    #[test]
    fn loader_parses_valid_certificate() {
        let parsed = parse_pem_candidates(VALID_CERTIFICATE_PEM.as_bytes());
        assert_eq!(parsed.der.len(), 1);
        assert_eq!(parsed.rejected, 0);
        assert!(parsed.saw_block);
    }

    #[test]
    fn loader_parses_multiple_certificates() {
        let bundle = format!("{VALID_CERTIFICATE_PEM}{VALID_CERTIFICATE_PEM}");
        let parsed = parse_pem_candidates(bundle.as_bytes());
        assert_eq!(parsed.der.len(), 2);
        assert_eq!(parsed.rejected, 0);
    }

    #[test]
    fn loader_keeps_valid_certificates_from_mixed_bundle() {
        let bundle = format!("{VALID_CERTIFICATE_PEM}{INVALID_BASE64_PEM}{VALID_CERTIFICATE_PEM}");
        let parsed = parse_pem_candidates(bundle.as_bytes());
        assert_eq!(parsed.der.len(), 2);
        assert_eq!(parsed.rejected, 1);
    }

    #[test]
    fn loader_rejects_invalid_bundle() {
        let parsed = parse_pem_candidates(INVALID_BASE64_PEM.as_bytes());
        assert!(parsed.der.is_empty());
        assert_eq!(parsed.rejected, 1);
        assert!(parsed.saw_block);
    }

    #[test]
    fn unterminated_material_does_not_hide_later_valid_certificate() {
        let bundle = format!("-----BEGIN CERTIFICATE-----\nAAAA\n{VALID_CERTIFICATE_PEM}");
        let parsed = parse_pem_candidates(bundle.as_bytes());
        assert_eq!(parsed.der.len(), 1);
        assert_eq!(parsed.rejected, 1);
    }

    #[test]
    fn capped_reader_accepts_exact_limit_and_rejects_one_byte_over() {
        let dir = std::env::temp_dir().join(format!("grok-extra-ca-test-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir(&dir).unwrap();
        let result = (|| {
            let at_limit = dir.join("at-limit.pem");
            std::fs::write(&at_limit, vec![b'A'; MAX_EXTRA_CA_BUNDLE_BYTES as usize]).unwrap();
            assert_eq!(
                read_bundle_capped(&at_limit).unwrap().len(),
                MAX_EXTRA_CA_BUNDLE_BYTES as usize
            );

            let oversized = dir.join("oversized.pem");
            std::fs::write(
                &oversized,
                vec![b'A'; MAX_EXTRA_CA_BUNDLE_BYTES as usize + 1],
            )
            .unwrap();
            assert!(matches!(
                read_bundle_capped(&oversized),
                Err(BundleReadError::TooLarge)
            ));
        })();
        let _ = std::fs::remove_dir_all(dir);
        result
    }

    #[test]
    fn capped_reader_rejects_non_regular_input() {
        let dir = std::env::temp_dir().join(format!("grok-extra-ca-file-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir(&dir).unwrap();
        let result = (|| {
            assert!(matches!(
                read_bundle_capped(&dir),
                Err(BundleReadError::NotRegular)
            ));

            #[cfg(unix)]
            {
                let target = dir.join("target.pem");
                std::fs::write(&target, VALID_CERTIFICATE_PEM).unwrap();
                let link = dir.join("link.pem");
                std::os::unix::fs::symlink(&target, &link).unwrap();
                assert!(matches!(
                    read_bundle_capped(&link),
                    Err(BundleReadError::NotRegular)
                ));
            }
        })();
        let _ = std::fs::remove_dir_all(dir);
        result
    }

    #[cfg(unix)]
    #[test]
    fn capped_reader_rejects_fifo_without_opening_it() {
        use nix::sys::stat::Mode;
        use nix::unistd::mkfifo;

        let dir = std::env::temp_dir().join(format!("grok-extra-ca-fifo-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir(&dir).unwrap();
        let fifo = dir.join("bundle.pem");
        mkfifo(&fifo, Mode::S_IRUSR | Mode::S_IWUSR).unwrap();

        assert!(matches!(
            read_bundle_capped(&fifo),
            Err(BundleReadError::NotRegular)
        ));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn reqwest_012_async_adapter_accepts_valid_and_skips_invalid_der() {
        let roots = validate_async_roots(&[valid_der(), vec![0, 1, 2, 3]]);
        assert_eq!(roots.len(), 1);
        add_async_roots(reqwest::Client::builder(), &roots)
            .build()
            .expect("async builder with accepted root must build");
    }

    #[test]
    fn reqwest_012_blocking_adapter_accepts_valid_and_skips_invalid_der() {
        let roots = validate_blocking_roots(&[valid_der(), vec![0, 1, 2, 3]]);
        assert_eq!(roots.len(), 1);
        std::thread::spawn(move || {
            add_blocking_roots(reqwest::blocking::Client::builder(), &roots)
                .build()
                .expect("blocking builder with accepted root must build");
        })
        .join()
        .expect("blocking builder thread must not panic");
    }

    #[test]
    fn process_isolated_environment_is_default_off_and_reads_unicode_path_once() {
        const CHILD_MODE: &str = "GROK_EXTRA_CA_TEST_CHILD";
        if let Some(mode) = std::env::var_os(CHILD_MODE) {
            match mode.to_str() {
                Some("default-off") => assert!(extra_root_certificate_der().is_empty()),
                Some("configured") => {
                    let first = extra_root_certificate_der();
                    assert_eq!(first.len(), 1);
                    assert!(std::ptr::eq(first, extra_root_certificate_der()));
                }
                other => panic!("unexpected child mode: {other:?}"),
            }
            return;
        }

        let dir = std::env::temp_dir().join(format!("grok-extra-ca-env-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir(&dir).unwrap();
        let bundle = dir.join("extra-ca-ß-证.pem");
        std::fs::write(&bundle, VALID_CERTIFICATE_PEM).unwrap();
        let test_name = "extra_ca::tests::process_isolated_environment_is_default_off_and_reads_unicode_path_once";

        let default_status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test_name, "--nocapture"])
            .env(CHILD_MODE, "default-off")
            .env_remove(ENV_GROK_EXTRA_CA_BUNDLE)
            .status()
            .unwrap();
        assert!(default_status.success());

        let configured_status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test_name, "--nocapture"])
            .env(CHILD_MODE, "configured")
            .env(ENV_GROK_EXTRA_CA_BUNDLE, &bundle)
            .status()
            .unwrap();
        assert!(configured_status.success());

        let _ = std::fs::remove_dir_all(dir);
    }
}
