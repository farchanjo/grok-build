//! ACP handlers for Prime index status, backfill, rebuild, and cancel.

use std::path::PathBuf;

use xai_grok_tools::util::grok_home::grok_home;

use crate::session::prime::{
    PrimeIndexOpKind, PrimeIndexOpRequest, cancel_job, prime_index_status, start_job,
};

use super::ExtResult;

fn cwd_from(req: &PrimeIndexOpRequest) -> PathBuf {
    PathBuf::from(req.cwd.as_deref().filter(|s| !s.is_empty()).unwrap_or("."))
}

fn version_error(message: &str) -> ExtResult {
    super::to_ext_response(Err::<serde_json::Value, _>(anyhow::anyhow!("{message}")))
}

pub fn mixed_version_error(api_version: Option<u32>) -> Option<ExtResult> {
    match crate::session::prime::require_api_version(api_version) {
        Ok(_) => None,
        Err(message) => Some(version_error(message)),
    }
}

pub async fn handle(args: &agent_client_protocol::ExtRequest) -> ExtResult {
    let method = args.method.as_ref();
    let req: PrimeIndexOpRequest = serde_json::from_str(args.params.get()).unwrap_or_default();
    if let Some(err) = mixed_version_error(req.api_version) {
        return err;
    }
    let home = grok_home();
    let cwd = cwd_from(&req);
    match method {
        "x.ai/prime/index/status" => match prime_index_status(&home, &cwd, &req) {
            Ok(status) => super::to_ext_response(Ok(status)),
            Err(e) => version_error(&e),
        },
        "x.ai/prime/index/backfill" => {
            match start_job(&home, &cwd, req, PrimeIndexOpKind::Backfill) {
                Ok(job) => super::to_ext_response(Ok(job)),
                Err(e) => version_error(&e),
            }
        }
        "x.ai/prime/index/rebuild" => {
            match start_job(&home, &cwd, req, PrimeIndexOpKind::Rebuild) {
                Ok(job) => super::to_ext_response(Ok(job)),
                Err(e) => version_error(&e),
            }
        }
        "x.ai/prime/index/cancel" => match cancel_job(&home, &cwd, &req) {
            Ok(job) => super::to_ext_response(Ok(job)),
            Err(e) => version_error(&e),
        },
        other => Err(agent_client_protocol::Error::method_not_found()
            .data(format!("unknown ACP extension method: {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_version_fail_closed() {
        assert!(mixed_version_error(None).is_some());
        assert!(mixed_version_error(Some(0)).is_some());
        assert!(mixed_version_error(Some(2)).is_some());
        assert!(mixed_version_error(Some(1)).is_none());
    }
}
