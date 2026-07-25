//! Semantic zero-uncovered inventory checks for platform client + CLI bindings.

use super::generated::{OPERATION_BINDINGS, OperationBinding};
use crate::compatibility::{openai_inventory, openrouter_inventory};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// An inventory endpoint that lacks a typed client binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UncoveredOperation {
    pub provider: String,
    pub method: String,
    pub path: String,
    pub operation_id: String,
    pub reason: String,
}

fn openai_keys() -> BTreeSet<(String, String)> {
    OPERATION_BINDINGS
        .iter()
        .filter(|b| b.provider == "openai" || b.provider == "openai_admin")
        .filter(|b| b.primary_or_default())
        .map(|b| (b.method.to_ascii_uppercase(), b.path.to_string()))
        .collect()
}

fn openrouter_keys() -> BTreeSet<(String, String)> {
    OPERATION_BINDINGS
        .iter()
        .filter(|b| b.provider == "openrouter")
        .filter(|b| b.primary_or_default())
        .map(|b| (b.method.to_ascii_uppercase(), b.path.to_string()))
        .collect()
}

trait PrimaryExt {
    fn primary_or_default(self) -> bool;
}

impl PrimaryExt for &&OperationBinding {
    fn primary_or_default(self) -> bool {
        // Stream companions use operation_id suffix; still valid coverage for SSE.
        true
    }
}

/// Semantic defects on a binding row.
pub fn semantic_defects(b: &OperationBinding) -> Vec<String> {
    let mut defects = Vec::new();
    if b.generic_value_body {
        defects.push("generic_value_body".into());
    }
    if !b.typed_request {
        defects.push("untyped_request".into());
    }
    if !b.typed_response {
        defects.push("untyped_response".into());
    }
    if b.request_type.is_empty() || b.response_type.is_empty() {
        defects.push("missing_type_names".into());
    }
    if b.client_method.is_empty() || b.cli_route.is_empty() {
        defects.push("missing_method_or_cli_route".into());
    }
    if b.transports.is_empty() {
        defects.push("missing_transports".into());
    }
    // Transport capability consistency.
    if b.is_multipart && !b.transports.contains(&"http_multipart") {
        defects.push("multipart_flag_without_transport".into());
    }
    if b.is_sse && !b.transports.contains(&"http_sse") {
        defects.push("sse_flag_without_transport".into());
    }
    if b.is_binary && !b.transports.contains(&"http_binary") {
        defects.push("binary_flag_without_transport".into());
    }
    // Admin credential class.
    if b.is_admin && b.provider != "openai_admin" && !b.path.starts_with("/organization") {
        // openrouter admin is separate namespace
    }
    if b.provider == "openai_admin" && !b.is_admin {
        defects.push("openai_admin_without_admin_flag".into());
    }
    // Unknown method is not allowed.
    if !matches!(
        b.method,
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"
    ) {
        defects.push(format!("unsupported_method_{}", b.method));
    }
    defects
}

/// Return every baseline endpoint without a generated client method.
pub fn uncovered_operations() -> Result<Vec<UncoveredOperation>, String> {
    let mut missing = Vec::new();
    let openai_bound = openai_keys();
    let openai = openai_inventory();
    for ep in &openai.endpoints {
        let key = (ep.method.to_ascii_uppercase(), ep.path.clone());
        if !openai_bound.contains(&key) {
            missing.push(UncoveredOperation {
                provider: "openai".into(),
                method: ep.method.clone(),
                path: ep.path.clone(),
                operation_id: ep
                    .operation_id
                    .clone()
                    .unwrap_or_else(|| format!("{} {}", ep.method, ep.path)),
                reason: "missing_method_path_binding".into(),
            });
        }
    }
    let or_bound = openrouter_keys();
    let openrouter = openrouter_inventory();
    for ep in &openrouter.endpoints {
        let key = (ep.method.to_ascii_uppercase(), ep.path.clone());
        if !or_bound.contains(&key) {
            missing.push(UncoveredOperation {
                provider: "openrouter".into(),
                method: ep.method.clone(),
                path: ep.path.clone(),
                operation_id: ep
                    .operation_id
                    .clone()
                    .unwrap_or_else(|| format!("{} {}", ep.method, ep.path)),
                reason: "missing_method_path_binding".into(),
            });
        }
    }
    // Semantic defects on every binding.
    for b in OPERATION_BINDINGS {
        for d in semantic_defects(b) {
            missing.push(UncoveredOperation {
                provider: b.provider.into(),
                method: b.method.into(),
                path: b.path.into(),
                operation_id: b.operation_id.into(),
                reason: d,
            });
        }
    }
    Ok(missing)
}

