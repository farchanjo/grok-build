//! reqwest 0.13 adaptation for provider-neutral extra root DER candidates.

use std::sync::OnceLock;

static REQWEST_013_ROOTS: OnceLock<Vec<reqwest::Certificate>> = OnceLock::new();

/// Add independently validated extra roots to a reqwest 0.13 builder.
///
/// Each candidate DER certificate is parsed and tested in an isolated one-root
/// client build. Invalid roots are skipped so one bad entry cannot poison MCP
/// HTTPS clients or prevent later valid roots from being installed.
pub(crate) fn with_extra_root_certificates(
    builder: reqwest::ClientBuilder,
) -> reqwest::ClientBuilder {
    add_roots(
        builder,
        REQWEST_013_ROOTS
            .get_or_init(|| validate_roots(xai_grok_tools::extra_ca::extra_root_certificate_der())),
    )
}

fn add_roots(
    mut builder: reqwest::ClientBuilder,
    roots: &[reqwest::Certificate],
) -> reqwest::ClientBuilder {
    for certificate in roots {
        builder = builder.add_root_certificate(certificate.clone());
    }
    builder
}

fn validate_roots(candidates: &[Vec<u8>]) -> Vec<reqwest::Certificate> {
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
    let rejected = candidates.len().saturating_sub(accepted.len());
    if rejected > 0 {
        tracing::warn!(
            adapter = "reqwest 0.13 async",
            accepted = accepted.len(),
            rejected,
            "GROK_EXTRA_CA_BUNDLE adapter skipped unusable certificate roots"
        );
    }
    accepted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reqwest_013_adapter_builds_and_skips_invalid_der() {
        let roots = validate_roots(&[vec![0, 1, 2, 3]]);
        assert!(roots.is_empty());
        add_roots(reqwest::Client::builder(), &roots)
            .build()
            .expect("reqwest 0.13 builder must survive an invalid candidate");
    }
}
