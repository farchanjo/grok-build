//! Zero-uncovered inventory checks for platform client + CLI bindings.

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
}

fn openai_keys() -> BTreeSet<(String, String)> {
    OPERATION_BINDINGS
        .iter()
        .filter(|b| b.provider == "openai" || b.provider == "openai_admin")
        .map(|b| (b.method.to_ascii_uppercase(), b.path.to_string()))
        .collect()
}

fn openrouter_keys() -> BTreeSet<(String, String)> {
    OPERATION_BINDINGS
        .iter()
        .filter(|b| b.provider == "openrouter")
        .map(|b| (b.method.to_ascii_uppercase(), b.path.to_string()))
        .collect()
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
            });
        }
    }
    Ok(missing)
}

/// Fail when any baseline endpoint is unbound.
pub fn assert_zero_uncovered_operations() -> Result<(), String> {
    let missing = uncovered_operations()?;
    if missing.is_empty() {
        Ok(())
    } else {
        let sample: Vec<String> = missing
            .iter()
            .take(12)
            .map(|m| format!("{} {} {} ({})", m.provider, m.method, m.path, m.operation_id))
            .collect();
        Err(format!(
            "{} uncovered operations; sample: {}",
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
        "format_version": 1,
        "openai_baseline_endpoints": openai.endpoints.len(),
        "openrouter_baseline_endpoints": openrouter.endpoints.len(),
        "bindings_total": OPERATION_BINDINGS.len(),
        "uncovered_count": missing.len(),
        "uncovered": missing.iter().map(|m| json!({
            "provider": m.provider,
            "method": m.method,
            "path": m.path,
            "operation_id": m.operation_id,
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
        assert_zero_uncovered_operations().expect("all baseline ops must be bound");
    }

    #[test]
    fn coverage_report_is_redacted_shape() {
        let report = coverage_report_json().unwrap();
        assert!(report.get("uncovered_count").is_some());
        let s = report.to_string();
        assert!(!s.contains("Bearer "));
        assert!(!s.contains("api_key"));
    }

    #[test]
    fn every_binding_has_cli_route() {
        for b in OPERATION_BINDINGS {
            assert!(!b.cli_route.is_empty(), "{}", b.operation_id);
            assert!(!b.client_method.is_empty(), "{}", b.operation_id);
            assert!(!b.request_type.is_empty(), "{}", b.operation_id);
        }
    }
}