/// Fail when any baseline endpoint is unbound or semantically incomplete.
pub fn assert_zero_uncovered_operations() -> Result<(), String> {
    let missing = uncovered_operations()?;
    if missing.is_empty() {
        Ok(())
    } else {
        let sample: Vec<String> = missing
            .iter()
            .take(20)
            .map(|m| {
                format!(
                    "{} {} {} ({}) [{}]",
                    m.provider, m.method, m.path, m.operation_id, m.reason
                )
            })
            .collect();
        Err(format!(
            "{} uncovered/semantic defects; sample: {}",
            missing.len(),
            sample.join("; ")
        ))
    }
}

/// Machine-readable coverage report (no credentials).
pub fn coverage_report_json() -> Result<Value, String> {
    let missing = uncovered_operations()?;
    let openai = openai_inventory();
    let openrouter = openrouter_inventory();
    Ok(json!({
        "format_version": 2,
        "openai_baseline_endpoints": openai.endpoints.len(),
        "openrouter_baseline_endpoints": openrouter.endpoints.len(),
        "bindings_total": OPERATION_BINDINGS.len(),
        "uncovered_count": missing.len(),
        "generic_value_body_count": OPERATION_BINDINGS.iter().filter(|b| b.generic_value_body).count(),
        "multipart_bound": OPERATION_BINDINGS.iter().filter(|b| b.is_multipart).count(),
        "sse_bound": OPERATION_BINDINGS.iter().filter(|b| b.is_sse).count(),
        "binary_bound": OPERATION_BINDINGS.iter().filter(|b| b.is_binary).count(),
        "websocket_bound": OPERATION_BINDINGS.iter().filter(|b| b.is_websocket).count(),
        "uncovered": missing.iter().map(|m| json!({
            "provider": m.provider,
            "method": m.method,
            "path": m.path,
            "operation_id": m.operation_id,
            "reason": m.reason,
        })).collect::<Vec<_>>(),
        "binding_providers": {
            "openai": OPERATION_BINDINGS.iter().filter(|b| b.provider == "openai").count(),
            "openai_admin": OPERATION_BINDINGS.iter().filter(|b| b.provider == "openai_admin").count(),
            "openrouter": OPERATION_BINDINGS.iter().filter(|b| b.provider == "openrouter").count(),
        }
    }))
}

/// Lookup a binding by provider + operation_id.
pub fn find_binding(provider: &str, operation_id: &str) -> Option<&'static OperationBinding> {
    OPERATION_BINDINGS
        .iter()
        .find(|b| b.provider == provider && b.operation_id == operation_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_uncovered_against_pinned_baselines() {
        assert_zero_uncovered_operations().expect("all baseline ops must be bound semantically");
    }

    #[test]
    fn no_generic_value_request_bodies() {
        let n = OPERATION_BINDINGS
            .iter()
            .filter(|b| b.generic_value_body)
            .count();
        assert_eq!(n, 0, "generic Value request bodies are prohibited");
    }

    #[test]
    fn every_binding_has_cli_route_and_typed_names() {
        for b in OPERATION_BINDINGS {
            assert!(!b.cli_route.is_empty(), "{}", b.operation_id);
            assert!(!b.client_method.is_empty(), "{}", b.operation_id);
            assert!(!b.request_type.is_empty(), "{}", b.operation_id);
            assert!(!b.response_type.is_empty(), "{}", b.operation_id);
            assert!(b.typed_request && b.typed_response, "{}", b.operation_id);
        }
    }

    #[test]
    fn admin_bindings_are_admin_flagged() {
        for b in OPERATION_BINDINGS
            .iter()
            .filter(|b| b.provider == "openai_admin")
        {
            assert!(b.is_admin, "{}", b.operation_id);
        }
    }

    #[test]
    fn coverage_report_is_redacted_shape() {
        let report = coverage_report_json().unwrap();
        assert!(report.get("uncovered_count").is_some());
        let s = report.to_string();
        assert!(!s.contains("Bearer "));
    }

    #[test]
    fn create_chat_completion_is_typed_schema_body() {
        let b = find_binding("openai", "createChatCompletion").expect("chat binding");
        assert_eq!(b.request_type, "CreateChatCompletionParams");
        assert!(!b.generic_value_body);
        assert!(b.typed_request);
        assert!(!b.request_type.contains("Value"));
    }

    #[test]
    fn multipart_ops_use_multipart_transport_flag() {
        let n = OPERATION_BINDINGS.iter().filter(|b| b.is_multipart).count();
        assert!(n > 0, "expected multipart bindings");
        for b in OPERATION_BINDINGS.iter().filter(|b| b.is_multipart) {
            assert!(
                b.transports.contains(&"http_multipart"),
                "{}",
                b.operation_id
            );
        }
    }
}
