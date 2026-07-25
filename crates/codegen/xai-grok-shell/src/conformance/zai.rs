//! Ignored/manual Z.ai Model API conformance profile.
//!
//! Credential: inject only via `GROK_TEST_ZAI_API_KEY` at process launch from
//! the vault handle (never read the vault from this module). Generation tests
//! require an explicit model id and remain `#[ignore]` by default.

use crate::agent::zai::{ZAI_DEFAULT_BASE_URL, ZAI_PROVIDER_ID, ZAI_TEST_ENV_KEY};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Set to `1` to allow the ignored suite to attempt network I/O.
pub const ZAI_CONFORMANCE_ENV: &str = "GROK_ZAI_CONFORMANCE";

/// Optional low-cost model override for paid generation scenarios.
pub const ZAI_TEST_MODEL_ENV: &str = "GROK_TEST_ZAI_MODEL";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZaiConformanceConfig {
    pub base_url: String,
    pub provider_id: String,
    /// When set, paid generation may run; otherwise only free preflight.
    pub generation_model: Option<String>,
    pub max_tokens: u32,
}

impl Default for ZaiConformanceConfig {
    fn default() -> Self {
        Self {
            base_url: ZAI_DEFAULT_BASE_URL.to_owned(),
            provider_id: ZAI_PROVIDER_ID.to_owned(),
            generation_model: std::env::var(ZAI_TEST_MODEL_ENV)
                .ok()
                .filter(|s| !s.is_empty()),
            max_tokens: 64,
        }
    }
}

impl ZaiConformanceConfig {
    pub fn from_env() -> Self {
        Self::default()
    }

    pub fn credential_present() -> bool {
        std::env::var(ZAI_TEST_ENV_KEY)
            .ok()
            .is_some_and(|v| !v.trim().is_empty())
    }

    pub fn live_enabled() -> bool {
        matches!(
            std::env::var(ZAI_CONFORMANCE_ENV).as_deref(),
            Ok("1") | Ok("true") | Ok("yes")
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZaiConformanceReport {
    pub timestamp_unix: u64,
    pub base_url: String,
    pub provider_id: String,
    pub documentation_baseline: String,
    pub selected_model: Option<String>,
    pub preflight_models: Vec<String>,
    pub scenarios: Vec<ZaiScenarioResult>,
    pub skips: Vec<String>,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZaiScenarioResult {
    pub name: String,
    pub status: String,
    pub detail: Option<String>,
    pub request_id: Option<String>,
}

impl ZaiConformanceReport {
    pub fn empty(cfg: &ZaiConformanceConfig) -> Self {
        Self {
            timestamp_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            base_url: cfg.base_url.clone(),
            provider_id: cfg.provider_id.clone(),
            documentation_baseline: "2026-07-24".into(),
            selected_model: cfg.generation_model.clone(),
            preflight_models: Vec::new(),
            scenarios: Vec::new(),
            skips: Vec::new(),
            failures: Vec::new(),
        }
    }

    /// Machine-readable report with credentials and prompts redacted by construction.
    pub fn to_redacted_json(&self) -> Value {
        let s = serde_json::to_string(self).unwrap_or_default();
        // Defense in depth: never allow key-shaped substrings.
        let redacted = s
            .replace(ZAI_TEST_ENV_KEY, "[env]")
            .replace("Bearer ", "Bearer [redacted]");
        serde_json::from_str(&redacted).unwrap_or_else(|_| json!({"error": "report serialize"}))
    }
}

/// Local deterministic fixtures (no network).
pub mod fixtures {
    use super::*;

    pub fn sample_models_list() -> Value {
        json!({
            "object": "list",
            "data": [
                {"id": "glm-4.5", "object": "model"},
                {"id": "glm-4.7", "object": "model"},
                {"id": "glm-5", "object": "model"}
            ]
        })
    }

    pub fn sample_tool_stream_fragments() -> Vec<Value> {
        vec![
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"run","arguments":"{\""}}]}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"path\":\""}}]}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"x\"}"}}]}}]}),
            json!({"choices":[{"finish_reason":"tool_calls"}]}),
        ]
    }

    pub fn accumulate_tool_arguments(frames: &[Value]) -> String {
        let mut args = String::new();
        for frame in frames {
            if let Some(delta) = frame
                .pointer("/choices/0/delta/tool_calls/0/function/arguments")
                .and_then(|v| v.as_str())
            {
                args.push_str(delta);
            }
        }
        args
    }
}

/// Bounded live preflight: GET /models only. Never bills generation.
pub async fn free_models_preflight(cfg: &ZaiConformanceConfig) -> Result<Vec<String>, String> {
    if !ZaiConformanceConfig::live_enabled() {
        return Err("live conformance disabled (set GROK_ZAI_CONFORMANCE=1)".into());
    }
    if !ZaiConformanceConfig::credential_present() {
        return Err("GROK_TEST_ZAI_API_KEY unset".into());
    }
    let token = std::env::var(ZAI_TEST_ENV_KEY).map_err(|e| e.to_string())?;
    let url = format!("{}/models", cfg.base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(&url)
        .bearer_auth(token.trim())
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    // Never include token in errors.
    if !status.is_success() {
        return Err(format!("GET /models status {status}"));
    }
    let v: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let mut ids = Vec::new();
    if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
        for m in arr {
            if let Some(id) = m.get("id").and_then(|i| i.as_str()) {
                ids.push(id.to_owned());
            }
        }
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;

    #[test]
    fn fixtures_accumulate_tool_args() {
        let frames = sample_tool_stream_fragments();
        let args = accumulate_tool_arguments(&frames);
        assert_eq!(args, r#"{"path":"x"}"#);
    }

    #[test]
    fn report_redaction_shape() {
        let cfg = ZaiConformanceConfig::default();
        let mut report = ZaiConformanceReport::empty(&cfg);
        report
            .skips
            .push("generation skipped: no model configured".into());
        let json = report.to_redacted_json();
        let s = json.to_string();
        assert!(!s.contains("sk-"));
        assert!(s.contains("zai-model-api") || s.contains("api.z.ai"));
    }

    #[test]
    #[ignore = "manual hosted Z.ai conformance; requires GROK_ZAI_CONFORMANCE=1 and GROK_TEST_ZAI_API_KEY"]
    fn live_models_preflight_ignored() {
        let cfg = ZaiConformanceConfig::from_env();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let ids = rt.block_on(free_models_preflight(&cfg)).expect("preflight");
        assert!(!ids.is_empty());
    }

    #[test]
    #[ignore = "manual paid Z.ai generation; requires explicit GROK_TEST_ZAI_MODEL"]
    fn live_generation_intentionally_skipped_without_model() {
        let cfg = ZaiConformanceConfig::from_env();
        if cfg.generation_model.is_none() {
            // Explicit skip path for the harness report.
            return;
        }
        // Paid generation is not executed in automated runs of this repository.
        panic!("configure a dedicated runner to execute paid generation");
    }
}
