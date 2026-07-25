//! Ignored/manual `solaris` SSH-oriented conformance harness.
//!
//! Development-only. Never ships host endpoints as user defaults. Never
//! starts/stops remote services. Connect only when `GROK_SOLARIS_CONFORMANCE=1`
//! and SSH is available; stopped services are explicit skips.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const SOLARIS_HARNESS_ENV: &str = "GROK_SOLARIS_CONFORMANCE";
pub const SOLARIS_SSH_TARGET_ENV: &str = "GROK_SOLARIS_SSH_TARGET";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolarisServiceKind {
    SglangChat,
    LlamaCppChat,
    VllmEmbeddings,
    VllmReranker,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolarisServiceTarget {
    pub id: String,
    pub kind: SolarisServiceKind,
    /// Host-local port as seen on the SSH host.
    pub configured_port: u16,
    /// Historical port to reconcile during preflight (e.g. 30000 for glm).
    pub historical_ports: Vec<u16>,
    pub base_path: String,
}

impl SolarisServiceTarget {
    pub fn inventory() -> Vec<Self> {
        vec![
            Self {
                id: "solaris-qwen35-sglang".into(),
                kind: SolarisServiceKind::SglangChat,
                configured_port: 30000,
                historical_ports: vec![30000],
                base_path: "/v1".into(),
            },
            Self {
                id: "solaris-glm47-llama".into(),
                kind: SolarisServiceKind::LlamaCppChat,
                configured_port: 8000,
                historical_ports: vec![30000, 8000],
                base_path: "/v1".into(),
            },
            Self {
                id: "solaris-qwen3-embedding-vllm".into(),
                kind: SolarisServiceKind::VllmEmbeddings,
                configured_port: 8001,
                historical_ports: vec![8001],
                base_path: "/v1".into(),
            },
            Self {
                id: "solaris-qwen3-reranker-vllm".into(),
                kind: SolarisServiceKind::VllmReranker,
                configured_port: 8002,
                historical_ports: vec![8002],
                base_path: "/v1".into(),
            },
        ]
    }

    pub fn host_local_base_url(&self, port: u16) -> String {
        format!("http://127.0.0.1:{port}{}", self.base_path)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SolarisHarnessConfig {
    pub ssh_target: String,
    pub services: Vec<SolarisServiceTarget>,
}

impl Default for SolarisHarnessConfig {
    fn default() -> Self {
        Self {
            ssh_target: std::env::var(SOLARIS_SSH_TARGET_ENV)
                .unwrap_or_else(|_| "root@solaris".into()),
            services: SolarisServiceTarget::inventory(),
        }
    }
}

impl SolarisHarnessConfig {
    pub fn live_enabled() -> bool {
        matches!(
            std::env::var(SOLARIS_HARNESS_ENV).as_deref(),
            Ok("1") | Ok("true") | Ok("yes")
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SolarisServiceReport {
    pub id: String,
    pub configured_port: u16,
    pub observed_port: Option<u16>,
    pub implementation: Option<String>,
    pub model_id: Option<String>,
    pub status: String,
    pub skip_reason: Option<String>,
    pub capabilities: Vec<String>,
    pub results: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SolarisConformanceReport {
    pub timestamp_unix: u64,
    pub ssh_target_alias: String,
    pub services: Vec<SolarisServiceReport>,
}

impl SolarisConformanceReport {
    pub fn skeleton(cfg: &SolarisHarnessConfig) -> Self {
        Self {
            timestamp_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            ssh_target_alias: cfg.ssh_target.clone(),
            services: cfg
                .services
                .iter()
                .map(|s| SolarisServiceReport {
                    id: s.id.clone(),
                    configured_port: s.configured_port,
                    observed_port: None,
                    implementation: None,
                    model_id: None,
                    status: "not_run".into(),
                    skip_reason: None,
                    capabilities: Vec::new(),
                    results: Vec::new(),
                })
                .collect(),
        }
    }

    pub fn to_redacted_json(&self) -> Value {
        let mut v = serde_json::to_value(self).unwrap_or(json!({}));
        // Never embed credentials or remote filesystem paths.
        if let Some(obj) = v.as_object_mut() {
            obj.insert("redacted".into(), Value::Bool(true));
            obj.insert(
                "note".into(),
                Value::String(
                    "development-only harness; host endpoints are not shipping defaults".into(),
                ),
            );
        }
        v
    }
}

/// Read-only preflight contract description (no network).
pub fn preflight_contract(target: &SolarisServiceTarget) -> Value {
    json!({
        "id": target.id,
        "kind": target.kind,
        "checks": [
            format!("listener on configured port {}", target.configured_port),
            "GET /health when exposed",
            "GET /v1/models",
            "GET /openapi.json when exposed",
        ],
        "historical_ports": target.historical_ports,
        "generative_allowed": matches!(
            target.kind,
            SolarisServiceKind::SglangChat | SolarisServiceKind::LlamaCppChat
        ),
        "embeddings_only": matches!(target.kind, SolarisServiceKind::VllmEmbeddings),
        "note": "Embedding servers must not be marked chat-capable solely because OpenAPI lists chat paths"
    })
}

/// Mark a stopped service as an explicit skip (no start/stop).
pub fn skip_stopped(target: &SolarisServiceTarget, reason: &str) -> SolarisServiceReport {
    SolarisServiceReport {
        id: target.id.clone(),
        configured_port: target.configured_port,
        observed_port: None,
        implementation: None,
        model_id: None,
        status: "skipped".into(),
        skip_reason: Some(reason.to_owned()),
        capabilities: Vec::new(),
        results: vec!["service not listening; not started by harness".into()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_ports_match_plan() {
        let inv = SolarisServiceTarget::inventory();
        assert_eq!(inv[0].configured_port, 30000);
        assert_eq!(inv[1].configured_port, 8000);
        assert_eq!(inv[2].configured_port, 8001);
        assert_eq!(inv[3].configured_port, 8002);
    }

    #[test]
    fn embedding_target_not_marked_generative() {
        let emb = &SolarisServiceTarget::inventory()[2];
        let contract = preflight_contract(emb);
        assert_eq!(contract["embeddings_only"], true);
        assert_eq!(contract["generative_allowed"], false);
    }

    #[test]
    fn skip_stopped_is_explicit() {
        let t = &SolarisServiceTarget::inventory()[0];
        let r = skip_stopped(t, "no listener on 30000");
        assert_eq!(r.status, "skipped");
        assert!(r.skip_reason.unwrap().contains("30000"));
    }

    #[test]
    fn report_marks_dev_only() {
        let cfg = SolarisHarnessConfig::default();
        let report = SolarisConformanceReport::skeleton(&cfg);
        let s = report.to_redacted_json().to_string();
        assert!(s.contains("development-only"));
        assert!(!s.contains("password"));
    }

    #[test]
    #[ignore = "manual solaris SSH harness; requires GROK_SOLARIS_CONFORMANCE=1; does not start services"]
    fn live_solaris_preflight_ignored() {
        assert!(
            !SolarisHarnessConfig::live_enabled(),
            "this ignored test documents the opt-in gate only"
        );
    }
}
